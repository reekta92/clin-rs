//! Terminal-native Obsidian Bases documents, queries, and interactions.

mod authoring;
mod dates;
pub mod edits;
pub(crate) mod embedding;
pub mod eval;
mod export;
pub mod expr;
pub mod format;
mod groups;
mod input;
mod layout_options;
mod media;
pub mod query;
mod regexp;
mod render;
mod search;
pub mod state;
pub mod value;
pub mod worker;
mod write;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod workflow_tests;
