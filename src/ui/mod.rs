//! Core UI components for the RFC Reader application.
//!
//! Provides rendering, event handling, and state management for the
//! terminal-based user interface.
mod app;
mod event;
pub mod guard;
pub mod logging;
mod toc_panel;

pub use app::{App, AppMode, AppStateFlags};
pub use event::{Event, EventHandler};
