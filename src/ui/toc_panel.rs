//! Table of Contents Panel
//!
//! A panel that displays and manages a table of contents for an RFC document.
//!
//! Provides navigation capabilities and tracks the currently selected entry.
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState},
};

/// Represents an entry in the table of contents.
///
/// Each entry contains a title and its corresponding line number
/// in the document content.
#[derive(Debug, Clone)]
pub struct TocEntry
{
    /// The title text of the section
    pub title: String,
    /// The line number where this section appears in the document
    pub line_number: usize,
}

/// Panel that displays and manages a table of contents.
///
/// Provides navigation capabilities and tracks the currently selected entry.
pub struct TocPanel
{
    /// Collection of table of contents entries
    entries: Vec<TocEntry>,
    /// Current selection state
    state: ListState,
}

impl TocPanel
{
    /// Creates a new `TocPanel` from document content.
    ///
    /// Parses the content to extract a table of contents and initializes
    /// the selection state to the first entry if available.
    ///
    /// # Arguments
    ///
    /// * `content` - The document content to parse
    ///
    /// # Returns
    ///
    /// A new `TocPanel` instance
    pub fn new(content: &str) -> Self
    {
        let entries = parse_toc(content);
        let mut state = ListState::default();

        if !entries.is_empty()
        {
            state.select(Some(0));
        }

        Self { entries, state }
    }

    /// Renders the table of contents panel to the specified area.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render to
    /// * `area` - The area within the frame to render the panel
    pub fn render(&self, frame: &mut Frame, area: ratatui::layout::Rect)
    {
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|entry| ListItem::new(Line::from(entry.title.clone())))
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Table of Contents"),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.state.clone());
    }

    /// Moves the selection to the next entry, wrapping to the beginning if at
    /// the end.
    pub fn next(&mut self)
    {
        if let Some(i) = self.state.selected()
        {
            self.state.select(Some(i + 1));
        }
        else
        {
            self.state.select(Some(0));
        }
        /*let i = match self.state.selected()
        {
            Some(i) =>
            {
                if i >= self.entries.len() - 1
                {
                    0
                }
                else
                {
                    i + 1
                }
            }
            None => 0,
        };*/
        //self.state.select(Some(i));
    }

    /// Moves the selection to the previous entry, wrapping to the end if at the
    /// beginning.
    pub fn previous(&mut self)
    {
        let i = match self.state.selected()
        {
            Some(i) =>
            {
                if i == 0
                {
                    self.entries.len() - 1
                }
                else
                {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Returns the line number of the currently selected entry.
    ///
    /// # Returns
    ///
    /// The line number of the selected entry, or `None` if no entry is selected
    /// or the entries list is empty.
    pub fn selected_line(&self) -> Option<usize>
    {
        if self.entries.is_empty()
        {
            return None;
        }

        self.state
            .selected()
            .map(|i| self.entries[i].line_number)
    }
}

/// Parses the document content to extract a table of contents.
///
/// Identifies section headers in RFC format (e.g., "1. Introduction") and
/// capitalized headings as TOC entries.
///
/// # Arguments
///
/// * `content` - The document content to parse
///
/// # Returns
///
/// A vector of `TocEntry` instances representing the document's structure
fn parse_toc(content: &str) -> Vec<TocEntry>
{
    let mut entries = Vec::new();
    let mut section_pattern = false;

    for (idx, line) in content.lines().enumerate()
    {
        let line = line.trim();

        // Check for section headers in typical RFC format
        if line.starts_with(|c: char| c.is_ascii_digit()) && line.contains('.')
        {
            let parts: Vec<&str> = line.splitn(2, '.').collect();
            if parts.len() == 2 && !parts[0].contains(' ')
            {
                entries.push(TocEntry {
                    title: line.to_string(),
                    line_number: idx,
                });
                section_pattern = true;
            }
        }
        // If we didn't find standard section patterns, look for capitalized headings
        else if !section_pattern && line.len() > 3 && line == line.to_uppercase()
        {
            entries.push(TocEntry {
                title: line.to_string(),
                line_number: idx,
            });
        }
    }

    entries
}
