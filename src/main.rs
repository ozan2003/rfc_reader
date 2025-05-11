use anyhow::Result;
use clap::{Arg, ArgAction, Command};
use crossterm::ExecutableCommand;
use crossterm::event::KeyCode;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use rfc_reader::{App, AppMode, Event, EventHandler, RfcCache, RfcClient};
use std::{io, time::Duration};

#[tokio::main]
async fn main() -> Result<()>
{
    // Parse command line arguments
    let matches = Command::new("rfc_reader")
        //.version(env!("CARGO_PKG_VERSION"))
        //.author("RFC Reader Team")
        .about("A terminal-based RFC reader")
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

    // Initialize cache
    let cache = RfcCache::new()?;

    // Clear cache if requested
    if matches.get_flag("clear-cache")
    {
        // Clear all cached RFCs
        cache.clear()?;
        println!("Cache cleared successfully");
        return Ok(());
    }

    // Setup client
    let client = RfcClient::new();

    // Setup terminal
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    // Get RFC if specified
    let rfc_number = matches
        .get_one::<String>("rfc")
        .and_then(|s| s.parse::<u32>().ok());

    // Placeholder for getting RFC content
    let (rfc_number, rfc_content) = if let Some(number) = rfc_number
    {
        // Get the RFC content - first check cache, then fetch from network if needed
        let content = match cache.get_cached_rfc(number)
        {
            Some(cached_content) =>
            {
                println!("Using cached version of RFC {number}");
                cached_content
            }
            None =>
            {
                let offline_mode = matches.get_flag("offline");
                if offline_mode
                {
                    println!("Cannot load RFC {number} - not in cache and offline mode is enabled");
                    String::from("Offline mode - RFC {number} not available in cache")
                }
                else
                {
                    // Fetch RFC from network since it's not in cache
                    println!("Fetching RFC {number} from network...");

                    match client.fetch_rfc(number).await
                    {
                        Ok(content) =>
                        {
                            // Cache the fetched content for future use.
                            if let Err(e) = cache.cache_rfc(number, &content)
                            {
                                println!("Warning: Failed to cache RFC {number}: {e}");
                            }
                            content
                        }
                        Err(e) =>
                        {
                            println!("Error fetching RFC {number}: {e}");
                            format!("Failed to fetch RFC {number}. Error: {e}")
                        }
                    }
                }
            }
        };
        (number, content)
    }
    else
    {
        eprintln!("No RFC number provided");
        return Err(anyhow::anyhow!("No RFC number provided"));
    };

    // Create app state
    let app = App::new(rfc_number, rfc_content);

    // Create event handler with 250ms tick rate
    let event_handler = EventHandler::new(Duration::from_millis(250));

    // Run the main loop
    let res = run_app(&mut terminal, app, &event_handler);

    // Cleanup terminal
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    // Return any error from the app
    if let Err(err) = res
    {
        eprintln!("Error: {err}");
    }

    Ok(())
}

fn run_app<T: ratatui::backend::Backend>(
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
                (AppMode::Normal, KeyCode::Char('?'))  | (AppMode::Help, KeyCode::Char('?')) | (AppMode::Help, KeyCode::Esc) =>
                {
                    app.toggle_help();
                }
                /*(AppMode::Help, KeyCode::Char('?')) | (AppMode::Help, KeyCode::Esc) =>
                {
                    app.toggle_help();
                }*/

                // Table of contents toggle with 't'
                (AppMode::Normal, KeyCode::Char('t')) =>
                {
                    app.toggle_toc();
                }

                // Navigation in normal mode
                (AppMode::Normal, KeyCode::Char('j')) | (AppMode::Normal, KeyCode::Down) =>
                {
                    app.scroll_down(1);
                }
                (AppMode::Normal, KeyCode::Char('k')) | (AppMode::Normal, KeyCode::Up) =>
                {
                    app.scroll_up(1);
                }
                (AppMode::Normal, KeyCode::Char('f')) | (AppMode::Normal, KeyCode::PageDown) =>
                {
                    app.scroll_down(10);
                }
                (AppMode::Normal, KeyCode::Char('b')) | (AppMode::Normal, KeyCode::PageUp) =>
                {
                    app.scroll_up(10);
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
                (AppMode::Search, KeyCode::Char(c)) =>
                {
                    app.add_search_char(c);
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
