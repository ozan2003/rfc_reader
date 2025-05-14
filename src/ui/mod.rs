//! User Interface module for the RFC Reader application.
//!
//! Contains components for rendering and managing the terminal UI,
//! including event handling, application state, and UI components.
mod app;
mod event;
mod toc_panel;
mod guard;

pub use app::{App, AppMode};
pub use event::{Event, EventHandler};
pub use guard::{TerminalGuard, init_panic_hook, init_tui};