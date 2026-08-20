pub mod cache;
pub mod worker;

use std::path::PathBuf;

/// Identity for a decoded image: resolved absolute path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageKey {
    pub path: PathBuf,
}

/// Duration (150 ms) to wait after the last zoom/scroll event before
/// considering the view "settled". During a zoom burst, images are suppressed
/// and render as cheap placeholders; once `TRANSFORM_SETTLE` has elapsed with
/// no new zoom events, the real pixel image resumes rendering.
pub const TRANSFORM_SETTLE: std::time::Duration = std::time::Duration::from_millis(150);
