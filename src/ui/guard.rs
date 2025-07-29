//! Provides a RAII guard for safe terminal lifecycle management.
//!
//! This module uses the RAII (Resource Acquisition Is Initialization) pattern
//! to manage the terminal state.
//!
//! A guard object is created to initialize the TUI,
//! and its `Drop` implementation automatically restores the terminal when it
//! goes out of scope, either on normal exit or during a panic unwind.
use std::io::{Result as IoResult, stdout};
use std::panic::{set_hook, take_hook};

use crossterm::ExecutableCommand;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use log::error;
use ratatui::Terminal;
use ratatui::backend::{Backend as RatatuiBackend, CrosstermBackend};

/// RAII wrapper for terminal state.
///
/// Manages the terminal's configuration, ensuring it is always returned
/// to its original state when this struct is dropped.
pub struct TerminalGuard;

impl TerminalGuard
{
    /// Creates a `TerminalGuard` for TUI setup.
    ///
    /// Configures the terminal by entering raw mode and switching to the
    /// alternate screen buffer.
    ///
    /// # Returns
    ///
    /// The `TerminalGuard`. Holding this instance guarantees terminal
    /// restoration upon its drop.
    ///
    /// # Errors
    ///
    /// On failure to enter raw mode or switch screens.
    pub fn new() -> IoResult<Self>
    {
        // Setup terminal
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard
{
    /// Restores the terminal state.
    ///
    /// Automatically called on `TerminalGuard` drop.
    ///
    /// Exits raw mode and
    /// returns to the main screen, ensuring a clean terminal state.
    fn drop(&mut self)
    {
        // Terminal will be borked when failure, at least inform the user
        if let Err(err) = disable_raw_mode()
        {
            error!("Failed to disable raw mode: {err}");
        }

        if let Err(err) = stdout().execute(LeaveAlternateScreen)
        {
            error!("Failed to leave alternate screen: {err}");
        }
    }
}

/// Initialize the terminal
///
/// This creates a new terminal and returns it.
///
/// # Returns
///
/// Returns the terminal.
/// 
/// # Errors
/// 
/// Returns an error if the terminal fails to enter raw mode or leave
/// alternate screen.
pub fn init_tui() -> IoResult<Terminal<impl RatatuiBackend>>
{
    // Terminal setup is now handled by TerminalGuard
    // We just create and return the terminal
    let backend = CrosstermBackend::new(stdout());
    Terminal::new(backend)
}

/// Initialize the panic hook to handle panics
///
/// # Panics
///
/// This will panic if the terminal fails to enter raw mode or leave alternate
/// screen.
pub fn init_panic_hook()
{
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // Restore terminal to normal state without panicking
        disable_raw_mode().unwrap();
        stdout()
            .execute(LeaveAlternateScreen)
            .unwrap();

        error!("Application panicked: {panic_info}");

        // Call the original panic hook
        original_hook(panic_info);
    }));
}
