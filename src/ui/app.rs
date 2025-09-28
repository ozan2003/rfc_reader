//! Core application logic and app state management.
//!
//! Provides the central application state and handles UI rendering and user
//! input. This includes features such as document scrolling, searching,
//! and navigation.
use std::collections::HashMap;
use std::io::stdout;
use std::ops::Range;

use bitflags::bitflags;
use crossterm::execute;
use crossterm::terminal::{SetTitle, size};
use log::warn;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use regex::Regex;

use super::guard::TerminalGuard;
use super::toc_panel::TocPanel;

/// Style for highlighting matches in the search results.
const MATCH_HIGHLIGHT_STYLE: Style = Style::new()
    .fg(Color::Yellow)
    .add_modifier(Modifier::BOLD);

/// Style for highlighting titles in the document.
const TITLE_HIGHLIGHT_STYLE: Style = Style::new()
    .fg(Color::Cyan)
    .add_modifier(Modifier::BOLD);

/// Style for the statusbar.
const STATUSBAR_STYLE: Style = Style::new()
    .bg(Color::White)
    .fg(Color::Black);

// UI constants
/// Minimum terminal width in columns for proper UI rendering.
// Note: Gotta review when the text are edited.
const MIN_TERMINAL_WIDTH: u16 = 94;
/// Minimum terminal height in rows for proper UI rendering.
const MIN_TERMINAL_HEIGHT: u16 = 15;

// ToC/content split percentages.
/// 1/4 for `ToC`, 3/4 for content
const TOC_PERCENTAGE: u16 = 25;
/// Constraints for the `ToC`/content split.
const TOC_SPLIT_CONSTRAINTS: [Constraint; 2] = [
    Constraint::Percentage(TOC_PERCENTAGE),
    Constraint::Percentage(100 - TOC_PERCENTAGE),
];

/// Application mode for the current UI state.
///
/// Controls what is displayed and how the user input is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode
{
    /// Normal reading mode, default state
    Normal,
    /// Help overlay being displayed
    Help,
    /// Search mode, accepting search input
    Search,
}

bitflags! {
    /// Flags indicating the current state of the application.
    #[derive(Debug)]
    pub struct AppStateFlags: u8
    {
        /// Application should continue running
        const SHOULD_RUN = 1;
        /// Whether table of contents should be displayed
        const SHOW_TOC = 1 << 1;
        /// Whether search yields no results
        const HAS_NO_RESULTS = 1 << 2;
    }
}

impl Default for AppStateFlags
{
    fn default() -> Self
    {
        Self::SHOULD_RUN
    }
}

/// Type alias for line numbers.
pub(super) type LineNumber = usize;

/// Type alias for matches spanning a line.
type MatchSpan = Range<usize>;

/// Manages the core state and UI logic.
///
/// This includes rendering the document, processing user input, and handling
/// interactions like scrolling, searching, navigation and graceful shutdown.
pub struct App
{
    // Core document
    /// Content of the currently loaded RFC
    pub rfc_content: Box<str>,
    /// Number of the currently loaded RFC
    pub rfc_number: u16,
    /// Table of contents panel for the current document
    pub rfc_toc_panel: TocPanel,
    /// Total line number of the content
    pub rfc_line_number: LineNumber,

    // Navigation
    /// Current scroll position in the document
    pub current_scroll_pos: LineNumber,

    // UI state
    /// Current application mode
    pub mode: AppMode,
    /// Flags for managing the application state.
    pub app_state: AppStateFlags,
    /// Handle graceful terminal shutdown.
    #[allow(
        dead_code,
        reason = "Its purpose is its `Drop` implementation, not direct field \
                  access."
    )]
    guard: TerminalGuard,

    // Search
    /// Text of the query to search.
    pub query_text: String,
    /// Line numbers where query matches were found.
    pub query_match_line_nums: Vec<LineNumber>,
    /// Index of the currently selected query match.
    pub current_query_match_index: LineNumber,
    /// Line numbers and their positions of query matches.
    pub query_matches: HashMap<LineNumber, Vec<MatchSpan>>,
}

impl App
{
    /// Creates a new App instance with the specified RFC.
    ///
    /// # Arguments
    ///
    /// * `rfc_number` - The RFC number of the document
    /// * `content` - The content of the RFC document
    ///
    /// # Returns
    ///
    /// A new `App` instance initialized for the specified RFC
    #[must_use]
    pub fn new(rfc_number: u16, rfc_content: Box<str>) -> Self
    {
        let rfc_toc_panel = TocPanel::new(&rfc_content);
        let rfc_line_number = rfc_content.lines().count();

        let title = format!("RFC {rfc_number} - Press ? for help");
        if let Err(error) = execute!(stdout(), SetTitle(title))
        {
            warn!("Couldn't set the window title: {error}");
        }

        Self {
            rfc_content,
            rfc_number,
            rfc_toc_panel,
            rfc_line_number,
            ..Default::default()
        }
    }

    /// Checks if the terminal is too small.
    ///
    /// # Returns
    ///
    /// A boolean indicating if the terminal is too small.
    fn is_terminal_too_small() -> bool
    {
        let (current_width, current_height) =
            size().expect("Couldn't get terminal size");

        current_width < MIN_TERMINAL_WIDTH ||
            current_height < MIN_TERMINAL_HEIGHT
    }

    /// Builds the RFC text with highlighting for search matches and titles.
    fn build_text(&self) -> Text<'_>
    {
        // Check if we need search highlighting
        let has_searched =
            self.mode == AppMode::Search || !self.query_text.is_empty();

        let lines: Vec<Line> = self
            .rfc_content
            .lines()
            .enumerate()
            .map(|(line_num, line_str)| {
                let is_title = self.rfc_toc_panel
                                         .entries()
                                         .binary_search_by(|entry| entry.line_number.cmp(&line_num))
                                         .is_ok();

                if has_searched
                {
                    // Highlight search match
                    if let Some(matches) = self.query_matches.get(&line_num)
                    {
                        return Self::build_line_with_search_and_title_highlights(
                            line_str, matches, is_title,
                        );
                    }
                }

                if is_title
                {
                    // Only title highlighting
                    Line::from(Span::styled(line_str, TITLE_HIGHLIGHT_STYLE))
                }
                else
                {
                    // No highlighting
                    Line::from(line_str)
                }
            })
            .collect();

        Text::from(lines)
    }

    /// Builds a line with both search and title highlighting.
    ///
    /// # Arguments
    ///
    /// * `line_str` - The line content
    /// * `matches` - Search match spans in the line
    /// * `is_title` - Whether this line is a title
    ///
    /// # Returns
    ///
    /// A `Line` with appropriate highlighting applied
    fn build_line_with_search_and_title_highlights<'line_str>(
        line_str: &'line_str str,
        matches: &[MatchSpan],
        is_title: bool,
    ) -> Line<'line_str>
    {
        let mut spans = Vec::new();
        let mut last_end = 0;

        for match_span in matches
        {
            // Clamp indexes to the line length to avoid out of bounds access
            let start = match_span.start.min(line_str.len());
            let end = match_span.end.min(line_str.len());

            if start > last_end &&
                let Some(text) = line_str.get(last_end..start)
            {
                if is_title
                {
                    spans.push(Span::styled(text, TITLE_HIGHLIGHT_STYLE));
                }
                else
                {
                    spans.push(Span::raw(text));
                }
            }

            if let Some(m) = line_str.get(start..end)
            {
                spans.push(Span::styled(m, MATCH_HIGHLIGHT_STYLE));
            }

            last_end = end;
        }

        // Add remaining text after the last match
        if last_end < line_str.len() &&
            let Some(text) = line_str.get(last_end..)
        {
            if is_title
            {
                spans.push(Span::styled(text, TITLE_HIGHLIGHT_STYLE));
            }
            else
            {
                spans.push(Span::raw(text));
            }
        }

        Line::from(spans)
    }

    /// Renders the application UI to the provided frame.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render the UI to
    ///
    /// # Panics
    ///
    /// Panics if the frame cannot be rendered.
    pub fn render(&mut self, frame: &mut Frame)
    {
        /// Height of the status bar in rows.
        const STATUSBAR_HEIGHT_CONSTRAINT: Constraint = Constraint::Length(1);

        if Self::is_terminal_too_small()
        {
            Self::render_too_small_message(frame);
            return;
        }

        // Clear the entire frame on each render to prevent artifacts
        frame.render_widget(Clear, frame.area());

        // Create main layout with statusbar at bottom
        let [main_area, statusbar_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0), // Main content takes remaining space
                STATUSBAR_HEIGHT_CONSTRAINT,
            ])
            .areas(frame.area());

        let (content_area, toc_area) = if self
            .app_state
            .contains(AppStateFlags::SHOW_TOC)
        {
            // Create layout with ToC panel on the left
            let [toc_area, content_area] = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(TOC_SPLIT_CONSTRAINTS)
                .areas(main_area);

            (content_area, Some(toc_area))
        }
        else
        {
            (main_area, None)
        };

        if let Some(toc_area) = toc_area
        {
            // Render ToC in the left area
            self.rfc_toc_panel.render(frame, toc_area);
        }

        // Render the text with highlights if in search mode or if there is a
        // search text
        let text = self.build_text();

        // Clamp the scroll position instead of panicking
        let y = u16::try_from(self.current_scroll_pos).unwrap_or(u16::MAX);
        let paragraph = Paragraph::new(text).scroll((y, 0));

        // Rendering the paragraph happens here
        frame.render_widget(paragraph, content_area);

        // Render statusbar
        self.render_statusbar(frame, statusbar_area);

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

        // Render no search message
        if self
            .app_state
            .contains(AppStateFlags::HAS_NO_RESULTS)
        {
            Self::render_no_search_results(frame);
        }
    }

    /// Renders the help overlay with keyboard shortcuts.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render the help overlay to
    fn render_help(frame: &mut Frame)
    {
        /// Help overlay box width as percentage of the terminal width.
        const HELP_OVERLAY_WIDTH_CONSTRAINT: Constraint =
            Constraint::Percentage(60);
        /// Help overlay box height as percentage of the terminal height.
        const HELP_OVERLAY_HEIGHT_CONSTRAINT: Constraint =
            Constraint::Percentage(65);

        // Create a centered rectangle.
        let area = centered_rect(
            frame.area(),
            HELP_OVERLAY_WIDTH_CONSTRAINT,
            HELP_OVERLAY_HEIGHT_CONSTRAINT,
        );

        // Clear the area first to make it fully opaque
        frame.render_widget(Clear, area);

        let text = Text::from(vec![
            Line::from("Keybindings:"),
            Line::from(""),
            // Vim-like navigation
            Line::from("j/k or ↓/↑: Scroll down/up"),
            Line::from("f/b or PgDn/PgUp: Scroll page down/up"),
            Line::from("g/G: Go to start/end of document"),
            Line::from(""),
            Line::from("t: Toggle table of contents"),
            Line::from("w/s: Navigate ToC up/down"),
            Line::from("Enter: Jump to ToC entry"),
            Line::from(""),
            Line::from("/: Search"),
            Line::from("n/N: Next/previous search result"),
            Line::from("Esc: Reset search highlights"),
            Line::from(""),
            Line::from("q: Quit"),
            Line::from("?: Toggle help"),
        ]);

        let help_box = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("RFC Reader Help")
                    .title_alignment(Alignment::Center)
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
        /// Search box height in rows.
        const SEARCH_BOX_HEIGHT_ROWS: u16 = 3;
        /// Horizontal start position divisor (x = width /
        /// `SEARCH_BOX_X_DIVISOR`).
        const SEARCH_BOX_X_DIVISOR: u16 = 4;
        /// Box width divisor (`box_width` = width /
        /// `SEARCH_BOX_WIDTH_DIVISOR`).
        const SEARCH_BOX_WIDTH_DIVISOR: u16 = 2;
        /// Distance from bottom in rows.
        const SEARCH_BOX_BOTTOM_OFFSET_ROWS: u16 = 4;

        let area = Rect::new(
            frame.area().width / SEARCH_BOX_X_DIVISOR,
            frame
                .area()
                .height
                .saturating_sub(SEARCH_BOX_BOTTOM_OFFSET_ROWS),
            frame.area().width / SEARCH_BOX_WIDTH_DIVISOR,
            SEARCH_BOX_HEIGHT_ROWS,
        );

        // Clear the area first to make it fully opaque
        frame.render_widget(Clear, area);

        let text = Text::from(format!("/{}", self.query_text));

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

    /// Renders the no search results message.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render the no search results message to
    fn render_no_search_results(frame: &mut Frame)
    {
        /// No-search-results overlay width as percentage of the terminal width.
        const NO_SEARCH_OVERLAY_WIDTH_CONSTRAINT: Constraint =
            Constraint::Percentage(40);
        /// No-search-results overlay height percentage.
        const NO_SEARCH_OVERLAY_HEIGHT_CONSTRAINT: Constraint =
            Constraint::Percentage(25);
        /// No-search-results overlay title text.
        const NO_SEARCH_TITLE: &str = "No matches - Press Esc to dismiss";
        /// No-search-results overlay message text.
        const NO_SEARCH_MESSAGE: &str = "Search yielded nothing";

        let area = centered_rect(
            frame.area(),
            NO_SEARCH_OVERLAY_WIDTH_CONSTRAINT,
            NO_SEARCH_OVERLAY_HEIGHT_CONSTRAINT,
        );

        // Clear the area first to make it fully opaque
        frame.render_widget(Clear, area);

        let text = Text::raw(NO_SEARCH_MESSAGE);

        let no_search_box = Paragraph::new(text)
            .block(
                Block::default()
                    .title(NO_SEARCH_TITLE)
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Red)),
            )
            .alignment(Alignment::Center)
            .style(Style::default());

        frame.render_widget(no_search_box, area);
    }

    /// Renders the too small message.
    ///
    /// The message is displayed when the terminal is too small to display
    /// the application.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render the too small message to
    fn render_too_small_message(frame: &mut Frame)
    {
        /// "Terminal too small" overlay height as percentage of the terminal
        /// height.
        const TOO_SMALL_OVERLAY_HEIGHT_CONSTRAINT: Constraint =
            Constraint::Percentage(50);
        /// "Terminal too small" overlay title text.
        const TOO_SMALL_ERROR_TEXT: &str = "Terminal size is too small:";

        let (current_width, current_height) =
            size().expect("Couldn't get terminal size");

        // Determine colors based on whether dimensions meet requirements
        let current_width_color = if current_width >= MIN_TERMINAL_WIDTH
        {
            Color::Green
        }
        else
        {
            Color::Red
        };

        let current_height_color = if current_height >= MIN_TERMINAL_HEIGHT
        {
            Color::Green
        }
        else
        {
            Color::Red
        };

        // Clear the area first to make it fully opaque
        frame.render_widget(Clear, frame.area());

        let area = centered_rect(
            frame.area(),
            Constraint::Min(
                TOO_SMALL_ERROR_TEXT
                    .len()
                    .try_into()
                    .expect("TOO_SMALL_ERROR_TEXT length too big to cast"),
            ),
            TOO_SMALL_OVERLAY_HEIGHT_CONSTRAINT,
        );

        let text = Text::from(vec![
            Line::from(TOO_SMALL_ERROR_TEXT),
            Line::from(vec![
                Span::raw("Width: "),
                Span::styled(
                    format!("{current_width}"),
                    Style::default()
                        .fg(current_width_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(", "),
                Span::raw("Height: "),
                Span::styled(
                    format!("{current_height}"),
                    Style::default()
                        .fg(current_height_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from("Minimum required:"),
            Line::from(vec![
                Span::raw("Width: "),
                Span::styled(
                    format!("{MIN_TERMINAL_WIDTH}"),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(", "),
                Span::raw("Height: "),
                Span::styled(
                    format!("{MIN_TERMINAL_HEIGHT}"),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
        ]);

        let paragraph = Paragraph::new(text).alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Renders the statusbar with current status.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render the statusbar to
    /// * `area` - The area to render the statusbar in
    fn render_statusbar(&self, frame: &mut Frame, area: Rect)
    {
        /// Statusbar left section minimum width in columns.
        const STATUSBAR_LEFT_MIN_COLS: u16 = 40;
        /// Statusbar middle section minimum width in columns.
        const STATUSBAR_MIDDLE_MIN_COLS: u16 = 0; // takes remaining space
        /// Statusbar right section minimum width in columns.
        const STATUSBAR_RIGHT_MIN_COLS: u16 = 42;

        let [left_section, middle_section, right_section] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(STATUSBAR_LEFT_MIN_COLS),
                Constraint::Min(STATUSBAR_MIDDLE_MIN_COLS), // Middle takes remaining space
                Constraint::Min(STATUSBAR_RIGHT_MIN_COLS),
            ])
            .flex(Flex::SpaceBetween)
            .areas(area);

        // Left section
        let progress_text = self.build_progress_text();
        let left_text = format!("RFC {} | {}", self.rfc_number, progress_text);
        let left_statusbar = Paragraph::new(left_text)
            .style(STATUSBAR_STYLE)
            .alignment(Alignment::Left);
        frame.render_widget(left_statusbar, left_section);

        // Middle section
        let mode_text = self.get_mode_text();
        let middle_statusbar = Paragraph::new(mode_text)
            .style(STATUSBAR_STYLE)
            .alignment(Alignment::Center);
        frame.render_widget(middle_statusbar, middle_section);

        // Right section
        let help_text = self.get_help_text();
        let right_statusbar = Paragraph::new(help_text)
            .style(STATUSBAR_STYLE)
            .alignment(Alignment::Right);
        frame.render_widget(right_statusbar, right_section);
    }

    /// Builds the mode text representation for the statusbar.
    ///
    /// # Returns
    ///
    /// A string containing the current mode.
    const fn get_mode_text(&self) -> &'static str
    {
        match self.mode
        {
            AppMode::Normal
                if self
                    .app_state
                    .contains(AppStateFlags::SHOW_TOC) =>
            {
                "NORMAL (ToC)"
            },
            AppMode::Normal => "NORMAL",
            AppMode::Help => "HELP",
            AppMode::Search => "SEARCH",
        }
    }

    /// Builds the progress text for the statusbar.
    ///
    /// # Returns
    ///
    /// A string containing the current line number, total lines, progress
    /// percentage, and search information.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "LineNumber not expected to overflow"
    )]
    fn build_progress_text(&self) -> String
    {
        let progress_percentage = if self.rfc_line_number > 0
        {
            (self.current_scroll_pos * 100) / self.rfc_line_number
        }
        else
        {
            0
        };

        let search_info = self.build_search_info().unwrap_or_default();

        format!(
            "L {}/{} ({}%){}",
            self.current_scroll_pos + 1,
            self.rfc_line_number,
            progress_percentage,
            search_info
        )
    }

    /// Builds the search info text for the statusbar.
    /// This includes the current match index and total match count.
    ///
    /// # Returns
    ///
    /// An `Option<String>` containing the search info if there are matches,
    /// or `None` if there are no matches or the query is empty.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "LineNumber not expected to overflow"
    )]
    fn build_search_info(&self) -> Option<String>
    {
        if !self.has_search_results()
        {
            return None;
        }

        let total_matches_n: LineNumber = self.query_match_line_nums.len();
        // Clamp index to last valid match
        let index: LineNumber = self
            .current_query_match_index
            .min(total_matches_n.saturating_sub(1));

        Some(format!(" | M {}/{}", index + 1, total_matches_n))
    }

    /// Builds the help text for the statusbar.
    /// Helps the user understand available commands.
    ///
    /// # Returns
    ///
    /// A string containing the help text for the statusbar.
    const fn get_help_text(&self) -> &'static str
    {
        match (self.mode, self.has_search_results())
        {
            (AppMode::Normal, _)
                if self
                    .app_state
                    .contains(AppStateFlags::SHOW_TOC) =>
            {
                "t:toggle ToC  w/s:nav  Enter:jump  q:quit"
            },
            (AppMode::Normal, true) => "n/N:next/prev  Esc:clear",
            (AppMode::Normal, false) =>
            {
                "up/down:scroll  /:search  ?:help  q:quit"
            },
            (AppMode::Help, _) => "?/Esc:close",
            (AppMode::Search, _) => "Enter:search  Esc:cancel",
        }
    }

    /// Scrolls the document up by the specified amount.
    ///
    /// # Arguments
    ///
    /// * `amount` - Number of lines to scroll up
    pub const fn scroll_up(&mut self, amount: LineNumber)
    {
        // Don't allow wrapping, once we reach the top, stay there.
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
        let last_line_pos = self.rfc_line_number.saturating_sub(1);
        // Clamp the scroll position to the last line.
        // Once we reach the bottom, stay there.
        self.current_scroll_pos = (self
            .current_scroll_pos
            .saturating_add(amount))
        .min(last_line_pos);
    }

    /// Jumps to the current `ToC` entry by scrolling to its line.
    ///
    /// If no entry is selected, does nothing.
    pub fn jump_to_toc_entry(&mut self)
    {
        if let Some(line_num) = self.rfc_toc_panel.selected_line()
        {
            self.current_scroll_pos = line_num;
        }
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
    ///
    /// If the panel is shown, it will be hidden, and vice versa.
    pub fn toggle_toc(&mut self)
    {
        self.app_state
            .toggle(AppStateFlags::SHOW_TOC);
    }

    /// Enters search mode, clearing any previous search.
    pub fn enter_search_mode(&mut self)
    {
        self.mode = AppMode::Search;
        self.query_text.clear(); // Start with an empty search
    }

    /// Exits search mode and returns to normal mode.
    pub const fn exit_search_mode(&mut self)
    {
        self.mode = AppMode::Normal;
    }

    /// Checks if there are any search results.
    ///
    /// # Returns
    ///
    /// A boolean indicating if there are any search results.
    const fn has_search_results(&self) -> bool
    {
        !self.query_text.is_empty() && !self.query_match_line_nums.is_empty()
    }

    /// Adds a character to the search text.
    ///
    /// # Arguments
    ///
    /// * `ch` - The character to add
    pub fn add_search_char(&mut self, ch: char)
    {
        self.query_text.push(ch);
    }

    /// Removes the last character from the search text.
    pub fn remove_search_char(&mut self)
    {
        self.query_text.pop();
    }

    /// Performs a search using the current search text.
    ///
    /// Finds all occurrences of the search text in the RFC content
    /// and stores the results. If results are found, jumps to the
    /// first result starting from the current scroll position.
    pub fn perform_search(&mut self)
    {
        self.query_match_line_nums.clear();
        self.query_matches.clear();

        if self.query_text.is_empty()
        {
            return;
        }

        let pattern = regex::escape(&self.query_text);
        // TODO: Could cache the regexes for repeated searches
        let Ok(regex) = Regex::new(&format!("(?i){pattern}"))
        else
        {
            return;
        };

        // Search line by line.
        for (line_num, line) in self.rfc_content.lines().enumerate()
        {
            let mut matches_in_line: Vec<MatchSpan> = Vec::new();
            for r#match in regex.find_iter(line)
            {
                // Add the range of the match.
                matches_in_line.push(r#match.range());
            }

            if !matches_in_line.is_empty()
            {
                // Shave off excess capacity.
                // Might be premature optimization, i don't know.
                matches_in_line.shrink_to_fit();

                // Add the line number and matches to the search results.
                self.query_match_line_nums.push(line_num);

                // Sort the match ranges by start position to allow
                // consistent iteration order.
                matches_in_line
                    .sort_unstable_by_key(|span: &MatchSpan| span.start);

                self.query_matches
                    .insert(line_num, matches_in_line);
            }
        }

        if self.query_match_line_nums.is_empty()
        {
            self.app_state
                .insert(AppStateFlags::HAS_NO_RESULTS);
        }
        // Jump to the first result starting from our location.
        else
        {
            self.app_state
                .remove(AppStateFlags::HAS_NO_RESULTS);

            self.current_query_match_index = self
                .query_match_line_nums
                // First position where line_num >= self.current_scroll_pos
                .partition_point(|&line_num: &LineNumber| {
                    line_num < self.current_scroll_pos
                });

            self.jump_to_search_result();
        }
    }

    /// Moves to the next search result after the current scroll position.
    ///
    /// If there are no search results, does nothing.
    pub fn next_search_result(&mut self)
    {
        if !self.has_search_results()
        {
            return;
        }

        // Find the first result after the current scroll position
        if let Some(next_index) = self
            .query_match_line_nums
            .iter()
            .position(|&line_num| line_num > self.current_scroll_pos)
        {
            self.current_query_match_index = next_index;
            self.jump_to_search_result();
        }
    }

    /// Moves to the previous search result before the current scroll position.
    ///
    /// If there are no search results, does nothing.
    pub fn prev_search_result(&mut self)
    {
        if !self.has_search_results()
        {
            return;
        }

        // Find the last result before the current scroll position
        if let Some(prev_index) = self
            .query_match_line_nums
            .iter()
            .rposition(|&line_num| line_num < self.current_scroll_pos)
        {
            self.current_query_match_index = prev_index;
            self.jump_to_search_result();
        }
    }

    /// Jumps to the current search result by scrolling to its line.
    fn jump_to_search_result(&mut self)
    {
        if let Some(line_num) = self
            .query_match_line_nums
            .get(self.current_query_match_index)
        {
            self.current_scroll_pos = *line_num;
        }
    }

    /// Resets the search highlights.
    pub fn reset_search_highlights(&mut self)
    {
        self.query_text.clear();
        self.query_match_line_nums.clear();
        self.query_matches.clear();
        self.current_query_match_index = 0;
        self.app_state
            .remove(AppStateFlags::HAS_NO_RESULTS);
    }
}

impl Default for App
{
    fn default() -> Self
    {
        /// Initial capacities for common collections.
        const QUERY_TEXT_INITIAL_CAPACITY: usize = 20;
        const QUERY_RESULTS_INITIAL_CAPACITY: usize = 50;

        let guard =
            TerminalGuard::new().expect("Failed to create terminal guard");

        Self {
            rfc_content: Box::from(""),
            rfc_number: 0,
            rfc_toc_panel: TocPanel::default(),
            rfc_line_number: 0,
            current_scroll_pos: 0,
            mode: AppMode::Normal,
            app_state: AppStateFlags::default(),
            guard,
            query_text: String::with_capacity(QUERY_TEXT_INITIAL_CAPACITY),
            query_match_line_nums: Vec::with_capacity(
                QUERY_RESULTS_INITIAL_CAPACITY,
            ),
            current_query_match_index: 0,
            query_matches: HashMap::with_capacity(
                QUERY_RESULTS_INITIAL_CAPACITY,
            ),
        }
    }
}

/// Creates a centered rectangle inside the given area.
///
/// # Arguments
///
/// * `area` - The parent area
/// * `horizontal` - The horizontal constraint
/// * `vertical` - The vertical constraint
///
/// # Returns
///
/// A new rectangle positioned in the center of the parent
fn centered_rect(
    area: Rect,
    horizontal: Constraint,
    vertical: Constraint,
) -> Rect
{
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical])
        .flex(Flex::Center)
        .areas(area);
    area
}
