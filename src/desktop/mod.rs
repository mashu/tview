//! Desktop (egui) frontend — presentation only; all I/O goes through [`crate::backend`].

pub mod app;
pub mod theme;
pub mod ui;

pub use app::DesktopApp;
