//! Manages the RFC Table of Contents panel.
//!
//! Displays, navigates, and tracks selection for RFC document entries.
use std::collections::HashSet;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use regex::Regex;
use textwrap::wrap;

use crate::types::LineNumber;

/// Style for each individual `ToC` entry.
const TOC_HIGHLIGHT_STYLE: Style = Style::new()
    .fg(Color::LightYellow)
    .add_modifier(Modifier::BOLD);

/// Style for the `ToC` border.
const TOC_BORDER_STYLE: Style = Style::new().fg(Color::Gray);

/// Symbol used to highlight the currently selected `ToC` entry.
const TOC_HIGHLIGHT_SYMBOL: &str = "> ";

/// Represents a table of contents entry.
///
/// Contains a title and its document line number.
#[derive(Debug, Clone, Default)]
pub struct TocEntry
{
    /// The title text of the section.
    pub title: Box<str>,
    /// The line number where this section appears in the document.
    pub line_number: LineNumber,
}

/// Panel that displays and manages a table of contents.
///
/// Provides navigation capabilities and tracks the currently selected entry.
#[derive(Default)]
pub struct TocPanel
{
    /// Collection of table of contents entries.
    entries: Vec<TocEntry>,
    /// Set of line numbers corresponding to `ToC` titles for $O(1)$ lookup.
    title_line_numbers: HashSet<LineNumber>,
    /// Current selection state.
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
    /// A new `TocPanel` instance.
    pub fn new(content: &str) -> Self
    {
        let entries = parsing::parse_toc(content);
        let title_line_numbers = entries
            .iter()
            .map(|entry| entry.line_number)
            .collect();
        let mut state = ListState::default();

        if !entries.is_empty()
        {
            state.select(Some(0));
        }

        Self {
            entries,
            title_line_numbers,
            state,
        }
    }

    /// Returns a slice of `ToC` entries, sorted by their first appearance.
    ///
    /// # Returns
    ///
    /// A slice of all `ToC` entries.
    pub fn entries(&self) -> &[TocEntry]
    {
        &self.entries
    }

    /// Renders the table of contents panel to the specified area.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render to
    /// * `area` - The area within the frame to render the panel
    pub fn render(&mut self, frame: &mut Frame, area: Rect)
    {
        // Long titles need to be wrapped to fit within the panel width.
        // 2 for the border
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "usize not expected to overflow"
        )]
        let wrap_width = usize::from(area.width)
            .saturating_sub(TOC_HIGHLIGHT_SYMBOL.len() + 2);

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|entry| {
                let wrapped_title = wrap(&entry.title, wrap_width)
                    .into_iter()
                    .map(Line::raw)
                    .collect::<Vec<Line>>();

                ListItem::new(wrapped_title)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::RIGHT)
                    .border_style(TOC_BORDER_STYLE)
                    .title("Contents")
                    .title_alignment(Alignment::Left)
                    .title_style(
                        Style::new()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
            )
            .highlight_style(TOC_HIGHLIGHT_STYLE)
            .highlight_symbol(TOC_HIGHLIGHT_SYMBOL);

        frame.render_stateful_widget(list, area, &mut self.state);
    }

    /// Moves the selection to the next entry.
    pub const fn next(&mut self)
    {
        if self.entries.is_empty()
        {
            return;
        }

        let max_index = self.entries.len().saturating_sub(1);
        if let Some(i) = self.state.selected()
        {
            let next_index = i.saturating_add(1);
            let bounded_index = if next_index > max_index
            {
                max_index
            }
            else
            {
                next_index
            };
            self.state.select(Some(bounded_index));
        }
    }

    /// Moves the selection to the previous entry.
    pub const fn previous(&mut self)
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
            .and_then(|i| self.entries.get(i))
            .map(|entry| entry.line_number)
    }

    /// Checks if the given line number corresponds to a table of contents
    /// title.
    ///
    /// # Arguments
    ///
    /// * `line_number` - The line number to check
    ///
    /// # Returns
    ///
    /// `true` if the line is a title, `false` otherwise.
    #[must_use]
    pub fn is_title_line(&self, line_number: LineNumber) -> bool
    {
        self.title_line_numbers
            .contains(&line_number)
    }
}

/// Specialized functions for parsing document content to extract a table of
/// contents.
pub mod parsing
{
    use std::collections::HashSet;
    use std::str::Lines;
    use std::sync::LazyLock;

    use super::{LineNumber, Regex, TocEntry};

    // Static regex patterns for better performance.

    /// Matches lines that announce the start of a table of contents block.
    ///
    /// Case-insensitive; supported variants:
    /// - `Table of Contents`
    /// - `Contents`
    /// - Numbered form like `3. Table of Contents`
    static TOC_HEADER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:\d+\.?\s+)?(?:Table of Contents|Contents)$")
            .expect("Invalid TOC header regex")
    });

    /// Patterns for individual `ToC` rows.
    ///
    /// Capture groups are intentionally consistent across patterns:
    /// - group 1: section label (for example `1.2` or `Appendix A`)
    /// - group 2: section title text, with any trailing page number stripped
    ///
    /// A trailing page number is optional and may be dot-delimited
    /// (".... 17") or whitespace-separated (" 17"); modern RFCs such as
    /// RFC 9000 omit it entirely.
    static TOC_ENTRY_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        vec![
            // Numbered entry, for example:
            // `1. Introduction..................5`
            // `2.1  Terminology`
            // `1   Introduction ........ 7`
            Regex::new(
                r"^\s*(\d+(?:\.\d+)*\.?)\s+(.+?)(?:\s*\.{2,}\s*\d+|\s+\d+)?\s*$",
            )
            .expect("Invalid TOC entry regex"),
            // Appendix entry, for example:
            // `Appendix A. Packet Format`
            // `APPENDIX A:  Examples & Scenarios .... 34`
            Regex::new(
                r"(?i)^\s*(Appendix\s+[A-Z])\.?:?\s+(.+?)(?:\s*\.{2,}\s*\d+|\s+\d+)?\s*$",
            )
            .expect("Invalid appendix regex"),
        ]
    });

    /// Matches numbered section headings at the start of the line.
    ///
    /// Used together with [`PAGE_NUMBER_REGEX`] as a stop signal: a column-0
    /// numbered heading with no page number is the start of the body, not a
    /// `ToC` row.
    static SECTION_HEADING_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(\d+(?:\.\d+)*\.?)\s+\S")
            .expect("Invalid section heading regex")
    });

    /// Matches a trailing page number, dot-delimited or whitespace-separated.
    ///
    /// `ToC` rows carry a page number; body section headings do not. In
    /// formats where both start at column 0 (e.g. RFC 791) this is the only
    /// way to tell them apart.
    static PAGE_NUMBER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:\.{2,}\s*|\s+)\d+\s*$")
            .expect("Invalid page number regex")
    });

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
        // Skip the header line itself; entries begin on the following line.
        let header_index = find_toc_start(content.lines())?;
        let entries =
            extract_toc_entries(content, header_index.saturating_add(1));

        if entries.is_empty()
        {
            None
        }
        else
        {
            Some(entries)
        }
    }

    /// Find the index of the `ToC` header line.
    ///
    /// # Arguments
    ///
    /// * `lines` - The lines of the document
    ///
    /// # Returns
    ///
    /// The 0-based index of the `ToC` header, or `None` if no header is
    /// found.
    fn find_toc_start(lines: Lines<'_>) -> Option<LineNumber>
    {
        lines.enumerate().find_map(|(index, line)| {
            TOC_HEADER_REGEX
                .is_match(line.trim())
                .then_some(index)
        })
    }

    /// Extract `ToC` entries from content.
    ///
    /// # Arguments
    ///
    /// * `content` - The document content
    /// * `start_index` - The 0-based index of the first line after the `ToC`
    ///   header
    ///
    /// # Returns
    ///
    /// A vector of `TocEntry` instances representing the document's structure.
    fn extract_toc_entries(
        content: &str,
        start_index: LineNumber,
    ) -> Vec<TocEntry>
    {
        let mut entries = Vec::new();
        let mut consecutive_empty_lines = 0usize;
        let mut non_entry_lines = 0usize;

        for (index, line) in content
            .lines()
            .enumerate()
            .skip(start_index)
            .map(|(line_index, raw)| (line_index, raw.trim_end()))
        {
            if line.is_empty()
            {
                consecutive_empty_lines =
                    consecutive_empty_lines.saturating_add(1);
                if consecutive_empty_lines >= 5 && !entries.is_empty()
                {
                    break;
                }
                continue;
            }
            consecutive_empty_lines = 0;

            // A column-0 numbered heading with no page number is the start of
            // the body, not a `ToC` row.
            if !entries.is_empty() && is_body_heading(line)
            {
                break;
            }

            if let Some(entry) = try_extract_entry(content, line, index)
            {
                entries.push(entry);
            }
            else if entries.is_empty()
            {
                // Bail out if the header isn't followed by a real `ToC`.
                non_entry_lines = non_entry_lines.saturating_add(1);
                if non_entry_lines > 30
                {
                    break;
                }
            }
        }

        entries
    }

    /// Checks whether `line` is a body section heading rather than a `ToC`
    /// row.
    ///
    /// A body heading starts at column 0 (no leading whitespace), matches a
    /// numbered heading, and carries no page number. `ToC` rows in formats
    /// that don't indent (e.g. RFC 791) still carry a page number, so they
    /// are excluded.
    fn is_body_heading(line: &str) -> bool
    {
        SECTION_HEADING_REGEX.is_match(line) &&
            !PAGE_NUMBER_REGEX.is_match(line)
    }

    /// Try to extract a `ToC` entry from a line.
    ///
    /// # Arguments
    ///
    /// * `content` - The document content
    /// * `line` - The line to check
    /// * `index` - The 0-based index of the line
    ///
    /// # Returns
    ///
    /// A `TocEntry` instance representing the extracted entry, or `None` if no
    /// entry is found.
    fn try_extract_entry(
        content: &str,
        line: &str,
        index: LineNumber,
    ) -> Option<TocEntry>
    {
        for entry_regex in TOC_ENTRY_PATTERNS.iter()
        {
            if let Some(caps) = entry_regex.captures(line)
            {
                // Both capture groups must be present (group 0 is the full
                // match).
                if caps.len() >= 3
                {
                    let section_num = caps[1].trim();
                    let title = caps[2].trim();

                    if let Some(line_number) =
                        find_body_heading(content, index, section_num, title)
                    {
                        return Some(TocEntry {
                            title: format!("{section_num} {title}").into(),
                            line_number,
                        });
                    }
                }
                break; // Stop checking patterns if one matched
            }
        }
        None
    }

    /// Find the body heading line number for a `ToC` entry.
    ///
    /// Searches forward from the entry's line for a heading whose section
    /// label and title match.
    ///
    /// # Arguments
    ///
    /// * `content` - The document content
    /// * `after_index` - The 0-based index of the `ToC` entry line
    /// * `section_num` - The section label from the `ToC` (e.g. `1.2`)
    /// * `title` - The section title from the `ToC`
    ///
    /// # Returns
    ///
    /// The 0-based index of the matching body heading, or `None` if no match
    /// is found.
    fn find_body_heading(
        content: &str,
        after_index: LineNumber,
        section_num: &str,
        title: &str,
    ) -> Option<LineNumber>
    {
        for (line_number, doc_line) in content
            .lines()
            .enumerate()
            .skip(after_index.saturating_add(1))
        {
            let trimmed = doc_line.trim_start();
            let Some(rest) = trimmed.strip_prefix(section_num)
            else
            {
                continue;
            };
            // Strip any `: ` or `. ` separator that may follow the label (for
            // appendix entries) plus surrounding whitespace.
            let rest = rest.trim_start_matches(|ch: char| {
                ch.is_whitespace() || ch == ':' || ch == '.'
            });
            if rest.starts_with(title)
            {
                return Some(line_number);
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
    /// A vector of `TocEntry` instances representing the document's structure.
    ///
    /// # Warning
    ///
    /// This function is not guaranteed to work correctly for all documents.
    /// It is intended to be used as a last resort when no existing `ToC` is
    /// found.
    fn parse_toc_heuristic(content: &str) -> Vec<TocEntry>
    {
        let mut numbered = Vec::new();
        let mut all_caps = Vec::new();

        for (line_number, line) in content.lines().enumerate()
        {
            let line = line.trim_end();

            if is_numbered_heading(line)
            {
                numbered.push(TocEntry {
                    title: line.into(),
                    line_number,
                });
            }
            else if is_all_caps_heading(line)
            {
                all_caps.push(TocEntry {
                    title: line.into(),
                    line_number,
                });
            }
        }

        // Numbered headings are the reliable signal; fall back to all-caps
        // headings only when the document has none.
        if numbered.is_empty()
        {
            all_caps
        }
        else
        {
            numbered
        }
    }

    /// Checks whether `line` is a numbered section heading such as
    /// `1. Introduction` or `1.2  Terminology`.
    fn is_numbered_heading(line: &str) -> bool
    {
        if !line.starts_with(|ch: char| ch.is_ascii_digit())
        {
            return false;
        }

        let Some((label, _)) = line.split_once('.')
        else
        {
            return false;
        };

        !label.contains(' ') &&
            label
                .chars()
                .all(|ch: char| ch.is_ascii_digit())
    }

    /// Checks whether `line` looks like an all-caps section heading.
    ///
    /// Requires at least one alphabetic character, no lowercase letters, and
    /// no digits (to exclude "RFC 9999", page footers, and table numbers).
    /// Punctuation-only separators such as "-----" are rejected.
    fn is_all_caps_heading(line: &str) -> bool
    {
        let mut has_alpha = false;
        for ch in line.chars()
        {
            if ch.is_ascii_digit()
            {
                return false;
            }
            if ch.is_alphabetic()
            {
                has_alpha = true;
                if ch.is_lowercase()
                {
                    return false;
                }
            }
        }
        has_alpha && line.len() > 3
    }

    /// Parses the document to extract a table of contents.
    ///
    /// # Arguments
    ///
    /// * `content` - The document content to parse
    ///
    /// # Returns
    ///
    /// A vector of `TocEntry` instances representing the document's structure.
    pub fn parse_toc(content: &str) -> Vec<TocEntry>
    {
        // First, look for existing ToC. Otherwise, use heuristic.
        let mut entries = parse_toc_existing(content)
            .unwrap_or_else(|| parse_toc_heuristic(content));

        // Drop duplicate rows (the same section listed twice).
        let mut seen: HashSet<Box<str>> = HashSet::new();
        entries.retain(|entry| seen.insert(entry.title.clone()));

        entries
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_parse_toc_dot_leaders()
    {
        // RFC 2616-style: indented rows, dot leaders, body headings at
        // column 0 with no trailing dot on the top-level number.
        #[rustfmt::skip]
        let content = concat!(
            "RFC 9999\n\n",
            "Table of Contents\n\n",
            "   1   Introduction ...................................................7\n",
            "   1.1    Purpose......................................................8\n\n",
            "1 Introduction\n\n",
            "1.1 Purpose\n",
        );

        let entries = parsing::parse_toc(content);

        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.title.as_ref(), entry.line_number))
                .collect::<Vec<_>>(),
            vec![("1 Introduction", 7), ("1.1 Purpose", 9)],
        );
    }

    #[test]
    fn test_parse_toc_whitespace_page_numbers()
    {
        // No dot leaders: page number separated by plain whitespace. The
        // title must not swallow the page number (the review's H2 case).
        let content = concat!(
            "RFC 9999\n\n",
            "Table of Contents\n\n",
            "1. Alpha 3\n",
            "2. Beta 5\n\n",
            "1. Alpha\n\n",
            "2. Beta\n",
        );

        let entries = parsing::parse_toc(content);

        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.title.as_ref(), entry.line_number))
                .collect::<Vec<_>>(),
            vec![("1. Alpha", 7), ("2. Beta", 9)],
        );
    }

    #[test]
    fn test_parse_toc_no_page_numbers()
    {
        // RFC 9000-style: indented rows with no page number; the body
        // headings are identical except for the indentation.
        let content = concat!(
            "RFC 9999\n\n",
            "Table of Contents\n\n",
            "   1.  Overview\n",
            "     1.1.  Document Structure\n",
            "   2.  Streams\n\n",
            "1.  Overview\n\n",
            "1.1.  Document Structure\n\n",
            "2.  Streams\n",
        );

        let entries = parsing::parse_toc(content);

        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.title.as_ref(), entry.line_number))
                .collect::<Vec<_>>(),
            vec![
                ("1. Overview", 8),
                ("1.1. Document Structure", 10),
                ("2. Streams", 12),
            ],
        );
    }

    #[test]
    fn test_parse_toc_stops_at_body_heading()
    {
        // The review's H1 case: numbered list items that repeat later in the
        // body must not leak into the ToC.
        let content = concat!(
            "RFC 9999\n\n",
            "Table of Contents\n\n",
            "   1.  Alpha ............ 3\n",
            "   2.  Beta ............. 5\n\n",
            "1.  Alpha\n",
            "   Intro.\n\n",
            "   2.  Pick option A\n",
            "   3.  Pick option B\n\n",
            "2.  Beta\n\n",
            "   2.  Pick option A\n",
            "   3.  Pick option B\n",
        );

        let entries = parsing::parse_toc(content);

        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.title.as_ref(), entry.line_number))
                .collect::<Vec<_>>(),
            vec![("1. Alpha", 7), ("2. Beta", 13)],
        );
    }

    #[test]
    fn test_parse_toc_header_case_insensitive()
    {
        for header in ["TABLE OF CONTENTS", "Contents"]
        {
            let content = format!(
                "RFC 9999\n\n{header}\n\n   1.  Alpha .... 2\n\n1.  Alpha\n",
            );

            let entries = parsing::parse_toc(&content);

            assert_eq!(entries.len(), 1);
            let entry = entries.first().expect("one entry");
            assert_eq!(entry.title.as_ref(), "1. Alpha");
            assert_eq!(entry.line_number, 6);
        }
    }

    #[test]
    fn test_parse_toc_heuristic_rejects_punctuation()
    {
        // No ToC header, so the heuristic runs. Separator lines must not
        // become entries.
        let content = "Intro\n\n-----\n\n1. Alpha\n\n-----\n\n2. Beta\n";

        let entries = parsing::parse_toc(content);

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.title.as_ref())
                .collect::<Vec<_>>(),
            vec!["1. Alpha", "2. Beta"],
        );
    }

    #[test]
    fn test_parse_toc_dedupes_duplicate_rows()
    {
        let content = concat!(
            "RFC 9999\n\n",
            "Table of Contents\n\n",
            "   1.  Alpha .... 2\n",
            "   1.  Alpha .... 2\n\n",
            "1.  Alpha\n",
        );

        let entries = parsing::parse_toc(content);

        assert_eq!(entries.len(), 1);
        let entry = entries.first().expect("one entry");
        assert_eq!(entry.title.as_ref(), "1. Alpha");
    }

    #[test]
    fn test_toc_navigation_bounds()
    {
        let mut panel = TocPanel {
            entries: vec![
                TocEntry {
                    title: "Section 1".into(),
                    line_number: 10,
                },
                TocEntry {
                    title: "Section 2".into(),
                    line_number: 20,
                },
            ],
            title_line_numbers: [10, 20].into_iter().collect(),
            state: ListState::default(),
        };
        panel.state.select(Some(0));

        assert_eq!(panel.selected_line(), Some(10));
        panel.next();
        assert_eq!(panel.selected_line(), Some(20));
        // Calling next at the end must not overflow or lose selection
        panel.next();
        assert_eq!(panel.selected_line(), Some(20));

        panel.previous();
        assert_eq!(panel.selected_line(), Some(10));
        // Calling previous at start must not underflow
        panel.previous();
        assert_eq!(panel.selected_line(), Some(10));
    }

    #[test]
    fn test_toc_empty_panel_navigation()
    {
        let mut panel = TocPanel::default();
        assert_eq!(panel.selected_line(), None);
        panel.next();
        assert_eq!(panel.selected_line(), None);
        panel.previous();
        assert_eq!(panel.selected_line(), None);
    }

    #[test]
    fn test_is_title_line()
    {
        let panel = TocPanel {
            entries: vec![TocEntry {
                title: "Section 1".into(),
                line_number: 42,
            }],
            title_line_numbers: std::iter::once(42).collect(),
            state: ListState::default(),
        };

        assert!(panel.is_title_line(42));
        assert!(!panel.is_title_line(0));
        assert!(!panel.is_title_line(43));
    }
}
