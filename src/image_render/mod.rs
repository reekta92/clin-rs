pub mod cache;
pub mod worker;

use std::path::PathBuf;

/// Identity for a decoded image: resolved absolute path + mtime (bytes-identical → reuse).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageKey {
    pub path: PathBuf,
    pub mtime: u64,
}
