//! RFC Reader Library
//!
//! A library for fetching, caching, and displaying RFC documents in a terminal
//! interface.
//!
//! # Features
//!
//! - Fetching RFCs from the official RFC Editor website
//! - Local caching of RFC documents to reduce network requests
//! - Terminal-based user interface with navigation and search capabilities
//! - Table of contents generation and navigation
//!
//! # Modules
//!
//! - `client`: HTTP client for fetching RFCs from remote sources
//! - `cache`: Local storage for RFCs to improve performance
//! - `ui`: Terminal user interface components and event handling
pub mod cache;
pub mod client;
pub mod ui;

pub use ui::logging;
