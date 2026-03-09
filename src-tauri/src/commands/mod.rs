pub mod actions;
pub mod activity;
pub mod analysis;
pub mod audio;
pub mod intent;
pub mod settings;
pub mod workflow;

// Re-export non-command functions used by lib.rs
pub use analysis::do_analyze;
pub use settings::{get_auto_analyze, get_cooldown_secs};
