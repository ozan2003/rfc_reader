//! Table of Contents Panel
//!
//! A panel that displays and manages a table of contents for an RFC document.
//!
//! Provides navigation capabilities and tracks the currently selected entry.
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState},
};
use regex::Regex;

use super::app::LineNumber;

const TOC_HIGHLIGHT_STYLE: Style = Style::new()
    .fg(Color::LightYellow)
    .add_modifier(Modifier::BOLD);

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
    pub line_number: LineNumber,
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
        let entries = parsing::parse_toc(content);
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
    pub fn render(&mut self, frame: &mut Frame, area: Rect)
    {
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|entry| ListItem::new(Line::raw(&entry.title)))
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Table of Contents")
                    .title_alignment(Alignment::Center),
            )
            .highlight_style(TOC_HIGHLIGHT_STYLE)
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.state);
    }

    /// Moves the selection to the next entry.
    pub fn next(&mut self)
    {
        if let Some(i) = self.state.selected()
        {
            self.state.select(Some(i.saturating_add(1)));
        }
    }

    /// Moves the selection to the previous entry.
    pub fn previous(&mut self)
    {
        if let Some(i) = self.state.selected()
        {
            self.state.select(Some(i.saturating_sub(1)));
        }
    }

    /// Returns the line number of the currently selected entry.
    ///
    /// # Returns
    ///
    /// The line number of the selected entry, or `None` if no entry is selected
    /// or the entries list is empty.
    pub fn selected_line(&self) -> Option<LineNumber>
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

pub(crate) mod parsing
{
    use super::{LineNumber, Regex, TocEntry};
    use std::sync::LazyLock;

    // Static regex patterns for better performance
    //
    // Note: Don't trim the leading whitespace or eat the other chars
    // before beginning of the line so that we can distinguish the actual
    // ToC entries from the section headings by preserving the indentation.
    static TOC_HEADER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        let toc_entries = [
            r"(?:Table of Contents|Contents)", // Standard header
            r"(?:TABLE OF CONTENTS)",          // All caps variant
            r"(?:\d+\.?\s+Table of Contents)", // Numbered ToC section
        ];
        let pattern = format!("^({})$", toc_entries.join("|"));
        Regex::new(&pattern).expect("Invalid TOC header regex")
    });

    static TOC_ENTRY_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        vec![
            // Standard format with dots: "1. Introduction..................5"
            Regex::new(r"^(\d+(?:\.\d+)*\.?)\s+(.*?)(?:\.{2,}\s*\d+)?$")
                .expect("Invalid TOC entry regex"),
            // Appendix format: "Appendix A. Example"
            Regex::new(r"^Appendix\s+([A-Z])\.?\s+(.*?)(?:\.{2,}\s*\d+)?$")
                .expect("Invalid appendix regex"),
        ]
    });

    static SECTION_HEADING_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\d+\.\s+\w+").expect("Invalid section heading regex"));

    /// Parses the document by existing `ToC`.
    ///
    /// # Arguments
    ///
    /// * `content` - The document content to parse
    ///
    /// # Returns
    ///
    /// A vector of `TocEntry` instances representing the document's structure
    /// or `None` if no `ToC` is found.
    fn parse_toc_existing(content: &str) -> Option<Vec<TocEntry>>
    {
        let lines: Vec<&str> = content.lines().collect();

        // Find ToC start
        let start_index = find_toc_start(&lines, &TOC_HEADER_REGEX)?;

        // Process ToC entries
        let entries = extract_toc_entries(
            &lines,
            start_index,
            &TOC_ENTRY_PATTERNS,
            &SECTION_HEADING_REGEX,
        );

        if entries.is_empty()
        {
            None
        }
        else
        {
            Some(entries)
        }
    }

    /// Find the start of `ToC` section.
    ///
    /// # Arguments
    ///
    /// * `lines` - The lines of the document
    /// * `toc_regex` - The regex to find the `ToC` header
    ///
    /// # Returns
    ///
    /// The index of the start of the `ToC` section, or `None` if no `ToC` is
    /// found.
    fn find_toc_start(lines: &[&str], toc_regex: &Regex) -> Option<LineNumber>
    {
        lines
            .iter()
            .enumerate()
            .find_map(|(index, line)| {
                if toc_regex.is_match(line.trim())
                {
                    Some(index + 1) // Skip the `ToC` header line
                }
                else
                {
                    None
                }
            })
    }

    /// Extract `ToC` entries from content.
    ///
    /// # Arguments
    ///
    /// * `lines` - The lines of the document
    /// * `start_index` - The index of the start of the `ToC` section
    /// * `toc_entry_patterns` - The regex patterns to find `ToC` entries
    /// * `section_heading` - The regex to find section headings
    ///
    /// # Returns
    ///
    /// A vector of `TocEntry` instances representing the document's structure
    fn extract_toc_entries(
        lines: &[&str],
        start_index: LineNumber,
        toc_entry_patterns: &[Regex],
        section_heading: &Regex,
    ) -> Vec<TocEntry>
    {
        let mut entries = Vec::new();
        let mut consecutive_empty_lines = 0;
        let mut has_found_entries = false;
        let mut lines_without_entries = 0;

        for (index, trimmed_line) in lines
            .iter()
            .enumerate()
            .skip(start_index)
            .map(|(i, line)| (i, line.trim_end()))
        {
            // Check stopping conditions
            if should_stop_parsing(
                trimmed_line,
                section_heading,
                toc_entry_patterns,
                has_found_entries,
                &mut consecutive_empty_lines,
                &mut lines_without_entries,
            )
            {
                break;
            }

            // Try to match and extract entries
            if let Some(entry) = try_extract_entry(trimmed_line, toc_entry_patterns, lines, index)
            {
                has_found_entries = true;
                entries.push(entry);
            }
        }

        entries
    }

    /// Check if we should stop parsing the `ToC`
    ///
    /// # Arguments
    ///
    /// * `trimmed_line` - The trimmed line to check
    /// * `section_heading` - The regex to find section headings
    /// * `toc_entry_patterns` - The regex patterns to find `ToC` entries
    /// * `has_found_entries` - Whether we have found any entries
    /// * `consecutive_empty_lines` - The number of consecutive empty lines
    /// * `lines_without_entries` - The number of lines without entries
    ///
    /// # Returns
    ///
    /// A boolean indicating whether we should stop parsing the `ToC`
    fn should_stop_parsing(
        trimmed_line: &str,
        section_heading: &Regex,
        toc_entry_patterns: &[Regex],
        has_found_entries: bool,
        consecutive_empty_lines: &mut u8,
        lines_without_entries: &mut u8,
    ) -> bool
    {
        // 1. Check for section headings outside ToC
        let does_look_like_section = section_heading.is_match(trimmed_line);
        let is_matching_toc_pattern = toc_entry_patterns
            .iter()
            .any(|re| re.is_match(trimmed_line));

        if does_look_like_section && !is_matching_toc_pattern && has_found_entries
        {
            return true;
        }

        // 2. Check empty lines
        // Multiple consecutive empty lines indicate the end of the ToC
        if trimmed_line.is_empty()
        {
            *consecutive_empty_lines += 1;
            if *consecutive_empty_lines >= 2 && has_found_entries
            {
                return true;
            }
        }
        else
        {
            *consecutive_empty_lines = 0;
        }

        // 3. Check timeout for entries
        if !has_found_entries
        {
            *lines_without_entries += 1;
            if *lines_without_entries > 20
            {
                return true;
            }
        }

        false
    }

    /// Try to extract a `ToC` entry from a line
    ///
    /// # Arguments
    ///
    /// * `trimmed_line` - The trimmed line to check
    /// * `toc_entry_patterns` - The regex patterns to find `ToC` entries
    /// * `lines` - The lines of the document
    /// * `index` - The index of the line
    ///
    /// # Returns
    ///
    /// A `TocEntry` instance representing the extracted entry, or `None` if no
    /// entry is found.
    fn try_extract_entry(
        trimmed_line: &str,
        toc_entry_patterns: &[Regex],
        lines: &[&str],
        index: LineNumber,
    ) -> Option<TocEntry>
    {
        for entry_regex in toc_entry_patterns
        {
            if let Some(caps) = entry_regex.captures(trimmed_line)
            {
                // Ensure the regex captures both the section number and the title
                if caps.len() >= 3
                {
                    let section_num = caps[1].trim();
                    let title = caps[2].trim();

                    // Find actual section in document
                    let section_pattern = format!(
                        r"^\s*{}\s+{}",
                        regex::escape(section_num),
                        regex::escape(title)
                    );

                    if let Ok(section_regex) = Regex::new(&section_pattern)
                    {
                        // Look for the section in the document after the ToC
                        for (line_number, doc_line) in lines.iter().enumerate().skip(index + 1)
                        {
                            if section_regex.is_match(doc_line)
                            {
                                return Some(TocEntry {
                                    title: format!("{section_num} {title}"),
                                    line_number,
                                });
                            }
                        }
                    }
                }
                break; // Stop checking patterns if one matched
            }
        }
        None
    }

    /// Parses the document content heuristically to extract a table of
    /// contents.
    ///
    /// Identifies section headers in RFC format (e.g., "1. Introduction") and
    /// capitalized headings as `ToC` entries.
    ///
    /// # Arguments
    ///
    /// * `content` - The document content to parse
    ///
    /// # Returns
    ///
    /// A vector of `TocEntry` instances representing the document's structure
    ///
    /// # Warning
    ///
    /// This function is not guaranteed to work correctly for all documents.
    /// It is intended to be used as a last resort when no existing `ToC` is
    /// found.
    fn parse_toc_heuristic(content: &str) -> Vec<TocEntry>
    {
        let mut entries = Vec::new();
        let mut section_pattern = false;

        for (line_number, line) in content.lines().enumerate()
        {
            let line = line.trim_end();

            // Check for section headers in typical RFC format
            if line.starts_with(|ch: char| ch.is_ascii_digit()) && line.contains('.')
            {
                let parts: Vec<&str> = line.splitn(2, '.').collect();
                if parts.len() == 2 && !parts[0].contains(' ')
                {
                    entries.push(TocEntry {
                        title: line.to_string(),
                        line_number,
                    });
                    section_pattern = true;
                }
            }
            // If we didn't find standard section patterns, look for capitalized headings
            else if !section_pattern && line.len() > 3 && line == line.to_uppercase()
            {
                entries.push(TocEntry {
                    title: line.to_string(),
                    line_number,
                });
            }
        }

        entries
    }

    /// Parses the document to extract a table of contents.
    ///
    /// # Arguments
    ///
    /// * `content` - The document content to parse
    ///
    /// # Returns
    ///
    /// A vector of `TocEntry` instances representing the document's structure
    pub(crate) fn parse_toc(content: &str) -> Vec<TocEntry>
    {
        // First, look for existing ToC. Otherwise, use heuristic.
        parse_toc_existing(content).unwrap_or_else(|| parse_toc_heuristic(content))
    }
}
