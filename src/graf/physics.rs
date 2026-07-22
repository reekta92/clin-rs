use parking_lot::RwLock;
use std::sync::{Arc, mpsc};

use super::graph::GraphState;
use crate::config::{ClinConfig, PhysicsTickRate};

pub(crate) const MAX_DYNAMIC_NODES: usize = 1_000;
const MIN_DYNAMIC_ALPHA: f32 = 0.005;

pub fn simulation_step(state: &mut GraphState, timestep: f32) {
    if state.alpha < MIN_DYNAMIC_ALPHA {
        state.is_settled = true;
        return;
    }

    let alpha = state.alpha;
    // Step dt is higher when alpha is hot, shrinking as it cools.
    let step_dt = timestep * (0.2 + 0.8 * alpha);
    state.simulation.update(step_dt);

    // Friction increases as temperature drops:
    // Hot (alpha=1.0) -> cooloff=0.95 (nodes fly freely to find space)
    // Freezing (alpha->0.0) -> cooloff=0.50 (bounces are dampened into crystalline lock)
    let cooloff = 0.50 + 0.45 * alpha;
    for node in state.simulation.get_graph_mut().node_weights_mut() {
        node.velocity *= cooloff;

        assert!(node.location.x.is_finite(), "x location must be finite");
        assert!(node.location.y.is_finite(), "y location must be finite");
    }

    if let Some((tx, ty)) = state.drag_target
        && let Some(idx) = state.dragging_node
        && let Some(node) = state.simulation.get_graph_mut().node_weight_mut(idx)
    {
        node.location.x = tx;
        node.location.y = ty;
        node.velocity = fdg_sim::glam::Vec3::ZERO;
        state.reheat(0.4);
    }

    let graph = state.simulation.get_graph();
    state.graph_bounds = super::render::compute_graph_bounds(graph);
    state.spatial_grid.rebuild(state.simulation.get_graph());

    // Temperature decays towards 0 (mathematical energy minimum)
    state.alpha *= 0.95;
    if state.alpha < MIN_DYNAMIC_ALPHA {
        state.is_settled = true;
    }
}

fn continuous_simulation_step(state: &mut GraphState, timestep: f32) {
    if state.alpha < MIN_DYNAMIC_ALPHA {
        state.alpha = MIN_DYNAMIC_ALPHA;
    }
    state.is_settled = false;

    simulation_step(state, timestep);

    if state.alpha < MIN_DYNAMIC_ALPHA {
        state.alpha = MIN_DYNAMIC_ALPHA;
    }
    state.is_settled = false;
}

pub fn start_physics(
    state: Arc<RwLock<GraphState>>,
    config: &ClinConfig,
) -> Option<mpsc::Sender<()>> {
    let node_count = { state.read().simulation.get_graph().node_count() };
    if node_count == 0 || node_count > MAX_DYNAMIC_NODES {
        let mut guard = state.write();
        if node_count > MAX_DYNAMIC_NODES {
            guard.apply_static_cluster_layout(config.graf.physics.ideal_distance);
        }
        guard.physics_worker_active = false;
        guard.is_settled = true;
        guard.alpha = 0.0;
        return None;
    }

    {
        let mut guard = state.write();
        guard.physics_worker_active = true;
    }

    let timestep = 0.12;

    // Compute tick rate based on config mode and node count
    let tick_rate_mode = config.graf.physics.tick_rate;
    let sleep_ms: u64 = if tick_rate_mode == PhysicsTickRate::Fixed {
        16
    } else {
        match node_count {
            0..=500 => 16,    // ~60Hz
            501..=1000 => 33, // ~30Hz
            _ => 66,
        }
    };

    let (kill_tx, kill_rx) = mpsc::channel();
    let state_clone = state.clone();

    std::thread::spawn(move || {
        loop {
            match kill_rx.try_recv() {
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {
                    let mut guard = state_clone.write();
                    guard.physics_worker_active = false;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }

            {
                let mut guard = state_clone.write();
                if let Some((tx, ty)) = guard.drag_target
                    && let Some(idx) = guard.dragging_node
                {
                    let graph = guard.simulation.get_graph_mut();
                    if let Some(node) = graph.node_weight_mut(idx) {
                        node.location.x = tx;
                        node.location.y = ty;
                        node.velocity = fdg_sim::glam::Vec3::ZERO;
                    }
                }
                continuous_simulation_step(&mut guard, timestep as f32);
            }

            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
        }
    });

    Some(kill_tx)
}
#[cfg(test)]
mod tests {
    use super::*;
    use fdg_sim::ForceGraphHelper;

    fn should_stop(res: Result<(), mpsc::TryRecvError>) -> bool {
        matches!(res, Ok(_) | Err(mpsc::TryRecvError::Disconnected))
    }

    #[test]
    fn test_physics_stop_condition() {
        assert!(should_stop(Ok(())));
        assert!(!should_stop(Err(mpsc::TryRecvError::Empty)));
        assert!(should_stop(Err(mpsc::TryRecvError::Disconnected)));
    }

    #[test]
    fn test_simulation_step_converges() {
        // Create minimal storage with two linked notes
        let temp_dir = tempfile::tempdir().unwrap();
        let notes_dir = temp_dir.path().join("notes");
        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();

        std::fs::write(notes_dir.join("a.md"), "[[b]]").unwrap();
        std::fs::write(notes_dir.join("b.md"), "[[a]]").unwrap();

        let storage = crate::storage::Storage {
            data_dir: temp_dir.path().join("data"),
            config_dir,
            notes_dir,
            templates_dir: temp_dir.path().join("templates"),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        std::fs::create_dir_all(&storage.data_dir).unwrap();
        std::fs::create_dir_all(&storage.templates_dir).unwrap();

        let config = crate::config::ClinConfig::default();
        let note_ids = storage.list_note_ids(true, false).unwrap();
        let summaries: Vec<_> = note_ids
            .iter()
            .filter_map(|id| storage.load_note_summary(id).ok())
            .collect();
        let mut gs = GraphState::new(&summaries, &config).expect("GraphState::new");

        // Run simulation steps
        for _ in 0..300 {
            simulation_step(&mut gs, 0.12);
            if gs.is_settled {
                break;
            }
        }

        // All node locations should be finite (not NaN or inf)
        let graph = gs.simulation.get_graph();
        for node in graph.node_weights() {
            assert!(node.location.x.is_finite(), "x should be finite");
            assert!(node.location.y.is_finite(), "y should be finite");
        }
        assert!(gs.is_settled || gs.graph_bounds != (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn test_simulation_step_enforces_drag_target() {
        let temp_dir = tempfile::tempdir().unwrap();
        let notes_dir = temp_dir.path().join("notes");
        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();

        std::fs::write(notes_dir.join("a.md"), "[[b]]").unwrap();
        std::fs::write(notes_dir.join("b.md"), "[[a]]").unwrap();

        let storage = crate::storage::Storage {
            data_dir: temp_dir.path().join("data"),
            config_dir,
            notes_dir,
            templates_dir: temp_dir.path().join("templates"),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        std::fs::create_dir_all(&storage.data_dir).unwrap();
        std::fs::create_dir_all(&storage.templates_dir).unwrap();

        let config = crate::config::ClinConfig::default();
        let note_ids = storage.list_note_ids(true, false).unwrap();
        let summaries: Vec<_> = note_ids
            .iter()
            .filter_map(|id| storage.load_note_summary(id).ok())
            .collect();
        let mut gs = GraphState::new(&summaries, &config).expect("GraphState::new");

        let node_indices: Vec<_> = gs.simulation.get_graph().node_indices().collect();
        assert!(!node_indices.is_empty());
        let dragging_idx = node_indices[0];

        gs.dragging_node = Some(dragging_idx);
        gs.drag_target = Some((100.0, 200.0));

        simulation_step(&mut gs, 0.12);

        let node = gs.simulation.get_graph().node_weight(dragging_idx).unwrap();
        assert_eq!(node.location.x, 100.0);
        assert_eq!(node.location.y, 200.0);
        assert_eq!(node.velocity, fdg_sim::glam::Vec3::ZERO);
    }
    #[test]
    fn test_continuous_simulation_step() {
        let temp_dir = tempfile::tempdir().unwrap();
        let notes_dir = temp_dir.path().join("notes");
        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(notes_dir.join("a.md"), "[[b]]").unwrap();
        std::fs::write(notes_dir.join("b.md"), "[[a]]").unwrap();

        let storage = crate::storage::Storage {
            data_dir: temp_dir.path().join("data"),
            config_dir,
            notes_dir,
            templates_dir: temp_dir.path().join("templates"),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        std::fs::create_dir_all(&storage.data_dir).unwrap();
        std::fs::create_dir_all(&storage.templates_dir).unwrap();

        let config = crate::config::ClinConfig::default();
        let note_ids = storage.list_note_ids(true, false).unwrap();
        let summaries: Vec<_> = note_ids
            .iter()
            .filter_map(|id| storage.load_note_summary(id).ok())
            .collect();
        let mut gs = GraphState::new(&summaries, &config).expect("GraphState::new");

        gs.alpha = 0.001;
        continuous_simulation_step(&mut gs, 0.12);

        assert!(gs.alpha >= MIN_DYNAMIC_ALPHA);
        assert!(!gs.is_settled);
    }

    #[test]
    fn test_start_physics_thresholds() {
        let config = crate::config::ClinConfig::default();

        let gs_0 = GraphState {
            simulation: fdg_sim::Simulation::from_graph(fdg_sim::ForceGraph::default(), fdg_sim::SimulationParameters::default()),
            viewport: crate::graf::viewport::Viewport::default(),
            selected_node: None,
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            alpha: 0.4,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: parking_lot::Mutex::new(crate::graf::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: crate::graf::spatial::SpatialGrid::new(100.0),
            physics_worker_active: true,
        };
        let state_0 = Arc::new(RwLock::new(gs_0));
        let handle_0 = start_physics(state_0.clone(), &config);
        assert!(handle_0.is_none());
        assert!(!state_0.read().physics_worker_active);
        assert!(state_0.read().is_settled);
        assert_eq!(state_0.read().alpha, 0.0);

        let mut graph = fdg_sim::ForceGraph::default();
        let mut nodes = Vec::new();
        for i in 0..1001 {
            let data = crate::graf::graph::GraphNodeData {
                note_id: format!("{i}"),
                title: format!("Node {i}"),
                tags: vec![],
                link_count: if i < 2 { 1 } else { 0 },
                folder: "".to_string(),
            };
            let idx = graph.add_force_node(format!("Node {i}"), data);
            nodes.push(idx);
        }
        graph.add_edge(nodes[0], nodes[1], ());

        let gs_1001 = GraphState {
            simulation: fdg_sim::Simulation::from_graph(graph, fdg_sim::SimulationParameters::default()),
            viewport: crate::graf::viewport::Viewport::default(),
            selected_node: None,
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            alpha: 0.4,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: parking_lot::Mutex::new(crate::graf::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: crate::graf::spatial::SpatialGrid::new(80.0),
            physics_worker_active: true,
        };
        let state_1001 = Arc::new(RwLock::new(gs_1001));
        let handle_1001 = start_physics(state_1001.clone(), &config);
        assert!(handle_1001.is_none());
        assert!(!state_1001.read().physics_worker_active);
        assert!(state_1001.read().is_settled);
        assert_eq!(state_1001.read().alpha, 0.0);

        let state_read = state_1001.read();
        let g = state_read.simulation.get_graph();
        let loc_0 = g[nodes[0]].location;
        let loc_1 = g[nodes[1]].location;
        let dist = (loc_0.x - loc_1.x).hypot(loc_0.y - loc_1.y);
        assert!((dist - 80.0).abs() < 1e-4f32);

        for &idx in &nodes {
            let node = &g[idx];
            let mut found = false;
            state_read.spatial_grid.for_each_near(
                node.location.x as f64,
                node.location.y as f64,
                1.0,
                |n_idx| {
                    if n_idx == idx {
                        found = true;
                    }
                }
            );
            assert!(found, "Node {:?} not found in spatial grid near location {:?}", idx, node.location);
        }

        let mut graph = fdg_sim::ForceGraph::default();
        let data = crate::graf::graph::GraphNodeData {
            note_id: "1".to_string(),
            title: "Node 1".to_string(),
            tags: vec![],
            link_count: 0,
            folder: "".to_string(),
        };
        graph.add_force_node("Node 1", data);
        let gs_1 = GraphState {
            simulation: fdg_sim::Simulation::from_graph(graph, fdg_sim::SimulationParameters::default()),
            viewport: crate::graf::viewport::Viewport::default(),
            selected_node: None,
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            alpha: 0.4,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: parking_lot::Mutex::new(crate::graf::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: crate::graf::spatial::SpatialGrid::new(100.0),
            physics_worker_active: false,
        };
        let state_1 = Arc::new(RwLock::new(gs_1));
        let handle_1 = start_physics(state_1.clone(), &config);
        assert!(handle_1.is_some());
        assert!(state_1.read().physics_worker_active);
    }

    #[test]
    fn test_static_node_drag_regression() {
        let temp_dir = tempfile::tempdir().unwrap();
        let notes_dir = temp_dir.path().join("notes");
        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(notes_dir.join("a.md"), "[[b]]").unwrap();
        std::fs::write(notes_dir.join("b.md"), "[[a]]").unwrap();

        let storage = crate::storage::Storage {
            data_dir: temp_dir.path().join("data"),
            config_dir,
            notes_dir,
            templates_dir: temp_dir.path().join("templates"),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        std::fs::create_dir_all(&storage.data_dir).unwrap();
        std::fs::create_dir_all(&storage.templates_dir).unwrap();

        let config = crate::config::ClinConfig::default();
        let note_ids = storage.list_note_ids(true, false).unwrap();
        let summaries: Vec<_> = note_ids
            .iter()
            .filter_map(|id| storage.load_note_summary(id).ok())
            .collect();
        let mut gs = GraphState::new(&summaries, &config).expect("GraphState::new");

        gs.physics_worker_active = false;
        gs.is_settled = true;
        gs.alpha = 0.0;

        let node_indices: Vec<_> = gs.simulation.get_graph().node_indices().collect();
        assert!(!node_indices.is_empty());
        let dragging_idx = node_indices[0];

        gs.dragging_node = Some(dragging_idx);
        let wx = 500.0;
        let wy = 600.0;

        {
            let graph = gs.simulation.get_graph_mut();
            if let Some(node) = graph.node_weight_mut(dragging_idx) {
                node.location.x = wx as f32;
                node.location.y = wy as f32;
                node.velocity = fdg_sim::glam::Vec3::ZERO;
            }
            gs.alpha = 0.0;
            gs.is_settled = true;
            let graph = gs.simulation.get_graph();
            gs.graph_bounds = super::super::render::compute_graph_bounds(graph);
            gs.spatial_grid.rebuild(graph);
            gs.render_cache.lock().minimap_dirty = true;
        }

        let node = gs.simulation.get_graph().node_weight(dragging_idx).unwrap();
        assert_eq!(node.location.x, 500.0);
        assert_eq!(node.location.y, 600.0);
        assert_eq!(gs.alpha, 0.0);
        assert!(gs.is_settled);
        assert!(gs.render_cache.lock().minimap_dirty);
    }
}
