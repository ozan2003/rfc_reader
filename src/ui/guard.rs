//! Terminal state management.
//!
//! Ensures the terminal is properly initialized for the application and
//! restored to its original state when the program exits.
//!
//! It also sets up a panic hook to handle panics gracefully and the terminal
//! backend for the terminal UI.
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

/// Manage terminal state with RAII
///
/// Responsible for restoring the terminal to its original state when the
/// program exits.
pub struct TerminalGuard;

impl TerminalGuard
{
    /// Create a new `TerminalGuard`
    ///
    /// This does the standard terminal setup:
    /// - Enters raw mode
    /// - Enters alternate screen
    ///
    /// # Returns
    ///
    /// Returns a new `TerminalGuard` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal fails to enter raw mode or leave
    /// alternate screen.
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
    /// Drop the `TerminalGuard`
    ///
    /// Restores the terminal to a normal state.
    ///
    /// This does the following:
    /// - Exits raw mode
    /// - Switches back to the main screen
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
/// Returns an error if the terminal fails to initialize.
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
