//! Terminal state management.
//!
//! Ensures the terminal is properly initialized for the application and
//! restored to its original state when the program exits.
#[allow(clippy::wildcard_imports)]
use cli_log::*;
use crossterm::ExecutableCommand;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::{Backend as RatatuiBackend, CrosstermBackend};
use std::io::{Result as IoResult, stdout};
use std::panic::{set_hook, take_hook};

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
    /// This restores the terminal to a normal state by taking advantage of
    /// RAII.
    ///
    /// This does the following:
    /// - Exits raw mode
    /// - Exits alternate screen
    ///
    /// This is performed even if the program panics or returns early
    fn drop(&mut self)
    {
        disable_raw_mode().unwrap();
        stdout()
            .execute(LeaveAlternateScreen)
            .unwrap();
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
/// This will panic if the terminal fails to enter raw mode or leave alternate screen.
pub fn init_panic_hook()
{
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // Restore terminal to normal state without panicking
        disable_raw_mode().unwrap();
        stdout()
            .execute(LeaveAlternateScreen)
            .unwrap();

        // Log the panic info
        error!("Application panicked: {panic_info}");

        // Call the original panic hook
        original_hook(panic_info);
    }));
}
