//! A library for reading RFC documents in the terminal.
//!
//! Provides functionality for fetching, caching, and rendering RFCs with a
//! rich terminal user interface.
//!
//! # Features
//!
//! - Fetching RFCs from the official RFC Editor website.
//! - Local caching of RFCs to minimize network requests.
//! - Terminal UI with navigation, search, and table of contents.
//!
//! # Modules
//!
//! - `client`: HTTP client for remote RFC fetching.
//! - `cache`: Local storage for performance improvement.
//! - `ui`: Terminal user interface components and event handling.
pub mod cache;
pub mod client;
pub mod ui;

pub use ui::logging;
