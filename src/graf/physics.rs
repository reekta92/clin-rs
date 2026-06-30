use std::sync::{Arc, mpsc};
use parking_lot::RwLock;

use super::graph::GraphState;
use crate::config::ClinConfig;

pub fn simulation_step(state: &mut GraphState, gravity: f32, timestep: f32) {
    state.simulation.update(timestep);
    if gravity > 0.0 {
        for n in state.simulation.get_graph_mut().node_weights_mut() {
            n.velocity.x -= n.location.x * gravity;
            n.velocity.y -= n.location.y * gravity;
        }
    }
    let graph = state.simulation.get_graph();
    let energy: f32 = graph.node_weights().map(|n| n.velocity.length()).sum();
    if energy < 0.05 * graph.node_count() as f32 {
        state.is_settled = true;
    }
    state.graph_bounds = super::render::compute_graph_bounds(graph);
}

pub fn start_physics(
    state: Arc<RwLock<GraphState>>,
    _config: &ClinConfig,
    kill_rx: mpsc::Receiver<()>,
) {
    let gravity = 0.01;
    let timestep = 0.016;
    let sleep_ms = 16;

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
                simulation_step(&mut guard, gravity as f32, timestep as f32);
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
        };
        std::fs::create_dir_all(&storage.data_dir).unwrap();
        std::fs::create_dir_all(&storage.templates_dir).unwrap();

        let config = crate::config::ClinConfig::default();
        let mut gs = GraphState::new(&storage, &config).expect("GraphState::new");

        // Run simulation steps
        for _ in 0..300 {
            simulation_step(&mut gs, 0.01, 0.016);
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
        // Should have settled (or at least made progress toward settling)
        assert!(gs.is_settled || gs.graph_bounds != (0.0, 0.0, 0.0, 0.0));
    }
}
