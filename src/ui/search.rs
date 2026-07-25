//! Search engine for finding matches in RFC documents.
//!
//! Provides the core search functionality including regex compilation,
//! serial and parallel search strategies, and result collection.
use std::num::NonZeroUsize;
use std::thread;

use cached::macros::cached;
use regex::Regex;

use crate::types::{LineNumber, MatchSpan};

// Search parallelization thresholds.
/// Minimum number of lines before search work can be parallelized.
const MIN_LINES_FOR_PARALLEL_SEARCH: usize = 1500;
/// Minimum number of lines each worker should handle.
const PARALLEL_SEARCH_MIN_LINES_PER_WORKER: usize = 250;

/// Search execution strategy for collecting query matches.
#[derive(Debug, Clone, Copy)]
pub enum SearchStrategy
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
pub(super) fn collect_search_matches(
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
        thread::available_parallelism().map(NonZeroUsize::get)
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
    max_size = 20,
    key = "String",
    convert = r#"{ format!("{}-{}-{}", query, is_case_sensitive, is_regex) }"#
)]
pub(super) fn get_compiled_regex(
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
