pub mod adapters;
pub mod aggregate;
pub mod arms;
mod assets;
pub mod bundle;
pub mod codex_spiral;
pub mod datasets;
pub mod driver;
pub mod facade;
pub mod net_guard;
pub mod nudge_guard;
mod paths;
pub mod scaffolds;
pub mod sdk;
pub mod spiral_stage;
pub mod swebench;
pub mod sweep;
pub mod usage;
pub mod workloads;

/// The imported API distinguishes argument parser exits from main() return values.
#[derive(Default, serde::Serialize)]
pub struct CommandOutput {
    pub exit: Option<i32>,
    pub returncode: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}
