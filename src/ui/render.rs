//! Rendering logic for the RFC reader application.
//!
//! Contains all UI drawing code including the main document view,
//! help overlay, search box, statusbar, and too-small message.
use std::borrow::Cow;

use crossterm::terminal::size;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::app::{App, AppMode, AppStateFlags};
use crate::types::LineNumber;

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

impl App
{
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
        matches: &[std::ops::Range<usize>],
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
        #[expect(
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
