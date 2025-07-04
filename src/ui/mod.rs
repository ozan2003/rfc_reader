//! User Interface module for the RFC Reader application.
//!
//! Contains components for rendering and managing the terminal UI,
//! including event handling, application state, and UI components.
mod app;
mod event;
pub mod guard;
pub mod logging;
mod toc_panel;

pub use app::{App, AppMode, AppStateFlags};
pub use event::{Event, EventHandler};