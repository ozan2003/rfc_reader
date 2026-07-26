//! Provides a RAII guard for safe terminal lifecycle management.
//!
//! This module uses the RAII (Resource Acquisition Is Initialization)
//! pattern to manage the terminal state.
//!
//! A guard object is created to initialize the TUI,
//! and its `Drop` implementation automatically restores the terminal when it
//! goes out of scope, either on normal exit or during a panic unwind.
use std::io::stdout;
use std::panic::{set_hook, take_hook};

use anyhow::Result;
use crossterm::ExecutableCommand as _;
use crossterm::cursor::{SetCursorStyle, Show};
use crossterm::terminal::{
    EnterAlternateScreen,
    LeaveAlternateScreen,
    disable_raw_mode,
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
    /// Sets up the terminal and creates the TUI terminal.
    ///
    /// Configures the terminal by entering raw mode and switching to the
    /// alternate screen buffer, and creates a [`Terminal`] to draw with.
    ///
    /// # Returns
    ///
    /// The `TerminalGuard` and the `Terminal`. Holding the guard
    /// guarantees terminal restoration upon its drop.
    ///
    /// # Errors
    ///
    /// On failure to initialize the terminal, enter raw mode, or switch
    /// screens.
    pub fn init()
    -> Result<(Self, Terminal<impl RatatuiBackend<Error = std::io::Error>>)>
    {
        // Create the terminal first
        let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

        // Setup terminal and cursor
        enable_raw_mode()?;

        // If a later step fails, undo raw mode so the terminal isn't
        // left broken with no guard alive to restore it.
        if let Err(err) = stdout().execute(SetCursorStyle::BlinkingBar)
        {
            let _ = disable_raw_mode();
            return Err(err.into());
        }

        if let Err(err) = stdout().execute(EnterAlternateScreen)
        {
            let _ = disable_raw_mode();
            return Err(err.into());
        }

        Ok((Self, terminal))
    }
}

impl Drop for TerminalGuard
{
    /// Restores the terminal state.
    ///
    /// Automatically called on `TerminalGuard` drop.
    fn drop(&mut self)
    {
        restore_terminal();
    }
}

/// Restore the terminal to its original state.
///
/// Attempts every step even if earlier ones fail. Errors are logged,
/// never propagated. Safe to call multiple times.
fn restore_terminal()
{
    // Restore the cursor to visible and default style
    if let Err(err) = stdout().execute(Show)
    {
        error!("Failed to show cursor: {err}");
    }

    if let Err(err) = stdout().execute(SetCursorStyle::DefaultUserShape)
    {
        error!("Failed to reset cursor style: {err}");
    }

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

/// Initialize the panic hook to handle panics.
///
/// The installed hook restores the terminal to its original state and
/// logs the panic before chaining to the original hook.
pub fn init_panic_hook()
{
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // Never panic here: panicking inside a panic hook aborts the
        // process immediately.
        restore_terminal();

        error!("Application panicked: {panic_info}");

        // Call the original panic hook
        original_hook(panic_info);
    }));
}
