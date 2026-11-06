use std::time::{Duration, Instant};

const FRAME_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 60);

pub(crate) struct FramePacer {
    next_frame: Instant,
}

impl FramePacer {
    pub(crate) fn new() -> Self {
        Self {
            next_frame: Instant::now(),
        }
    }

    /// Bound input-driven work too: event polling can return without waiting.
    pub(crate) fn wait(&mut self) {
        let delay = self.next_frame.saturating_duration_since(Instant::now());
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
        self.next_frame = Instant::now() + FRAME_INTERVAL;
    }
}
