//! Application module for the RFC reader.
//!
//! This module provides the main application state and logic for the RFC
//! reader. It handles the display and interaction with RFC documents including
//! scrolling, searching, and navigation.
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
    },
};
use regex::Regex;

use super::toc_panel::TocPanel;

use std::collections::HashMap;
use std::ops::Range;

const HIGHLIGHT_STYLE: Style = Style::new()
    .fg(Color::Yellow)
    .add_modifier(Modifier::BOLD);

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

/// Type alias for line numbers.
pub(super) type LineNumber = usize;

/// Main application state for the RFC reader.
///
/// Handles the display and interaction with RFC documents including
/// scrolling, searching, and navigation.
pub struct App
{
    // Core document
    /// Content of the currently loaded RFC
    pub rfc_content: String,
    /// Number of the currently loaded RFC
    pub rfc_number: u16,
    /// Table of contents panel for the current document
    pub rfc_toc_panel: TocPanel,
    /// Line number of the content
    pub rfc_line_number: LineNumber,

    // Navigation
    /// Current scroll position in the document
    pub current_scroll_pos: LineNumber,

    // UI state
    /// Current application mode
    pub mode: AppMode,
    /// Flag indicating if the table of contents should be displayed
    pub show_toc: bool,
    /// Flag indicating if the application should run
    pub should_run: bool,

    // Search
    /// Current search query text
    pub search_text: String,
    /// Line numbers where search results were found
    pub search_results: Vec<LineNumber>,
    /// Index of the currently selected search result
    pub current_search_index: LineNumber,
    /// Map of line numbers to their matching spans.
    pub search_matches: HashMap<LineNumber, Vec<Range<usize>>>,
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
    pub fn new(rfc_number: u16, content: String) -> Self
    {
        let toc_panel = TocPanel::new(&content);
        let rfc_line_number = content.lines().count();

        Self {
            rfc_content: content,
            rfc_number,
            rfc_toc_panel: toc_panel,
            rfc_line_number,
            current_scroll_pos: 0,
            mode: AppMode::Normal,
            should_run: true,
            show_toc: false,
            search_text: String::with_capacity(20),
            search_results: Vec::with_capacity(50),
            current_search_index: 0,
            search_matches: HashMap::with_capacity(50),
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
    pub fn render(&mut self, frame: &mut Frame)
    {
        // Clear the entire frame on each render to prevent artifacts
        frame.render_widget(Clear, frame.area());

        // Normal mode layout
        let size = frame.area();

        let chunks = if self.show_toc
        {
            // Create layout with ToC panel on the left
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

        // If ToC is shown, render it on the left side (chunks[0])
        if self.show_toc
        {
            self.rfc_toc_panel.render(frame, chunks[0]);
        }

        // Render the main content area on the right side (chunks[1])
        // chunks[0] is the ToC if it is shown
        // chunks[1] is the content if ToC is not shown
        let content_area = if self.show_toc { chunks[1] } else { chunks[0] };

        // Render the text with highlights if in search mode or if there is a search
        // text
        let text = self.build_text();
        let title = format!("RFC {} - Press ? for help", self.rfc_number);

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .title_alignment(Alignment::Center),
            )
            .scroll((self.current_scroll_pos.try_into().unwrap(), 0));

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state =
            ScrollbarState::new(self.rfc_line_number).position(self.current_scroll_pos);

        // Rendering the paragraph and the scrollbar happens here.
        frame.render_widget(paragraph, content_area);
        frame.render_stateful_widget(scrollbar, content_area, &mut scrollbar_state);

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

    /// Builds the RFC text, highlighted if searching.
    fn build_text(&self) -> Text<'_>
    {
        if self.mode == AppMode::Search || !self.search_text.is_empty()
        {
            let lines: Vec<Line> = self
                .rfc_content
                .lines()
                .enumerate()
                .map(|(line_num, line_str)| {
                    // Highlight spans that match in the current line.
                    if let Some(matches) = self.search_matches.get(&line_num)
                    {
                        let mut spans = Vec::new();
                        let mut last_end = 0;
                        let mut sorted_matches = matches.clone();
                        sorted_matches.sort_by_key(|range| range.start);

                        for range in sorted_matches
                        {
                            if range.start > last_end
                            {
                                spans.push(Span::raw(&line_str[last_end..range.start]));
                            }
                            spans.push(Span::styled(
                                &line_str[range.start..range.end],
                                HIGHLIGHT_STYLE,
                            ));
                            last_end = range.end;
                        }
                        if last_end < line_str.len()
                        {
                            spans.push(Span::raw(&line_str[last_end..]));
                        }
                        Line::from(spans)
                    }
                    else
                    {
                        // No matches, leave the line as is.
                        Line::from(line_str)
                    }
                })
                .collect();

            Text::from(lines)
        }
        else
        {
            // No highlights
            Text::raw(&self.rfc_content)
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
            Line::from("w/s: Navigate ToC up/down"),
            Line::from("Enter: Jump to ToC entry"),
            Line::from("/: Search"),
            Line::from("n/N: Next/previous search result"),
            Line::from("Esc: Reset search highlights"),
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
    pub fn scroll_up(&mut self, amount: LineNumber)
    {
        self.current_scroll_pos = self
            .current_scroll_pos
            .saturating_sub(amount);
    }

    /// Scrolls the document down by the specified amount.
    ///
    /// # Arguments
    ///
    /// * `amount` - Number of lines to scroll down
    pub fn scroll_down(&mut self, amount: LineNumber)
    {
        let last_line_pos = self.rfc_line_number.saturating_sub(1); // Last line
        // Clamp the scroll position to the last line.
        self.current_scroll_pos = (self.current_scroll_pos + amount).min(last_line_pos);
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
        self.search_results.clear();
        self.search_matches.clear();

        if self.search_text.is_empty()
        {
            return;
        }

        let pattern = regex::escape(&self.search_text);
        let Ok(regex) = Regex::new(&format!("(?i){pattern}"))
        else
        {
            return;
        };

        // Search line by line.
        for (line_num, line) in self.rfc_content.lines().enumerate()
        {
            let mut matches_in_line = Vec::new();
            for r#match in regex.find_iter(line)
            {
                // Add the range of the match.
                matches_in_line.push(r#match.range());
            }

            if !matches_in_line.is_empty()
            {
                // Add the line number and matches to the search results.
                self.search_results.push(line_num);
                self.search_matches
                    .insert(line_num, matches_in_line);
            }
        }

        // Jump to the first result.
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

        if self.current_search_index != self.search_results.len() - 1
        {
            self.current_search_index += 1;
            self.jump_to_search_result();
        }
    }

    /// Moves to the previous search result.
    pub fn prev_search_result(&mut self)
    {
        if self.search_results.is_empty()
        {
            return;
        }

        if self.current_search_index != 0
        {
            self.current_search_index -= 1;
            self.jump_to_search_result();
        }
    }

    /// Jumps to the current search result by scrolling to its line.
    fn jump_to_search_result(&mut self)
    {
        if let Some(line) = self
            .search_results
            .get(self.current_search_index)
        {
            self.current_scroll_pos = *line;
        }
    }

    /// Resets the search highlights.
    pub fn reset_search_highlights(&mut self)
    {
        self.search_text.clear();
        self.search_results.clear();
        self.search_matches.clear();
        self.current_search_index = 0;
    }

    /// Jumps to the current `ToC` entry by scrolling to its line.
    pub fn jump_to_toc_entry(&mut self)
    {
        if let Some(line) = self.rfc_toc_panel.selected_line()
        {
            self.current_scroll_pos = line;
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
