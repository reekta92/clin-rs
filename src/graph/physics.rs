use std::sync::{Arc, RwLock, mpsc};

use super::GraphState;

pub fn start_physics(state: Arc<RwLock<GraphState>>, kill_rx: mpsc::Receiver<()>) {
    std::thread::spawn(move || {
        loop {
            if kill_rx.try_recv().is_ok() {
                break;
            }

            let should_update = {
                let guard = state.read().unwrap_or_else(|e| e.into_inner());
                !guard.is_settled
            };

            if should_update {
                let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                guard.simulation.update(0.016);

                let graph = guard.simulation.get_graph();
                let energy: f32 = graph.node_weights().map(|n| n.velocity.length()).sum();

                if energy < 0.05 * graph.node_count() as f32 {
                    guard.is_settled = true;
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    });
}
