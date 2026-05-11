mod manager;
mod model;
mod store;
mod variables;

// Public re-exports — preserves the external API used by app, storage, popups, etc.
pub use manager::{TemplateManager, TemplateSummary};
#[allow(unused_imports)]
pub use model::{ContentConfig, RenderedTemplate, Template, TitleConfig};
#[allow(unused_imports)]
pub use store::sanitize_filename;
#[allow(unused_imports)]
pub use variables::TemplateVariables;
