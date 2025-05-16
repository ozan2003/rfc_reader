use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};
#[allow(clippy::wildcard_imports)]
use cli_log::*;
use crossterm::event::KeyCode;
use ratatui::Terminal;
use ratatui::backend::Backend as RatatuiBackend;
use rfc_reader::{App, AppMode, Event, EventHandler, RfcCache, RfcClient};
use rfc_reader::{TerminalGuard, init_panic_hook, init_tui};
use std::time::Duration;

fn main() -> Result<()>
{
    init_panic_hook();

    // Initialize cache
    let cache = RfcCache::new()?;

    // Parse command line arguments
    let matches = Command::new("rfc_reader")
        .about("A terminal-based RFC reader")
        // Inform about the cache directory
        .after_help(format!(
            "This program caches RFCs to improve performance.\nThe cache is stored in the \
             following directory: {}",
            cache.cache_dir().display()
        ))
        .arg(
            Arg::new("rfc")
                .help("RFC number to open")
                .value_name("NUMBER")
                .index(1),
        )
        .arg(
            Arg::new("clear-cache")
                .long("clear-cache")
                .help("Clear the RFC cache")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("offline")
                .long("offline")
                .short('o')
                .help("Run in offline mode (only load cached RFCs)")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    // Clear cache if requested
    if matches.get_flag("clear-cache")
    {
        // Clear all cached RFCs
        cache.clear()?;
        info!("Cache cleared successfully");
        return Ok(());
    }

    // Setup client
    let client = RfcClient::new();

    // Use RAII to ensure terminal cleanup happens
    let _terminal_guard = TerminalGuard::new()?;

    // Setup terminal - this now returns just the terminal
    let mut terminal = init_tui()?;

    // Get RFC if specified
    let rfc_number = matches
        .get_one::<String>("rfc")
        .and_then(|s| s.parse::<u16>().ok());

    // Placeholder for getting RFC content
    let (rfc_number, rfc_content) = if let Some(number) = rfc_number
    {
        // Get the RFC content - first check cache, then fetch from network if needed
        let content = if let Some(cached_content) = cache.get_cached_rfc(number)
        {
            info!("Using cached version of RFC {number}");
            cached_content
        }
        else
        {
            let is_offline = matches.get_flag("offline");
            if is_offline
            {
                error!("Cannot load RFC {number} - not in cache and offline mode is enabled");
                return Err(anyhow::anyhow!(
                    "Cannot load RFC {number} - not in cache and offline mode is enabled"
                ));
            }
            // Fetch RFC from network since it's not in cache
            info!("Fetching RFC {number} from network...");

            let content = client
                .fetch_rfc(number)
                .context("Failed to fetch RFC {number}")?;

            // Cache the fetched content for future use.
            cache
                .cache_rfc(number, &content)
                .context("Could not cache RFC {number}")?;
            content
        };
        (number, content)
    }
    else
    {
        error!("No RFC number provided");
        return Err(anyhow::anyhow!("No RFC number provided"));
    };

    // Create app state
    let app = App::new(rfc_number, rfc_content);

    // Create event handler with 250ms tick rate
    let event_handler = EventHandler::new(Duration::from_millis(250));

    // Terminal will be cleaned up automatically when _terminal_guard is dropped
    // Just propagate any error from run_app
    run_app(&mut terminal, app, &event_handler)
}

/// Run the main loop
///
/// # Arguments
///
/// * `terminal` - The terminal to draw to
/// * `app` - The app to run
/// * `event_handler` - The event handler to handle events
///
/// # Errors
///
/// Returns an error if the terminal fails to draw to the screen.
fn run_app<T: RatatuiBackend>(
    terminal: &mut Terminal<T>,
    mut app: App,
    event_handler: &EventHandler,
) -> Result<()>
{
    loop
    {
        terminal.draw(|frame| app.render(frame))?;

        if let Event::Key(key) = event_handler.next()?
        {
            match (app.mode, key.code)
            {
                // Quit with 'q' in normal mode
                (AppMode::Normal, KeyCode::Char('q')) =>
                {
                    app.should_quit = true;
                }

                // Help toggle with '?'
                (AppMode::Normal | AppMode::Help, KeyCode::Char('?')) |
                (AppMode::Help, KeyCode::Esc) =>
                {
                    app.toggle_help();
                }
                // Table of contents toggle with 't'
                (AppMode::Normal, KeyCode::Char('t')) =>
                {
                    app.toggle_toc();
                }

                // Navigation in normal mode
                (AppMode::Normal, KeyCode::Char('j') | KeyCode::Down) =>
                {
                    app.scroll_down(1);
                }
                (AppMode::Normal, KeyCode::Char('k') | KeyCode::Up) =>
                {
                    app.scroll_up(1);
                }
                // 2 for borders
                (AppMode::Normal, KeyCode::Char('f') | KeyCode::PageDown) =>
                {
                    app.scroll_down(
                        terminal
                            .size()?
                            .height
                            .saturating_sub(2)
                            .into(),
                    );
                }
                (AppMode::Normal, KeyCode::Char('b') | KeyCode::PageUp) =>
                {
                    app.scroll_up(
                        terminal
                            .size()?
                            .height
                            .saturating_add(2)
                            .into(),
                    );
                }

                (AppMode::Normal, KeyCode::Char('g')) =>
                {
                    app.scroll_up(app.rfc_content.len());
                }
                (AppMode::Normal, KeyCode::Char('G')) =>
                {
                    app.scroll_down(app.rfc_content.len());
                }

                // Search handling
                (AppMode::Normal, KeyCode::Char('/')) =>
                {
                    app.enter_search_mode();
                }
                (AppMode::Search, KeyCode::Enter) =>
                {
                    app.perform_search();
                    app.exit_search_mode();
                }
                (AppMode::Search, KeyCode::Esc) =>
                {
                    app.exit_search_mode();
                }
                (AppMode::Search, KeyCode::Backspace) =>
                {
                    app.remove_search_char();
                }
                (AppMode::Search, KeyCode::Char(ch)) =>
                {
                    app.add_search_char(ch);
                }

                // Search result navigation
                (AppMode::Normal, KeyCode::Char('n')) =>
                {
                    app.next_search_result();
                }
                (AppMode::Normal, KeyCode::Char('N')) =>
                {
                    app.prev_search_result();
                }
                (AppMode::Normal, KeyCode::Esc) =>
                {
                    app.reset_search_highlights();
                }

                _ =>
                {} // Ignore other key combinations
            }
        }

        if app.should_quit
        {
            break;
        }
    }

    Ok(())
}
