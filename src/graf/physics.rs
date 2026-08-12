use parking_lot::RwLock;
use std::sync::{Arc, mpsc};

use super::graph::GraphState;
use crate::config::{ClinConfig, PhysicsTickRate};

pub(crate) const MAX_DYNAMIC_NODES: usize = 1_000;
const MIN_DYNAMIC_ALPHA: f32 = 0.005;
/// Per-tick node velocity cap, as a multiple of `ideal_distance`. Stops force
/// explosions (overlapping-node repulsion) from launching nodes off-screen.
/// Scaled by sqrt(node_count) at use so large dynamic graphs aren't choked.
const VELOCITY_FACTOR: f32 = 25.0;
/// Per-tick node displacement cap, as a multiple of `ideal_distance`. fdg-sim
/// integrates location inside `update()`, before our velocity cap runs, so a
/// force explosion (flung high-degree node: attraction = k·d²/scale) can move a
/// node thousands of world units in one tick. Clamping displacement catches the
/// jump the velocity cap misses. Scaled by sqrt(node_count) like VELOCITY_FACTOR.
const DISPLACEMENT_FACTOR: f32 = 0.5;
/// World-position clamp radius, as a multiple of `ideal_distance * sqrt(node_count)`.
/// Tuned to comfortably contain a settled Fruchterman-Reingold layout (~1.7x its
/// natural spread) so it never compresses a real graph, while catching nodes flung
/// beyond the cluster. Bounds `graph_bounds` so auto-fit never shrinks to a dot.
const SPREAD_FACTOR: f64 = 2.5;

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

    let node_count = state.simulation.get_graph().node_count();
    let ideal = state.physics_ideal_distance as f32;
    let max_velocity = ideal * VELOCITY_FACTOR * (node_count.max(1) as f32).sqrt();
    let max_displacement = ideal * DISPLACEMENT_FACTOR * (node_count.max(1) as f32).sqrt();
    let world_clamp_radius =
        state.physics_ideal_distance * (node_count as f64).max(4.0).sqrt() * SPREAD_FACTOR;
    let mut need_reheat = false;
    for idx in state
        .simulation
        .get_graph()
        .node_indices()
        .collect::<Vec<_>>()
    {
        let node = &mut state.simulation.get_graph_mut()[idx];
        node.velocity *= cooloff;

        // Non-finite hygiene MUST run for every node including the dragged one:
        // a NaN node skipped here survives into the next update(), where handy's
        // repulsion loop reads its NaN old_location for every other node and
        // cascades NaN across the whole graph.
        if !node.location.x.is_finite()
            || !node.location.y.is_finite()
            || !node.velocity.x.is_finite()
            || !node.velocity.y.is_finite()
        {
            // Jittered reset: distinct deterministic position per node index.
            // Resetting every NaN node to the SAME point (Vec3::ZERO) makes them
            // coincident, and fdg-sim repulsion divides by the inter-node
            // distance — coincident nodes NaN again every tick and lock up.
            // Golden-angle scatter guarantees distinct positions.
            let angle = idx.index() as f32 * 2.399_963; // golden angle, radians
            let r = ideal * 0.5;
            node.location = fdg_sim::glam::Vec3::new(r * angle.cos(), r * angle.sin(), 0.0);
            node.old_location = node.location;
            node.velocity = fdg_sim::glam::Vec3::ZERO;
            need_reheat = true;
        }

        if Some(idx) == state.dragging_node {
            continue; // caps only; position driven by drag_target below
        }

        // Cap velocity magnitude: stops a force explosion (overlapping-node
        // repulsion) from launching this node off-screen over subsequent ticks.
        let speed = node.velocity.length();
        if speed.is_finite() && speed > 0.0 && speed > max_velocity {
            node.velocity *= max_velocity / speed;
        }
        // Cap per-tick displacement: fdg-sim set old_location = location at the
        // start of this tick, so this delta is exactly what update() just added.
        // Without this, one tick of k·d²/scale attraction teleports a flung
        // heavy node to the world-clamp corner and its edges flicker across the
        // screen until alpha decays.
        let delta = node.location - node.old_location;
        let dist = delta.length();
        if dist.is_finite() && dist > max_displacement && dist > 0.0 {
            node.location = node.old_location + delta * (max_displacement / dist);
        }
        // Bound the node to a world box around the origin. The `centering` force
        // (enabled in fdg_sim::force::handy) subtracts the centroid every tick,
        // so the cluster sits at the origin post-update — this box is effectively
        // around the cluster. Strays snap to the edge and self-heal inward once the
        // drag force relaxes; they can never inflate graph_bounds past this radius.
        let radius = world_clamp_radius as f32;
        if node.location.x.is_finite() {
            node.location.x = node.location.x.clamp(-radius, radius);
        }
        if node.location.y.is_finite() {
            node.location.y = node.location.y.clamp(-radius, radius);
        }
    }
    if need_reheat {
        state.reheat(0.4);
    }

    if let Some((tx, ty)) = state.drag_target
        && tx.is_finite()
        && ty.is_finite()
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
                    && tx.is_finite()
                    && ty.is_finite()
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
    fn test_simulation_step_caps_displacement() {
        let temp_dir = tempfile::tempdir().unwrap();
        let notes_dir = temp_dir.path().join("notes");
        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();

        // 3 connected nodes so the graph has structure
        std::fs::write(notes_dir.join("a.md"), "[[b]]\n[[c]]").unwrap();
        std::fs::write(notes_dir.join("b.md"), "[[a]]\n[[c]]").unwrap();
        std::fs::write(notes_dir.join("c.md"), "[[a]]\n[[b]]").unwrap();

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
        gs.alpha = 1.0;

        let node_indices: Vec<_> = gs.simulation.get_graph().node_indices().collect();
        assert!(node_indices.len() >= 3);

        // Teleport one node far away with zero velocity
        let teleport_idx = node_indices[0];
        {
            let n = &mut gs.simulation.get_graph_mut()[teleport_idx];
            n.location = fdg_sim::glam::Vec3::new(1e5, 0.0, 0.0);
            n.velocity = fdg_sim::glam::Vec3::ZERO;
        }

        simulation_step(&mut gs, 0.12);

        // The displacement clamp limits per-tick movement; the world clamp then
        // bounds the final position. Together they prevent teleports. Verify
        // the teleported node ended up within the world-clamp radius.
        let radius = gs.physics_ideal_distance
            * ((gs.simulation.get_graph().node_count() as f64).max(4.0)).sqrt()
            * super::SPREAD_FACTOR;
        let n = gs.simulation.get_graph().node_weight(teleport_idx).unwrap();
        assert!(
            n.location.x.abs() <= radius as f32 + 1e-3,
            "teleported node x {} not within world clamp {}",
            n.location.x,
            radius
        );
        assert!(
            n.location.y.abs() <= radius as f32 + 1e-3,
            "teleported node y {} not within world clamp {}",
            n.location.y,
            radius
        );
    }

    #[test]
    fn test_simulation_step_displacement_clamp_skips_dragged_node() {
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
        gs.alpha = 1.0;

        let node_indices: Vec<_> = gs.simulation.get_graph().node_indices().collect();
        let dragging_idx = node_indices[0];

        gs.dragging_node = Some(dragging_idx);
        gs.drag_target = Some((1e5, 1e5));

        simulation_step(&mut gs, 0.12);

        // Dragged node should be at the exact drag target, not clamped
        let n = gs.simulation.get_graph().node_weight(dragging_idx).unwrap();
        assert_eq!(n.location.x, 1e5);
        assert_eq!(n.location.y, 1e5);
        assert_eq!(n.velocity, fdg_sim::glam::Vec3::ZERO);
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
            simulation: fdg_sim::Simulation::from_graph(
                fdg_sim::ForceGraph::default(),
                fdg_sim::SimulationParameters::default(),
            ),
            viewport: crate::graf::viewport::Viewport::default(),
            selected_node: None,
            selected_nodes: std::collections::HashSet::new(),
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            alpha: 0.4,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: parking_lot::Mutex::new(crate::graf::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: crate::graf::spatial::SpatialGrid::new(100.0),
            physics_worker_active: true,
            physics_ideal_distance: 80.0,
            context_menu: None,
            context_menu_screen: (0, 0),
            connection_source: None,
            deleting_connection_source: None,
            box_select_start: None,
            box_select_curr: None,
            right_down_pos: None,
            mode_banner: None,
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
            simulation: fdg_sim::Simulation::from_graph(
                graph,
                fdg_sim::SimulationParameters::default(),
            ),
            viewport: crate::graf::viewport::Viewport::default(),
            selected_node: None,
            selected_nodes: std::collections::HashSet::new(),
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            alpha: 0.4,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: parking_lot::Mutex::new(crate::graf::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: crate::graf::spatial::SpatialGrid::new(80.0),
            physics_worker_active: true,
            physics_ideal_distance: 80.0,
            context_menu: None,
            context_menu_screen: (0, 0),
            connection_source: None,
            deleting_connection_source: None,
            box_select_start: None,
            box_select_curr: None,
            right_down_pos: None,
            mode_banner: None,
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
                },
            );
            assert!(
                found,
                "Node {:?} not found in spatial grid near location {:?}",
                idx, node.location
            );
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
            simulation: fdg_sim::Simulation::from_graph(
                graph,
                fdg_sim::SimulationParameters::default(),
            ),
            viewport: crate::graf::viewport::Viewport::default(),
            selected_node: None,
            selected_nodes: std::collections::HashSet::new(),
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            alpha: 0.4,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: parking_lot::Mutex::new(crate::graf::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: crate::graf::spatial::SpatialGrid::new(100.0),
            physics_worker_active: false,
            physics_ideal_distance: 80.0,
            context_menu: None,
            context_menu_screen: (0, 0),
            connection_source: None,
            deleting_connection_source: None,
            box_select_start: None,
            box_select_curr: None,
            right_down_pos: None,
            mode_banner: None,
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

    #[test]
    fn test_non_finite_node_is_healed() {
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
        let idx = node_indices[0];

        // Corrupt the node with non-finite location
        let node = gs.simulation.get_graph_mut().node_weight_mut(idx).unwrap();
        node.location = fdg_sim::glam::Vec3::new(f32::INFINITY, f32::INFINITY, 0.0);
        node.velocity = fdg_sim::glam::Vec3::ZERO;

        simulation_step(&mut gs, 0.12);

        let node = gs.simulation.get_graph().node_weight(idx).unwrap();
        assert!(node.location.x.is_finite(), "x should be finite after heal");
        assert!(node.location.y.is_finite(), "y should be finite after heal");
        assert_eq!(node.velocity, fdg_sim::glam::Vec3::ZERO);
    }

    #[test]
    fn test_simulation_step_clamps_runaway_node() {
        // Reuse the 2-note tempdir + Storage + GraphState::new builder pattern.
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
        gs.physics_worker_active = true;
        let node_indices: Vec<_> = gs.simulation.get_graph().node_indices().collect();
        let victim = node_indices[0];

        // Place a node far beyond any real layout span and give it huge velocity,
        // simulating a force explosion from an overlapping drag.
        {
            let n = gs
                .simulation
                .get_graph_mut()
                .node_weight_mut(victim)
                .unwrap();
            n.location = fdg_sim::glam::Vec3::new(1.0e7, 1.0e7, 0.0);
            n.velocity = fdg_sim::glam::Vec3::new(1.0e6, 0.0, 0.0);
        }
        gs.alpha = 0.4; // keep the step running (hot)
        simulation_step(&mut gs, 0.12);

        let n = gs.simulation.get_graph().node_weight(victim).unwrap();
        let radius = gs.physics_ideal_distance
            * ((gs.simulation.get_graph().node_count() as f64).max(4.0)).sqrt()
            * super::SPREAD_FACTOR;
        assert!(
            n.location.x.abs() <= radius as f32 + 1e-3,
            "x {} not clamped to {}",
            n.location.x,
            radius
        );
        assert!(
            n.location.y.abs() <= radius as f32 + 1e-3,
            "y {} not clamped to {}",
            n.location.y,
            radius
        );
        let speed = n.velocity.length();
        let max_v = (gs.physics_ideal_distance as f32)
            * super::VELOCITY_FACTOR
            * (gs.simulation.get_graph().node_count().max(1) as f32).sqrt();
        assert!(
            speed <= max_v + 1e-3,
            "velocity {} not capped to {}",
            speed,
            max_v
        );
    }
    #[test]
    fn test_nan_reset_scatters_coincident_nodes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let notes_dir = temp_dir.path().join("notes");
        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();

        std::fs::write(notes_dir.join("a.md"), "[[b]]\n[[c]]").unwrap();
        std::fs::write(notes_dir.join("b.md"), "[[a]]\n[[c]]").unwrap();
        std::fs::write(notes_dir.join("c.md"), "[[a]]\n[[b]]").unwrap();

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
        gs.alpha = 1.0;

        let node_indices: Vec<_> = gs.simulation.get_graph().node_indices().collect();
        assert!(node_indices.len() >= 3);

        // Set two different nodes to the same NaN value — coincident after reset
        let nan_vec = fdg_sim::glam::Vec3::new(f32::NAN, f32::NAN, 0.0);
        {
            let g = gs.simulation.get_graph_mut();
            g[node_indices[0]].location = nan_vec;
            g[node_indices[1]].location = nan_vec;
        }

        simulation_step(&mut gs, 0.12);

        // Both nodes finite and NOT coincident (golden-angle scatter)
        let g = gs.simulation.get_graph();
        let loc_a = g[node_indices[0]].location;
        let loc_b = g[node_indices[1]].location;
        assert!(
            loc_a.x.is_finite() && loc_a.y.is_finite(),
            "node A NaN after reset"
        );
        assert!(
            loc_b.x.is_finite() && loc_b.y.is_finite(),
            "node B NaN after reset"
        );
        assert!(
            loc_a.distance(loc_b) > 0.0,
            "coincident after reset: {:?} vs {:?}",
            loc_a,
            loc_b
        );

        // Run 5 more steps — no re-NaN ping-pong
        for _ in 0..5 {
            simulation_step(&mut gs, 0.12);
            let g = gs.simulation.get_graph();
            for idx in &node_indices {
                let loc = g[*idx].location;
                assert!(
                    loc.x.is_finite() && loc.y.is_finite(),
                    "node re-NaN'd at {:?}: {:?}",
                    idx,
                    loc
                );
            }
        }
    }

    #[test]
    fn test_dragged_nan_node_is_reset() {
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
        gs.alpha = 1.0;

        let node_indices: Vec<_> = gs.simulation.get_graph().node_indices().collect();
        let a_idx = node_indices[0];

        // Set dragged node to NaN
        gs.simulation.get_graph_mut()[a_idx].location =
            fdg_sim::glam::Vec3::new(f32::NAN, f32::NAN, 0.0);
        gs.dragging_node = Some(a_idx);
        gs.drag_target = None; // no drag position override

        simulation_step(&mut gs, 0.12);

        // Node A must be finite — NaN reset ran despite drag exemption
        let loc = gs.simulation.get_graph()[a_idx].location;
        assert!(
            loc.x.is_finite() && loc.y.is_finite(),
            "dragged NaN node not reset: {:?}",
            loc
        );
    }

    #[test]
    fn test_nan_does_not_cascade_while_dragging() {
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
        gs.alpha = 1.0;

        let node_indices: Vec<_> = gs.simulation.get_graph().node_indices().collect();
        let a_idx = node_indices[0];

        // Set dragged node to NaN — regression: this used to cascade NaN to all nodes
        gs.simulation.get_graph_mut()[a_idx].location =
            fdg_sim::glam::Vec3::new(f32::NAN, f32::NAN, 0.0);
        gs.dragging_node = Some(a_idx);
        gs.drag_target = None;

        // Run 3 steps — every node must stay finite each time
        for step in 0..3 {
            simulation_step(&mut gs, 0.12);
            let g = gs.simulation.get_graph();
            for idx in &node_indices {
                let loc = g[*idx].location;
                assert!(
                    loc.x.is_finite() && loc.y.is_finite(),
                    "NaN cascade at step {} at node {:?}: {:?}",
                    step,
                    idx,
                    loc
                );
            }
        }
    }
}
