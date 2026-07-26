//! Core application logic and app state management.
//!
//! Provides the central application state and handles UI rendering and user
//! input. This includes features such as document scrolling, searching,
//! and navigation.
use std::collections::HashMap;
use std::io::stdout;
use std::num::NonZeroU16;

use bitflags::bitflags;
use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::SetTitle;
use log::warn;

use super::search::{collect_search_matches, get_compiled_regex};
use super::toc_panel::TocPanel;
use crate::types::{LineNumber, MatchSpan, RfcNum};

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
    pub(super) const fn has_search_results(&self) -> bool
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

        Self {
            rfc_content: Box::from(""),
            rfc_number: NonZeroU16::new(1).expect("its non-zero"),
            rfc_toc_panel: TocPanel::default(),
            rfc_line_number: 0,
            current_scroll_pos: 0,
            mode: AppMode::Normal,
            app_state: AppStateFlags::default(),
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
