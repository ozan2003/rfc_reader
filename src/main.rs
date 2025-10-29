use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use clap::{ArgAction, ArgGroup, Command, arg, crate_version};
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use log::{debug, error, info};
use ratatui::Terminal;
use ratatui::backend::Backend as RatatuiBackend;
use rfc_reader::cache::RfcCache;
use rfc_reader::client::RfcClient;
use rfc_reader::logging::{clear_log_file, get_log_file_path, init_logging};
use rfc_reader::ui::guard::{init_panic_hook, init_tui};
use rfc_reader::ui::{App, AppMode, AppStateFlags, Event, EventHandler};

fn main() -> Result<()>
{
    init_panic_hook();
    init_logging()?;

    // Initialize cache
    let cache = RfcCache::new()?;

    // Parse command line arguments
    let matches = Command::new("rfc_reader")
        .about("A terminal-based RFC reader")
        .version(crate_version!())
        // Inform about the cache and log directory
        .after_help(format!(
            "This program caches RFCs to improve performance.\nThe cache is \
             stored in the following directory: {}\n\nThe location of the log \
             file is: {}",
            cache.cache_dir().display(),
            get_log_file_path().display()
        ))
        // These args are irrelevant to `rfc`.
        .group(ArgGroup::new("maintenance").args([
            "clear-cache",
            "clear-log",
            "list",
        ]))
        .args([
            arg!([rfc] "RFC number to open")
                .value_name("NUMBER")
                .value_parser(clap::value_parser!(u16))
                .index(1)
                .required_unless_present("maintenance")
                // Disallow giving a NUMBER together with those actions
                .conflicts_with("maintenance"),
            arg!(--"clear-cache" "Clear the RFC cache")
                .action(ArgAction::SetTrue),
            arg!(--"clear-log" "Clear the log file").action(ArgAction::SetTrue),
            arg!(-o --offline "Run in offline mode (only load cached RFCs)")
                .action(ArgAction::SetTrue),
            arg!(-l --list "List all cached RFCs").action(ArgAction::SetTrue),
        ])
        .get_matches();

    // Handle maintenance actions: clear cache, clear log, list cached RFCs
    if matches.get_flag("clear-cache")
    {
        cache.clear()?;
        println!("Cache cleared successfully");
        return Ok(());
    }
    else if matches.get_flag("clear-log")
    {
        clear_log_file()?;
        println!("Log file cleared successfully");
        return Ok(());
    }
    else if matches.get_flag("list")
    {
        // Print the list of all cached RFCs one per line
        cache.print_list();
        return Ok(());
    }

    // Setup client
    let client = RfcClient::default();

    // Get RFC if specified
    let rfc_number = *matches
        .get_one::<u16>("rfc")
        .ok_or(anyhow!("RFC number is required"))?;

    // Get the RFC content - first check cache, then fetch from network if
    // needed
    let rfc_content = if let Ok(cached_content) =
        cache.get_cached_rfc(rfc_number)
    {
        info!("Using cached version of RFC {rfc_number}");
        cached_content
    }
    else
    {
        let is_offline = matches.get_flag("offline");
        if is_offline
        {
            error!(
                "RFC {rfc_number} unavailable: offline mode active and no \
                 cached copy found"
            );

            bail!(
                "Unable to access RFC {rfc_number} - network access disabled \
                 in offline mode and RFC not cached locally"
            );
        }
        // Fetch RFC from network since it's not in cache
        debug!("Fetching RFC {rfc_number} from network...");

        let content = client
            .fetch_rfc(rfc_number)
            .with_context(|| format!("Failed to fetch RFC {rfc_number}"))?;

        // Cache the fetched content for future use.
        cache
            .cache_rfc(rfc_number, &content)
            .with_context(|| format!("Could not cache RFC {rfc_number}"))?;

        debug!("Cached RFC {rfc_number}");
        content
    };

    // Setup necessary components for the app
    let mut terminal = init_tui()?;

    let app = App::new(rfc_number, rfc_content);

    let event_handler = EventHandler::new(Duration::from_millis(200));

    // Just propagate any error from run_app
    #[allow(
        clippy::needless_return,
        reason = "Explicit return for readability; the implicit return might \
                  look accidental."
    )]
    return run_app(&mut terminal, app, &event_handler);
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
#[allow(clippy::too_many_lines, reason = "Keybindings are verbose")]
fn run_app<T: RatatuiBackend>(
    terminal: &mut Terminal<T>,
    mut app: App,
    event_handler: &EventHandler,
) -> Result<()>
{
    while app
        .app_state
        .contains(AppStateFlags::SHOULD_RUN)
    {
        terminal.draw(|frame| app.render(frame))?;

        if let Event::Key(key) = event_handler.next()? &&
        // This is needed in Windows
            key.kind == KeyEventKind::Press
        {
            match (app.mode, key.code)
            {
                // Quit with 'q' in normal mode
                (AppMode::Normal, KeyCode::Char('q')) =>
                {
                    app.app_state
                        .remove(AppStateFlags::SHOULD_RUN);
                },

                // Help toggle with '?'
                (AppMode::Normal | AppMode::Help, KeyCode::Char('?')) |
                (AppMode::Help, KeyCode::Esc) =>
                {
                    app.toggle_help();
                },
                // Table of contents toggle with 't'
                (AppMode::Normal, KeyCode::Char('t')) =>
                {
                    app.toggle_toc();
                },

                // Navigation in normal mode
                (AppMode::Normal, KeyCode::Char('j') | KeyCode::Down) =>
                {
                    app.scroll_down(1);
                },
                (AppMode::Normal, KeyCode::Char('k') | KeyCode::Up) =>
                {
                    app.scroll_up(1);
                },
                // Scroll the whole viewpoint
                (AppMode::Normal, KeyCode::Char('f') | KeyCode::PageDown) =>
                {
                    app.scroll_down(terminal.size()?.height.into());
                },
                (AppMode::Normal, KeyCode::Char('b') | KeyCode::PageUp) =>
                {
                    app.scroll_up(terminal.size()?.height.into());
                },
                // Whole document scroll
                (AppMode::Normal, KeyCode::Char('g')) =>
                {
                    // Use total line count instead of the byte count of the
                    // document
                    app.scroll_up(app.rfc_line_number);
                },
                (AppMode::Normal, KeyCode::Char('G')) =>
                {
                    app.scroll_down(app.rfc_line_number);
                },

                // Search handling
                (AppMode::Normal, KeyCode::Char('/')) =>
                {
                    app.enter_search_mode();
                },
                (AppMode::Search, KeyCode::Enter) =>
                {
                    app.perform_search();
                    app.exit_search_mode();
                },
                (AppMode::Search, KeyCode::Esc) =>
                {
                    app.exit_search_mode();
                },
                (AppMode::Search, KeyCode::Backspace) =>
                {
                    app.remove_search_char();
                },
                (AppMode::Search, KeyCode::Delete) =>
                {
                    app.delete_search_char();
                },
                // Cursor navigation
                (AppMode::Search, KeyCode::Left) =>
                {
                    app.move_search_cursor_left();
                },
                (AppMode::Search, KeyCode::Right) =>
                {
                    app.move_search_cursor_right();
                },
                (AppMode::Search, KeyCode::Home) =>
                {
                    app.move_search_cursor_home();
                },
                (AppMode::Search, KeyCode::End) =>
                {
                    app.move_search_cursor_end();
                },
                // Ctrl + c toggles case sensitive mode
                (AppMode::Search, KeyCode::Char('c'))
                    if key.modifiers == KeyModifiers::CONTROL =>
                {
                    app.toggle_case_sensitivity();
                },
                // Ctrl + r toggles regex mode
                (AppMode::Search, KeyCode::Char('r'))
                    if key.modifiers == KeyModifiers::CONTROL =>
                {
                    app.toggle_regex_mode();
                },
                (AppMode::Search, KeyCode::Char(ch)) =>
                {
                    app.add_search_char(ch);
                },

                // Search result navigation
                (AppMode::Normal, KeyCode::Char('n')) =>
                {
                    app.next_search_result();
                },
                (AppMode::Normal, KeyCode::Char('N')) =>
                {
                    app.prev_search_result();
                },
                (AppMode::Normal, KeyCode::Esc) =>
                {
                    app.reset_search_highlights();
                },

                // ToC navigation
                (AppMode::Normal, KeyCode::Char('w'))
                    if app
                        .app_state
                        .contains(AppStateFlags::SHOULD_SHOW_TOC) =>
                {
                    app.rfc_toc_panel.previous();
                },
                (AppMode::Normal, KeyCode::Char('s'))
                    if app
                        .app_state
                        .contains(AppStateFlags::SHOULD_SHOW_TOC) =>
                {
                    app.rfc_toc_panel.next();
                },
                (AppMode::Normal, KeyCode::Enter)
                    if app
                        .app_state
                        .contains(AppStateFlags::SHOULD_SHOW_TOC) =>
                {
                    app.jump_to_toc_entry();
                },

                _ =>
                {}, // Ignore other key combinations
            }
        }
    }

    Ok(())
}
