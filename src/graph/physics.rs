use std::sync::{mpsc, Arc, RwLock};

use super::GraphState;
use crate::config::ClinConfig;

pub fn start_physics(
    state: Arc<RwLock<GraphState>>,
    config: &ClinConfig,
    kill_rx: mpsc::Receiver<()>,
) {
    let gravity = config.physics.gravity;
    let timestep = config.physics.timestep;
    let sleep_ms = config.physics.thread_sleep_ms;

    std::thread::spawn(move || loop {
        if kill_rx.try_recv().is_ok() {
            break;
        }

        let should_update = {
            let guard = state.read().unwrap_or_else(|e| e.into_inner());
            !guard.is_settled
        };

        if should_update {
            
            let new_bounds = {
                let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                guard.simulation.update(timestep as f32);

                
                
                if let Some((tx, ty)) = guard.drag_target
                    && let Some(idx) = guard.dragging_node {
                        let graph = guard.simulation.get_graph_mut();
                        if let Some(node) = graph.node_weight_mut(idx) {
                            node.location.x = tx;
                            node.location.y = ty;
                            node.velocity = fdg_sim::glam::Vec3::ZERO;
                        }
                    }

                if gravity > 0.0 {
                    let graph = guard.simulation.get_graph_mut();
                    for node in graph.node_weights_mut() {
                        node.velocity.x -= node.location.x * gravity as f32;
                        node.velocity.y -= node.location.y * gravity as f32;
                    }
                }

                let graph = guard.simulation.get_graph();
                let energy: f32 = graph.node_weights().map(|n| n.velocity.length()).sum();

                if energy < 0.05 * graph.node_count() as f32 {
                    guard.is_settled = true;
                }

                
                super::render::compute_graph_bounds(guard.simulation.get_graph())
            }; 

            
            {
                let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                guard.graph_bounds = new_bounds;
            }
        } else {
            
            std::thread::sleep(std::time::Duration::from_millis(sleep_ms * 6));
            continue;
        }

        std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
    });
}

