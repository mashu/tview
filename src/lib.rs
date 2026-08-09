//! tview — CSV/TSV metrics viewer (desktop + headless web).
//!
//! Architecture:
//! - [`backend`] owns data, file watching, load/export I/O on a worker thread
//! - [`desktop`] and [`web`] are thin frontends that send commands and read snapshots

pub mod backend;
pub mod data;
pub mod desktop;
pub mod export;
pub mod series;
pub mod web;

pub use backend::{Backend, Command, Event, PlotOptions, SharedView};
