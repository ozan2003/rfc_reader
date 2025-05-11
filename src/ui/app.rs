//! Application module for the RFC reader.
//!
//! This module provides the main application state and logic for the RFC
//! reader. It handles the display and interaction with RFC documents including
//! scrolling, searching, and navigation.
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::toc_panel::TocPanel;

/// Application mode that determines the current UI state.
///
/// Controls what is displayed and how user input is interpreted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode
{
    /// Normal reading mode - default state
    Normal,
    /// Help overlay is displayed
    Help,
    /// Search mode - accepting search input
    Search,
}

/// Main application state for the RFC reader.
///
/// Handles the display and interaction with RFC documents including
/// scrolling, searching, and navigation.
pub struct App
{
    /// Content of the currently loaded RFC
    pub rfc_content: String,
    /// Number of the currently loaded RFC
    pub rfc_number: u32,
    /// Current scroll position in the document
    pub scroll: usize,
    /// Current application mode
    pub mode: AppMode,
    /// Current search query text
    pub search_text: String,
    /// Line numbers where search results were found
    pub search_results: Vec<usize>,
    /// Index of the currently selected search result
    pub current_search_index: usize,
    /// Flag indicating if the application should exit
    pub should_quit: bool,
    /// Flag indicating if the table of contents should be displayed
    pub show_toc: bool,
    /// Table of contents panel for the current document
    pub toc_panel: TocPanel,
}

impl App
{
    /// Creates a new App instance with the specified RFC.
    ///
    /// # Arguments
    ///
    /// * `rfc_number` - The RFC number being displayed
    /// * `content` - The content of the RFC document
    ///
    /// # Returns
    ///
    /// A new `App` instance initialized for the specified RFC
    #[must_use]
    pub fn new(rfc_number: u32, content: String) -> Self
    {
        let toc_panel = TocPanel::new(&content);

        Self {
            rfc_content: content,
            rfc_number,
            scroll: 0,
            mode: AppMode::Normal,
            search_text: String::new(),
            search_results: Vec::new(),
            current_search_index: 0,
            should_quit: false,
            show_toc: false,
            toc_panel,
        }
    }

    /// Renders the application UI to the provided frame.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render the UI to
    ///
    /// # Panics
    ///
    /// Panics if the frame is not the correct size.
    pub fn render(&self, frame: &mut Frame)
    {
        // Normal mode layout
        let size = frame.area();

        let chunks = if self.show_toc
        {
            // Create layout with TOC panel on the left
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(25), Constraint::Percentage(75)].as_ref())
                .split(size)
        }
        else
        {
            // Full-width layout for content only
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(size)
        };

        // If TOC is shown, render it
        if self.show_toc
        {
            self.toc_panel.render(frame, chunks[0]);
        }

        // Render the main content area
        let content_area = if self.show_toc { chunks[1] } else { chunks[0] };

        let text = Text::from(self.rfc_content.clone());

        let title = format!("RFC {} - Press ? for help", self.rfc_number);

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll.try_into().unwrap(), 0));

        frame.render_widget(paragraph, content_area);

        // Render help if in help mode
        if self.mode == AppMode::Help
        {
            Self::render_help(frame);
        }

        // Render search if in search mode
        if self.mode == AppMode::Search
        {
            self.render_search(frame);
        }
    }

    /// Renders the help overlay with keyboard shortcuts.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render the help overlay to
    fn render_help(frame: &mut Frame)
    {
        // Create a centered rectangle.
        let area = centered_rect(60, 60, frame.area());

        // Clear the area first to make it fully opaque
        frame.render_widget(Clear, area);

        // Vim-like controls
        let text = Text::from(vec![
            Line::from("RFC Reader Help:"),
            Line::from(""),
            Line::from("j/k or ↓/↑: Scroll down/up"),
            Line::from("f/b or PgDn/PgUp: Scroll page down/up"),
            Line::from("g/G: Go to start/end of document"),
            Line::from("t: Toggle table of contents"),
            Line::from("/: Search"),
            Line::from("n/N: Next/previous search result"),
            Line::from("q: Quit"),
            Line::from("?: Toggle help"),
        ]);

        let help_box = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help")
                    .style(Style::default()),
            )
            .style(Style::default())
            .wrap(Wrap { trim: true });

        // Put the help box in it.
        frame.render_widget(help_box, area);
    }

    /// Renders the search input box.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render the search box to
    fn render_search(&self, frame: &mut Frame)
    {
        // Same logic as the help box.
        let area = Rect::new(
            frame.area().width / 4,
            frame.area().height - 3,
            frame.area().width / 2,
            3,
        );

        // Clear the area first to make it fully opaque
        frame.render_widget(Clear, area);

        let text = Text::from(format!("/{}", self.search_text));

        let search_box = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Search")
                    .style(Style::default()),
            )
            .style(Style::default());

        frame.render_widget(search_box, area);
    }

    /// Scrolls the document up by the specified amount.
    ///
    /// # Arguments
    ///
    /// * `amount` - Number of lines to scroll up
    pub fn scroll_up(&mut self, amount: usize)
    {
        self.scroll = self.scroll.saturating_sub(amount);
    }

    /// Scrolls the document down by the specified amount.
    ///
    /// # Arguments
    ///
    /// * `amount` - Number of lines to scroll down
    pub fn scroll_down(&mut self, amount: usize)
    {
        // The max scroll position will be handled by ratatui
        self.scroll = self.scroll.saturating_add(amount);
    }

    /// Toggles the help overlay.
    pub fn toggle_help(&mut self)
    {
        self.mode = if self.mode == AppMode::Help
        {
            AppMode::Normal
        }
        else
        {
            AppMode::Help
        };
    }

    /// Toggles the table of contents panel.
    pub fn toggle_toc(&mut self)
    {
        self.show_toc = !self.show_toc;
    }

    /// Enters search mode, clearing any previous search.
    pub fn enter_search_mode(&mut self)
    {
        self.mode = AppMode::Search;
        self.search_text.clear();
    }

    /// Exits search mode and returns to normal mode.
    pub fn exit_search_mode(&mut self)
    {
        self.mode = AppMode::Normal;
    }

    /// Adds a character to the search text.
    ///
    /// # Arguments
    ///
    /// * `ch` - The character to add
    pub fn add_search_char(&mut self, ch: char)
    {
        self.search_text.push(ch);
    }

    /// Removes the last character from the search text.
    pub fn remove_search_char(&mut self)
    {
        self.search_text.pop();
    }

    /// Performs a search using the current search text.
    ///
    /// Finds all occurrences of the search text in the RFC content
    /// and stores the results. If results are found, jumps to the
    /// first result.
    pub fn perform_search(&mut self)
    {
        if self.search_text.is_empty()
        {
            self.search_results.clear();
            return;
        }

        self.search_results = self
            .rfc_content
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                if line
                    .to_lowercase()
                    .contains(&self.search_text.to_lowercase())
                {
                    Some(index)
                }
                else
                {
                    None
                }
            })
            .collect();

        self.current_search_index = 0;

        if !self.search_results.is_empty()
        {
            self.jump_to_search_result();
        }
    }

    /// Moves to the next search result.
    pub fn next_search_result(&mut self)
    {
        if self.search_results.is_empty()
        {
            return;
        }

        self.current_search_index = (self.current_search_index + 1) % self.search_results.len();
        self.jump_to_search_result();
    }

    /// Moves to the previous search result.
    pub fn prev_search_result(&mut self)
    {
        if self.search_results.is_empty()
        {
            return;
        }

        self.current_search_index = if self.current_search_index == 0
        {
            self.search_results.len() - 1
        }
        else
        {
            self.current_search_index - 1
        };

        self.jump_to_search_result();
    }

    /// Jumps to the current search result by scrolling to its line.
    fn jump_to_search_result(&mut self)
    {
        if let Some(line) = self
            .search_results
            .get(self.current_search_index)
        {
            self.scroll = *line;
        }
    }
}

/// Creates a centered rectangle inside the given area.
///
/// # Arguments
///
/// * `percent_x` - Width of the rectangle as a percentage of the parent area
/// * `percent_y` - Height of the rectangle as a percentage of the parent area
/// * `r` - Parent rectangle
///
/// # Returns
///
/// A new rectangle positioned in the center of the parent
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect
{
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
