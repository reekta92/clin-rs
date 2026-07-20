use parking_lot::RwLock;
use std::sync::{Arc, mpsc};

use super::graph::GraphState;
use crate::config::{ClinConfig, PhysicsTickRate};

pub fn simulation_step(state: &mut GraphState, timestep: f32) {
    if state.alpha < 0.005 {
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
    let energy: f32 = graph.node_weights().map(|n| n.velocity.length()).sum();
    if energy < 0.05 * graph.node_count() as f32 || alpha < 0.005 {
        state.is_settled = true;
    }

    state.graph_bounds = super::render::compute_graph_bounds(graph);
    state.spatial_grid.rebuild(state.simulation.get_graph());

    // Temperature decays towards 0 (mathematical energy minimum)
    state.alpha *= 0.95;
}

pub fn start_physics(
    state: Arc<RwLock<GraphState>>,
    config: &ClinConfig,
    kill_rx: mpsc::Receiver<()>,
) {
    let timestep = 0.12;

    // Compute tick rate based on config mode and node count
    let tick_rate_mode = config.graf.physics.tick_rate;
    let node_count = { state.read().simulation.get_graph().node_count() };
    let sleep_ms: u64 = if tick_rate_mode == PhysicsTickRate::Fixed {
        16
    } else {
        match node_count {
            0..=500 => 16,      // ~60Hz
            501..=2000 => 33,   // ~30Hz
            _ => 66,            // ~15Hz
        }
    };

    std::thread::spawn(move || {
        loop {
            match kill_rx.try_recv() {
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => {}
            }

            let should_update = {
                let guard = state.read();
                !guard.is_settled
            };

            if should_update {
                let mut guard = state.write();
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
                simulation_step(&mut guard, timestep as f32);
            } else {
                std::thread::sleep(std::time::Duration::from_millis(sleep_ms * 6));
                continue;
            }

            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
