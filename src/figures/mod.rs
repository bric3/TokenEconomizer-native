//! Regenerate the pinned documentation figures using native layout and renderer SDKs.
mod framework;
mod renderer;
mod results;
pub mod teaser;

pub use framework::render as framework;
pub use results::render as results;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
