//! Core application logic and app state management.
//!
//! Provides the central application state and handles UI rendering and user
//! input. This includes features such as document scrolling, searching,
//! and navigation.
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::stdout;
use std::num::NonZeroU16;
use std::thread;

use bitflags::bitflags;
use cached::proc_macro::cached;
use crossterm::cursor::{Hide, Show};
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
use crate::types::{LineNumber, MatchSpan, RfcNum};

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
const MIN_TERMINAL_WIDTH: u16 = 94;
/// Minimum terminal height in rows for proper UI rendering.
const MIN_TERMINAL_HEIGHT: u16 = 15;

// ToC/content split percentages.
/// Constraints for the `ToC`/content split.
const TOC_SPLIT_CONSTRAINTS: [Constraint; 2] = {
    /// 1/4 for `ToC`, 3/4 for content.
    const TOC_PERCENTAGE: u16 = 25;

    [
        Constraint::Percentage(TOC_PERCENTAGE),
        Constraint::Percentage(100 - TOC_PERCENTAGE),
    ]
};

// Search parallelization thresholds.
/// Minimum number of lines before search work can be parallelized.
const MIN_LINES_FOR_PARALLEL_SEARCH: usize = 1500;
/// Minimum number of lines each worker should handle.
const PARALLEL_SEARCH_MIN_LINES_PER_WORKER: usize = 250;

/// Application mode for the current UI state.
///
/// Controls what is displayed and how the user input is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode
{
    /// Normal reading mode, default state.
    Normal,
    /// Help overlay being displayed.
    Help,
    /// Search mode, accepting search input.
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
        const SHOULD_SHOW_TOC = 1 << 1;
        /// Whether search yields no results
        const HAS_NO_RESULTS = 1 << 2;
        /// Are we searching case-sensitively?
        const IS_CASE_SENSITIVE = 1 << 3;
        /// Are we searching with regex?
        const IS_USING_REGEX = 1 << 4;
    }
}

impl Default for AppStateFlags
{
    fn default() -> Self
    {
        Self::SHOULD_RUN
    }
}

/// Manages the core state and UI logic.
///
/// This includes rendering the document, processing user input, and handling
/// interactions like scrolling, searching, navigation and graceful shutdown.
pub struct App
{
    // Core document
    /// Content of the currently loaded RFC.
    pub rfc_content: Box<str>,
    /// Number of the currently loaded RFC.
    pub rfc_number: RfcNum,
    /// Table of contents panel for the current document.
    pub rfc_toc_panel: TocPanel,
    /// Total line number of the content.
    pub rfc_line_number: LineNumber,

    // Navigation
    /// Current scroll position in the document.
    pub current_scroll_pos: LineNumber,

    // UI state
    /// Current application mode.
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
    /// Cursor position in the search text (byte index).
    pub query_cursor_pos: usize,
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
    /// A new `App` instance initialized for the specified RFC.
    #[must_use]
    pub fn new(rfc_number: RfcNum, rfc_content: Box<str>) -> Self
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
        // Keep confirmed highlights in Normal mode, but hide them while
        // actively editing in Search mode to avoid stale visuals.
        let should_show_search_highlights =
            self.mode != AppMode::Search && self.has_search_results();

        let lines: Vec<Line> = self
            .rfc_content
            .lines()
            .enumerate()
            .map(|(line_num, line_str)| {
                let is_title = self.rfc_toc_panel
                                         .entries()
                                         .binary_search_by(|entry| entry.line_number.cmp(&line_num))
                                         .is_ok();

                if should_show_search_highlights
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
    /// A `Line` with appropriate highlighting applied.
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

            if let Some(mtc) = line_str.get(start..end)
            {
                spans.push(Span::styled(mtc, MATCH_HIGHLIGHT_STYLE));
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
            .contains(AppStateFlags::SHOULD_SHOW_TOC)
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
        let paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((y, 0));

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
            Line::from("Ctrl+C: Toggle case sensitivity"),
            Line::from("Ctrl+R: Toggle regex search"),
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
        /// Search prompt prefix.
        const SEARCH_PROMPT: &str = "/";
        /// Prefix length for the search prompt ("/").
        #[allow(
            clippy::cast_possible_truncation,
            reason = "Terminal width is excpected to fit in u16 bounds"
        )]
        const SEARCH_PREFIX_LENGTH: u16 = SEARCH_PROMPT.len() as _;
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
        /// Border width for cursor position calculation.
        const SEARCH_BOX_BORDER_WIDTH: u16 = 1;

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

        let text = Text::from(format!("{}{}", SEARCH_PROMPT, self.query_text));

        let search_box = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Search")
                    .style(Style::default()),
            )
            .style(Style::default());

        frame.render_widget(search_box, area);

        // Calculate cursor position
        // The cursor should be after the "/" prefix and at the current position
        // in the query text
        let cursor_x = area
            .x
            .saturating_add(SEARCH_BOX_BORDER_WIDTH)
            .saturating_add(SEARCH_PREFIX_LENGTH)
            .saturating_add(
                self.query_text
                    .get(..self.query_cursor_pos)
                    .map_or(0, |before_cursor| before_cursor.chars().count())
                    .try_into()
                    .unwrap_or(0),
            );
        let cursor_y = area
            .y
            .saturating_add(SEARCH_BOX_BORDER_WIDTH);

        // Set cursor position
        frame.set_cursor_position((cursor_x, cursor_y));
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
        // Build text content first so sections are sized to their actual
        // content.
        let progress_text = self.build_progress_text();
        let left_text = format!("RFC {} | {}", self.rfc_number, progress_text);
        let mode_text = self.get_mode_text();
        let help_text = self.get_help_text();

        #[expect(
            clippy::cast_possible_truncation,
            reason = "Statusbar text lengths fit in u16"
        )]
        let left_len = left_text.chars().count() as u16;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Statusbar text lengths fit in u16"
        )]
        let right_len = help_text.chars().count() as u16;

        let [left_section, middle_section, right_section] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_len),
                Constraint::Fill(1),
                Constraint::Length(right_len),
            ])
            .flex(Flex::SpaceBetween)
            .areas(area);

        // Left section
        let left_statusbar = Paragraph::new(left_text)
            .style(STATUSBAR_STYLE)
            .alignment(Alignment::Left);
        frame.render_widget(left_statusbar, left_section);

        // Middle section
        let middle_statusbar = Paragraph::new(mode_text)
            .style(STATUSBAR_STYLE)
            .alignment(Alignment::Center);
        frame.render_widget(middle_statusbar, middle_section);

        // Right section
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
    fn get_mode_text(&self) -> Cow<'static, str>
    {
        match self.mode
        {
            AppMode::Normal
                if self
                    .app_state
                    .contains(AppStateFlags::SHOULD_SHOW_TOC) =>
            {
                Cow::Borrowed("NORMAL (ToC)")
            },
            AppMode::Normal => Cow::Borrowed("NORMAL"),
            AppMode::Help => Cow::Borrowed("HELP"),
            AppMode::Search => Cow::Owned(self.get_search_mode_text()),
        }
    }

    /// Builds the search mode text for the statusbar.
    /// Includes case sensitivity and regex flags.
    ///
    /// # Returns
    ///
    /// A string containing the search mode text.
    fn get_search_mode_text(&self) -> String
    {
        const EMPTY_BOX_CHAR: char = '☐';
        const CHECKED_BOX_CHAR: char = '☑';

        let case_char = if self
            .app_state
            .contains(AppStateFlags::IS_CASE_SENSITIVE)
        {
            CHECKED_BOX_CHAR
        }
        else
        {
            EMPTY_BOX_CHAR
        };

        let regex_char = if self
            .app_state
            .contains(AppStateFlags::IS_USING_REGEX)
        {
            CHECKED_BOX_CHAR
        }
        else
        {
            EMPTY_BOX_CHAR
        };

        format!("SEARCH | C:{case_char} R:{regex_char}")
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
        let progress_percentage = {
            let last_line_pos = self.rfc_line_number.saturating_sub(1);

            (self.current_scroll_pos * 100)
                .checked_div(last_line_pos)
                .unwrap_or(if self.rfc_line_number > 0 { 100 } else { 0 })
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
        // Don't show the previous search's info when entering a new search.
        if self.mode == AppMode::Search || !self.has_search_results()
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
                    .contains(AppStateFlags::SHOULD_SHOW_TOC) =>
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
            .toggle(AppStateFlags::SHOULD_SHOW_TOC);
    }

    /// Toggles case sensitivity for searches.
    ///
    /// If case sensitivity is enabled, searches will be case-sensitive.
    /// If disabled, searches will be case-insensitive.
    pub fn toggle_case_sensitivity(&mut self)
    {
        self.app_state
            .toggle(AppStateFlags::IS_CASE_SENSITIVE);
    }

    /// Toggles regex mode for searches.
    ///
    /// If regex mode is enabled, searches will interpret the query as a regex
    /// pattern.
    pub fn toggle_regex_mode(&mut self)
    {
        self.app_state
            .toggle(AppStateFlags::IS_USING_REGEX);
    }

    /// Enters search mode, clearing any previous search.
    pub fn enter_search_mode(&mut self)
    {
        self.mode = AppMode::Search;
        self.query_text.clear(); // Start with an empty search
        self.query_cursor_pos = 0;

        // Show cursor when entering search mode
        if let Err(error) = execute!(stdout(), Show)
        {
            warn!("Failed to show cursor: {error}");
        }
    }

    /// Exits search mode and returns to normal mode.
    pub fn exit_search_mode(&mut self)
    {
        self.mode = AppMode::Normal;

        // Hide cursor when exiting search mode
        if let Err(error) = execute!(stdout(), Hide)
        {
            warn!("Failed to hide cursor: {error}");
        }
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

    /// Adds a character to the search text at cursor position.
    ///
    /// # Arguments
    ///
    /// * `ch` - The character to add
    pub fn add_search_char(&mut self, ch: char)
    {
        self.query_text
            .insert(self.query_cursor_pos, ch);
        self.query_cursor_pos = self
            .query_cursor_pos
            .saturating_add(ch.len_utf8());
    }

    /// Removes the character before the cursor in the search text.
    pub fn remove_search_char(&mut self)
    {
        if self.query_cursor_pos > 0
        {
            self.move_search_cursor_left();
            self.delete_search_char();
        }
    }

    /// Deletes the character front of the cursor in the search text.
    pub fn delete_search_char(&mut self)
    {
        if self.query_cursor_pos < self.query_text.len()
        {
            self.query_text.remove(self.query_cursor_pos);
        }
    }

    /// Moves the search cursor left by one character.
    pub fn move_search_cursor_left(&mut self)
    {
        if self.query_cursor_pos > 0
        {
            // Find the previous character boundary
            let mut pos = self.query_cursor_pos.saturating_sub(1);
            while pos > 0 && !self.query_text.is_char_boundary(pos)
            {
                pos = pos.saturating_sub(1);
            }
            self.query_cursor_pos = pos;
        }
    }

    /// Moves the search cursor right by one character.
    pub fn move_search_cursor_right(&mut self)
    {
        if self.query_cursor_pos < self.query_text.len()
        {
            let mut pos = self.query_cursor_pos.saturating_add(1);
            while pos < self.query_text.len() &&
                !self.query_text.is_char_boundary(pos)
            {
                pos = pos.saturating_add(1);
            }
            self.query_cursor_pos = pos;
        }
    }

    /// Moves the search cursor to the start of the text.
    pub const fn move_search_cursor_home(&mut self)
    {
        self.query_cursor_pos = 0;
    }

    /// Moves the search cursor to the end of the text.
    pub const fn move_search_cursor_end(&mut self)
    {
        self.query_cursor_pos = self.query_text.len();
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

        let is_case_sensitive = self
            .app_state
            .contains(AppStateFlags::IS_CASE_SENSITIVE);
        let is_regex = self
            .app_state
            .contains(AppStateFlags::IS_USING_REGEX);

        let Some(regex) = get_compiled_regex(
            self.query_text.clone(),
            is_case_sensitive,
            is_regex,
        )
        else
        {
            self.app_state
                .insert(AppStateFlags::HAS_NO_RESULTS);
            return;
        };

        // Compute all search matches first, then commit to app state
        // atomically.
        let search_results: Vec<(LineNumber, Vec<MatchSpan>)> =
            collect_search_matches(&regex, &self.rfc_content);

        self.query_match_line_nums
            .reserve(search_results.len());
        self.query_matches
            .reserve(search_results.len());

        for (line_num, matches_in_line) in search_results
        {
            self.query_match_line_nums.push(line_num);
            self.query_matches
                .insert(line_num, matches_in_line);
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
            rfc_number: NonZeroU16::new(1).expect("its non-zero"),
            rfc_toc_panel: TocPanel::default(),
            rfc_line_number: 0,
            current_scroll_pos: 0,
            mode: AppMode::Normal,
            app_state: AppStateFlags::default(),
            guard,
            query_text: String::with_capacity(QUERY_TEXT_INITIAL_CAPACITY),
            query_cursor_pos: 0,
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
/// A new rectangle positioned in the center of the parent.
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

/// Search execution strategy for collecting query matches.
#[derive(Debug, Clone, Copy)]
enum SearchStrategy
{
    /// Process search linearly on a single thread.
    Serial,
    /// Process search using multiple workers.
    Parallel
    {
        /// Number of worker threads to spawn.
        worker_count: usize,
    },
}

/// Collects all search matches for the given content.
///
/// Uses bounded parallelism for larger documents and falls back to serial
/// processing for small documents or if a worker panics.
///
/// # Arguments
///
/// * `regex` - The regex to search with
/// * `content` - The content to search in
///
/// # Returns
///
/// An array of 2-tuples, where each tuple contains a line number and a vector
/// of match spans for that line.
fn collect_search_matches(
    regex: &Regex,
    content: &str,
) -> Vec<(LineNumber, Vec<MatchSpan>)>
{
    let lines: Vec<&str> = content.lines().collect();

    let worker_count = match determine_search_strategy(lines.len())
    {
        SearchStrategy::Serial =>
        {
            return collect_search_matches_serial(regex, &lines, 0);
        },
        SearchStrategy::Parallel { worker_count } => worker_count,
    };

    // Assign each worker a contiguous chunk of lines.
    let chunk_size = lines.len().div_ceil(worker_count);

    let parallel_result: Option<Vec<(LineNumber, Vec<MatchSpan>)>> =
        thread::scope(|scope| {
            let mut handles = Vec::with_capacity(worker_count);

            for (chunk_index, chunk) in lines.chunks(chunk_size).enumerate()
            {
                let line_offset = chunk_index.saturating_mul(chunk_size);
                handles.push(scope.spawn(move || {
                    collect_search_matches_serial(regex, chunk, line_offset)
                }));
            }

            let mut all_matches: Vec<(LineNumber, Vec<MatchSpan>)> =
                Vec::with_capacity(handles.len());

            for handle in handles
            {
                match handle.join()
                {
                    Ok(mut chunk_matches) =>
                    {
                        all_matches.append(&mut chunk_matches);
                    },
                    Err(_) => return None,
                }
            }

            Some(all_matches)
        });

    parallel_result
        // Fallback to serial processing if any worker panicked.
        .unwrap_or_else(|| collect_search_matches_serial(regex, &lines, 0))
}

/// Collects search matches line-by-line in a serial pass.
///
/// # Arguments
///
/// * `regex` - The regex to search with
/// * `lines` - The lines to search through
/// * `line_offset` - The line number offset to apply to the results (used for
///   parallel chunks)
///
/// # Returns
///
/// An array of 2-tuples, where each tuple contains a line number and a vector
/// of match spans for that line.
fn collect_search_matches_serial(
    regex: &Regex,
    lines: &[&str],
    line_offset: LineNumber,
) -> Vec<(LineNumber, Vec<MatchSpan>)>
{
    let mut results = Vec::new();

    for (relative_line_num, line) in lines.iter().enumerate()
    {
        let mut matches_in_line: Vec<MatchSpan> = Vec::new();
        for r#match in regex.find_iter(line)
        {
            matches_in_line.push(r#match.range());
        }

        if !matches_in_line.is_empty()
        {
            // Sort ranges defensively to keep deterministic highlight order.
            matches_in_line.sort_unstable_by_key(|span: &MatchSpan| span.start);
            matches_in_line.shrink_to_fit();

            results.push((
                line_offset.saturating_add(relative_line_num),
                matches_in_line,
            ));
        }
    }

    results
}

/// Determines whether search should run serially or in parallel.
///
/// # Arguments
///
/// * `total_lines` - The total number of lines in the document to search
///   through
///
/// # Returns
///
/// * [`SearchStrategy::Serial`] if the document is small or if parallelism is
///   not available
/// * [`SearchStrategy::Parallel`] with the number of worker threads to use for
///   larger documents
fn determine_search_strategy(total_lines: usize) -> SearchStrategy
{
    if total_lines < MIN_LINES_FOR_PARALLEL_SEARCH
    {
        return SearchStrategy::Serial;
    }

    let Ok(available_workers) =
        thread::available_parallelism().map(std::num::NonZeroUsize::get)
    else
    {
        return SearchStrategy::Serial;
    };

    let line_limited_workers =
        (total_lines / PARALLEL_SEARCH_MIN_LINES_PER_WORKER).max(1);

    let worker_count = available_workers.min(line_limited_workers);

    if worker_count <= 1
    {
        // 1 worker ain't making sense for parallelism, just do it serially.
        SearchStrategy::Serial
    }
    else
    {
        SearchStrategy::Parallel { worker_count }
    }
}

/// Gets a compiled regex for the given query, case sensitivity, and regex mode.
/// Uses caching to avoid recompiling the same regex multiple times.
///
/// # Arguments
///
/// * `query` - The search query string
/// * `is_case_sensitive` - Whether the search is case sensitive
/// * `is_regex` - Whether the query is a regex
///
/// # Returns
///
/// A compiled `Regex` if the query is valid, or `None` if invalid.
#[cached(
    size = 20,
    key = "String",
    convert = r#"{ format!("{}-{}-{}", query, is_case_sensitive, is_regex) }"#
)]
fn get_compiled_regex(
    query: String,
    is_case_sensitive: bool,
    is_regex: bool,
) -> Option<Regex>
{
    let pattern = if is_regex
    {
        query
    }
    else
    {
        regex::escape(&query)
    };

    let case_prefix = if is_case_sensitive { "" } else { "(?i)" };

    Regex::new(&format!("{case_prefix}{pattern}")).ok()
}
