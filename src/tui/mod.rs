//! TUI (Terminal User Interface) module for cloud-speed.
//!
//! This module provides real-time visual feedback during speed tests,
//! including progress indicators, live measurements, and animated
//! visualizations.

pub mod controller;
pub mod display_mode;
pub mod progress;
pub mod renderer;
pub mod state;

pub use controller::TuiController;
pub use display_mode::DisplayMode;
pub use progress::{BandwidthDirection, ProgressCallback, ProgressEvent, TestPhase};
pub use state::TuiState;
