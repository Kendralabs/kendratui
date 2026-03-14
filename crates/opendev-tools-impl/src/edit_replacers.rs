//! 9-pass fuzzy matching chain for the edit tool.
//!
//! LLMs frequently produce slightly different whitespace, indentation, or escaping
//! in `old_content`. This module implements a chain of increasingly flexible
//! matching strategies, tried in order until one succeeds.
//!
//! Pass order (strictest to most flexible):
//! 1. Simple — exact string match
//! 2. LineTrimmed — trim leading/trailing whitespace per line
//! 3. BlockAnchor — match by first/last lines as anchors, similarity for middle
//! 4. WhitespaceNormalized — collapse all whitespace to single space
//! 5. IndentationFlexible — strip indentation, match stripped content
//! 6. EscapeNormalized — normalize escape sequences
//! 7. TrimmedBoundary — trim first/last lines of old_content
//! 8. ContextAware — use surrounding context lines to locate position
//! 9. MultiOccurrence — trimmed line-by-line match as last resort

use regex::Regex;
use std::sync::LazyLock;

/// Result of a successful fuzzy match: the actual substring found in the original.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// The actual content from the original file that matched.
    pub actual: String,
    /// Which replacer pass found the match (for logging).
    pub pass_name: &'static str,
}

/// Normalize line endings to `\n`.
pub fn normalize_line_endings(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Run the 9-pass replacer chain. Returns the actual substring in `original`
/// that matches `old_content`, or `None` if no pass succeeds.
pub fn find_match(original: &str, old_content: &str) -> Option<MatchResult> {
    let original = normalize_line_endings(original);
    let old_content = normalize_line_endings(old_content);

    #[allow(clippy::type_complexity)]
    let passes: &[(&str, fn(&str, &str) -> Option<String>)] = &[
        ("simple", simple_find),
        ("line_trimmed", line_trimmed_find),
        ("block_anchor", block_anchor_find),
        ("whitespace_normalized", whitespace_normalized_find),
        ("indentation_flexible", indentation_flexible_find),
        ("escape_normalized", escape_normalized_find),
        ("trimmed_boundary", trimmed_boundary_find),
        ("context_aware", context_aware_find),
        ("multi_occurrence", multi_occurrence_find),
    ];

    for &(name, finder) in passes {
        if let Some(actual) = finder(&original, &old_content) {
            return Some(MatchResult {
                actual,
                pass_name: name,
            });
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Pass 1: Simple (exact match)
// ---------------------------------------------------------------------------

fn simple_find(original: &str, old_content: &str) -> Option<String> {
    if original.contains(old_content) {
        Some(old_content.to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Pass 2: LineTrimmed — trim each line before comparing
// ---------------------------------------------------------------------------

fn line_trimmed_find(original: &str, old_content: &str) -> Option<String> {
    let old_lines: Vec<&str> = old_content.split('\n').collect();
    let old_trimmed: Vec<&str> = old_lines.iter().map(|l| l.trim()).collect();

    if old_trimmed.is_empty() || old_trimmed.iter().all(|l| l.is_empty()) {
        return None;
    }

    let original_lines: Vec<&str> = original.split('\n').collect();

    for i in 0..original_lines.len() {
        if original_lines[i].trim() != old_trimmed[0] {
            continue;
        }
        if i + old_trimmed.len() > original_lines.len() {
            continue;
        }
        let all_match = old_trimmed
            .iter()
            .enumerate()
            .all(|(j, old_ln)| original_lines[i + j].trim() == *old_ln);
        if all_match {
            let actual = original_lines[i..i + old_trimmed.len()].join("\n");
            if original.contains(&actual) {
                return Some(actual);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Pass 3: BlockAnchor — first/last lines anchor, middle uses similarity
// ---------------------------------------------------------------------------

fn block_anchor_find(original: &str, old_content: &str) -> Option<String> {
    let old_lines: Vec<&str> = old_content.split('\n').collect();
    if old_lines.len() < 3 {
        return None;
    }

    let first_trimmed = old_lines[0].trim();
    let last_trimmed = old_lines[old_lines.len() - 1].trim();
    let middle_old: Vec<&str> = old_lines[1..old_lines.len() - 1]
        .iter()
        .map(|l| l.trim())
        .collect();

    let original_lines: Vec<&str> = original.split('\n').collect();
    let mut candidates: Vec<(usize, usize, f64)> = Vec::new(); // (start, end_inclusive, similarity)

    for i in 0..original_lines.len() {
        if original_lines[i].trim() != first_trimmed {
            continue;
        }
        let window_end = (i + old_lines.len() * 2).min(original_lines.len());
        for end_idx in (i + old_lines.len() - 1)..window_end {
            if end_idx >= original_lines.len() {
                break;
            }
            if original_lines[end_idx].trim() != last_trimmed {
                continue;
            }
            let middle_orig: Vec<&str> = original_lines[i + 1..end_idx]
                .iter()
                .map(|l| l.trim())
                .collect();

            let sim = if middle_old.is_empty() && middle_orig.is_empty() {
                1.0
            } else if middle_old.is_empty() || middle_orig.is_empty() {
                continue;
            } else {
                similarity(&middle_old.join("\n"), &middle_orig.join("\n"))
            };
            candidates.push((i, end_idx, sim));
        }
    }

    if candidates.is_empty() {
        return None;
    }

    let threshold = if candidates.len() == 1 { 0.0 } else { 0.3 };
    let best = candidates
        .iter()
        .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap())?;
    if best.2 < threshold {
        return None;
    }

    let actual = original_lines[best.0..=best.1].join("\n");
    if original.contains(&actual) {
        Some(actual)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Pass 4: WhitespaceNormalized — collapse whitespace runs
// ---------------------------------------------------------------------------

static WS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

fn ws_normalize(s: &str) -> String {
    s.split('\n')
        .map(|ln| WS_RE.replace_all(ln, " ").trim().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn whitespace_normalized_find(original: &str, old_content: &str) -> Option<String> {
    let norm_old = ws_normalize(old_content);
    let original_lines: Vec<&str> = original.split('\n').collect();
    let old_line_count = old_content.split('\n').count();

    for i in 0..original_lines.len() {
        let end_max = (i + old_line_count + 2).min(original_lines.len());
        for j in (i + old_line_count.saturating_sub(1))..=end_max {
            if j > original_lines.len() {
                break;
            }
            let candidate = original_lines[i..j].join("\n");
            if ws_normalize(&candidate) == norm_old && original.contains(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Pass 5: IndentationFlexible — ignore indentation entirely
// ---------------------------------------------------------------------------

fn indentation_flexible_find(original: &str, old_content: &str) -> Option<String> {
    let old_stripped: Vec<&str> = old_content
        .split('\n')
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if old_stripped.is_empty() {
        return None;
    }

    let original_lines: Vec<&str> = original.split('\n').collect();

    for i in 0..original_lines.len() {
        if original_lines[i].trim() != old_stripped[0] {
            continue;
        }
        let mut matched_indices: Vec<usize> = Vec::new();
        let mut j = 0;
        let search_end = (i + old_stripped.len() * 3).min(original_lines.len());
        for (k, orig_line) in original_lines[i..search_end].iter().enumerate() {
            let k = k + i;
            if j >= old_stripped.len() {
                break;
            }
            if orig_line.trim().is_empty() {
                continue; // skip blank lines
            }
            if orig_line.trim() == old_stripped[j] {
                matched_indices.push(k);
                j += 1;
            } else {
                break;
            }
        }

        if j == old_stripped.len() && !matched_indices.is_empty() {
            let start = matched_indices[0];
            let end = matched_indices[matched_indices.len() - 1] + 1;
            let actual = original_lines[start..end].join("\n");
            if original.contains(&actual) {
                return Some(actual);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Pass 6: EscapeNormalized — unescape common escape sequences
// ---------------------------------------------------------------------------

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\\", "\\")
        .replace("\\\"", "\"")
        .replace("\\'", "'")
}

fn escape_normalized_find(original: &str, old_content: &str) -> Option<String> {
    let unescaped = unescape(old_content);
    if unescaped == old_content {
        return None; // no escapes to normalize
    }
    if original.contains(&unescaped) {
        Some(unescaped)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Pass 7: TrimmedBoundary — trim first/last lines, expand to full lines
// ---------------------------------------------------------------------------

fn trimmed_boundary_find(original: &str, old_content: &str) -> Option<String> {
    let trimmed = old_content.trim();
    if trimmed == old_content {
        return None; // nothing to trim
    }

    if original.contains(trimmed) {
        return Some(trimmed.to_string());
    }

    // Try line-level boundary expansion
    let old_lines: Vec<&str> = old_content.split('\n').collect();
    let first_content = old_lines[0].trim();
    let last_content = old_lines[old_lines.len() - 1].trim();

    if first_content.is_empty() || last_content.is_empty() {
        return None;
    }

    let original_lines: Vec<&str> = original.split('\n').collect();
    for i in 0..original_lines.len() {
        if !original_lines[i].contains(first_content) {
            continue;
        }
        let end = (i + old_lines.len() + 2).min(original_lines.len());
        for j in (i + 1)..end {
            if j >= original_lines.len() {
                break;
            }
            if !original_lines[j].contains(last_content) {
                continue;
            }
            let candidate = original_lines[i..=j].join("\n");
            if original.contains(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Pass 8: ContextAware — use surrounding context to locate position
// ---------------------------------------------------------------------------

fn context_aware_find(original: &str, old_content: &str) -> Option<String> {
    let old_lines: Vec<&str> = old_content.split('\n').collect();
    if old_lines.len() < 2 {
        return None;
    }

    let original_lines: Vec<&str> = original.split('\n').collect();

    let first_ctx = old_lines
        .iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim())?;
    let last_ctx = old_lines
        .iter()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim())?;

    // Find all positions of first anchor
    let starts: Vec<usize> = original_lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim().contains(first_ctx))
        .map(|(i, _)| i)
        .collect();

    if starts.is_empty() {
        return None;
    }

    let mut best_match: Option<String> = None;
    let mut best_sim: f64 = 0.0;

    for start in starts {
        let search_end = (start + old_lines.len() * 2).min(original_lines.len());
        for end in (start + 1)..search_end {
            if original_lines[end].trim().contains(last_ctx) {
                let candidate = original_lines[start..=end].join("\n");
                let sim = similarity(old_content.trim(), candidate.trim());
                if sim > best_sim && sim > 0.5 {
                    best_sim = sim;
                    best_match = Some(candidate);
                }
                break; // only check first end anchor per start
            }
        }
    }

    match best_match {
        Some(ref m) if original.contains(m) => best_match,
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Pass 9: MultiOccurrence — trimmed line-by-line match as last resort
// ---------------------------------------------------------------------------

fn multi_occurrence_find(original: &str, old_content: &str) -> Option<String> {
    let trimmed = old_content.trim();
    if trimmed.is_empty() {
        return None;
    }

    let original_lines: Vec<&str> = original.split('\n').collect();
    let trimmed_lines: Vec<&str> = trimmed.split('\n').collect();

    if trimmed_lines.len() > original_lines.len() {
        return None;
    }

    for i in 0..=(original_lines.len() - trimmed_lines.len()) {
        let candidate_lines = &original_lines[i..i + trimmed_lines.len()];
        let all_match = candidate_lines
            .iter()
            .zip(trimmed_lines.iter())
            .all(|(a, b)| a.trim() == b.trim());
        if all_match {
            let candidate = candidate_lines.join("\n");
            if original.contains(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Similarity helper (SequenceMatcher-like)
// ---------------------------------------------------------------------------

/// Compute a similarity ratio between two strings (0.0 to 1.0).
/// Uses a simple longest-common-subsequence approach similar to Python's
/// `difflib.SequenceMatcher.ratio()`.
fn similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let lcs_len = lcs_length(a_bytes, b_bytes);
    (2.0 * lcs_len as f64) / (a_bytes.len() + b_bytes.len()) as f64
}

/// Length of the longest common subsequence (space-optimized).
fn lcs_length(a: &[u8], b: &[u8]) -> usize {
    let m = a.len();
    let n = b.len();
    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                curr[j] = prev[j - 1] + 1;
            } else {
                curr[j] = curr[j - 1].max(prev[j]);
            }
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.iter_mut().for_each(|v| *v = 0);
    }
    *prev.iter().max().unwrap_or(&0)
}

// ---------------------------------------------------------------------------
// Unified diff generation
// ---------------------------------------------------------------------------

/// Generate a unified diff between two strings (like `diff -u`).
pub fn unified_diff(
    file_path: &str,
    original: &str,
    modified: &str,
    context_lines: usize,
) -> String {
    let old_lines: Vec<&str> = original.split('\n').collect();
    let new_lines: Vec<&str> = modified.split('\n').collect();

    // Simple line-by-line diff using LCS on lines
    let matches = line_lcs(&old_lines, &new_lines);

    let mut old_idx = 0;
    let mut new_idx = 0;
    let mut changes: Vec<DiffLine> = Vec::new();

    for &(om, nm) in &matches {
        // Lines removed from old (before this match)
        while old_idx < om {
            changes.push(DiffLine::Remove(old_lines[old_idx]));
            old_idx += 1;
        }
        // Lines added in new (before this match)
        while new_idx < nm {
            changes.push(DiffLine::Add(new_lines[new_idx]));
            new_idx += 1;
        }
        // Context (matching line)
        changes.push(DiffLine::Context(old_lines[old_idx]));
        old_idx += 1;
        new_idx += 1;
    }
    // Remaining lines
    while old_idx < old_lines.len() {
        changes.push(DiffLine::Remove(old_lines[old_idx]));
        old_idx += 1;
    }
    while new_idx < new_lines.len() {
        changes.push(DiffLine::Add(new_lines[new_idx]));
        new_idx += 1;
    }

    // Group changes into hunks with context
    let change_positions: Vec<usize> = changes
        .iter()
        .enumerate()
        .filter(|(_, c)| !matches!(c, DiffLine::Context(_)))
        .map(|(i, _)| i)
        .collect();

    if change_positions.is_empty() {
        return String::new();
    }

    // Merge nearby changes into hunks
    let mut hunk_ranges: Vec<(usize, usize)> = Vec::new();
    let mut start = change_positions[0].saturating_sub(context_lines);
    let mut end = (change_positions[0] + context_lines + 1).min(changes.len());

    for &pos in &change_positions[1..] {
        let new_start = pos.saturating_sub(context_lines);
        let new_end = (pos + context_lines + 1).min(changes.len());
        if new_start <= end {
            end = new_end; // merge
        } else {
            hunk_ranges.push((start, end));
            start = new_start;
            end = new_end;
        }
    }
    hunk_ranges.push((start, end));

    // Build output
    let mut output = format!("--- a/{file_path}\n+++ b/{file_path}\n");

    for (hunk_start, hunk_end) in hunk_ranges {
        // Count old/new lines in hunk
        let mut old_start_line = 1;
        let mut new_start_line = 1;
        let mut old_count = 0;
        let mut new_count = 0;

        // Calculate starting line numbers
        let mut ol = 0;
        let mut nl = 0;
        for (i, change) in changes.iter().enumerate() {
            if i == hunk_start {
                old_start_line = ol + 1;
                new_start_line = nl + 1;
            }
            if i >= hunk_start && i < hunk_end {
                match change {
                    DiffLine::Context(_) => {
                        old_count += 1;
                        new_count += 1;
                    }
                    DiffLine::Remove(_) => {
                        old_count += 1;
                    }
                    DiffLine::Add(_) => {
                        new_count += 1;
                    }
                }
            }
            match change {
                DiffLine::Context(_) => {
                    ol += 1;
                    nl += 1;
                }
                DiffLine::Remove(_) => {
                    ol += 1;
                }
                DiffLine::Add(_) => {
                    nl += 1;
                }
            }
        }

        output.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            old_start_line, old_count, new_start_line, new_count
        ));

        for change in &changes[hunk_start..hunk_end] {
            match change {
                DiffLine::Context(l) => output.push_str(&format!(" {l}\n")),
                DiffLine::Remove(l) => output.push_str(&format!("-{l}\n")),
                DiffLine::Add(l) => output.push_str(&format!("+{l}\n")),
            }
        }
    }

    output
}

#[derive(Debug)]
enum DiffLine<'a> {
    Context(&'a str),
    Remove(&'a str),
    Add(&'a str),
}

/// LCS on line sequences — returns pairs of (old_index, new_index) for matching lines.
fn line_lcs<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<(usize, usize)> {
    let m = old.len();
    let n = new.len();

    // Build LCS table
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if old[i - 1] == new[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to find matching pairs
    let mut result = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if old[i - 1] == new[j - 1] {
            result.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    result.reverse();
    result
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Pass 1: Simple ----

    #[test]
    fn test_simple_exact_match() {
        let original = "fn main() {\n    println!(\"hello\");\n}";
        let old = "println!(\"hello\");";
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "simple");
        assert_eq!(result.actual, old);
    }

    #[test]
    fn test_simple_no_match() {
        let original = "fn main() {}";
        assert!(find_match(original, "nonexistent").is_none());
    }

    // ---- Pass 2: LineTrimmed ----

    #[test]
    fn test_line_trimmed_extra_indent() {
        let original = "fn foo() {\n    let x = 1;\n    let y = 2;\n}";
        // LLM provides without indentation
        let old = "let x = 1;\nlet y = 2;";
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "line_trimmed");
        assert_eq!(result.actual, "    let x = 1;\n    let y = 2;");
    }

    #[test]
    fn test_line_trimmed_different_indent_levels() {
        let original = "  if true {\n      do_thing();\n  }";
        let old = "if true {\n    do_thing();\n}";
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "line_trimmed");
        assert_eq!(result.actual, "  if true {\n      do_thing();\n  }");
    }

    // ---- Pass 3: BlockAnchor ----

    #[test]
    fn test_block_anchor_middle_differs() {
        let original = "fn test() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n}";
        // First and last lines match, middle is slightly different
        let old = "fn test() {\n    let a = 10;\n    let b = 20;\n    let c = 30;\n}";
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "block_anchor");
        assert!(result.actual.starts_with("fn test()"));
        assert!(result.actual.ends_with('}'));
    }

    #[test]
    fn test_block_anchor_too_few_lines() {
        let original = "fn test() {\n}";
        let old = "fn test() {\n}";
        // 2 lines — not enough for block anchor (needs >= 3), falls to simple
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "simple");
    }

    // ---- Pass 4: WhitespaceNormalized ----

    #[test]
    fn test_whitespace_normalized() {
        let original = "let   x  =   1;";
        let old = "let x = 1;";
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "whitespace_normalized");
        assert_eq!(result.actual, "let   x  =   1;");
    }

    #[test]
    fn test_whitespace_normalized_multiline() {
        let original = "fn foo() {\n    let   x  =  1;\n    let  y =  2;\n}";
        let old = "let x = 1;\nlet y = 2;";
        let result = find_match(original, old).unwrap();
        // Should match via line_trimmed or whitespace_normalized
        assert!(result.pass_name == "line_trimmed" || result.pass_name == "whitespace_normalized");
    }

    // ---- Pass 5: IndentationFlexible ----

    #[test]
    fn test_indentation_flexible_skips_blank_lines() {
        let original = "fn foo() {\n\n    let x = 1;\n\n    let y = 2;\n}";
        let old = "let x = 1;\nlet y = 2;";
        let result = find_match(original, old).unwrap();
        // May match via line_trimmed or indentation_flexible
        assert!(result.pass_name == "line_trimmed" || result.pass_name == "indentation_flexible");
    }

    // ---- Pass 6: EscapeNormalized ----

    #[test]
    fn test_escape_normalized() {
        let original = "let s = \"hello\nworld\";";
        // LLM sends literal \n instead of actual newline
        let old = "let s = \"hello\\nworld\";";
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "escape_normalized");
        assert_eq!(result.actual, "let s = \"hello\nworld\";");
    }

    #[test]
    fn test_escape_normalized_tab() {
        let original = "let s = \"hello\tworld\";";
        let old = "let s = \"hello\\tworld\";";
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "escape_normalized");
    }

    #[test]
    fn test_escape_no_change_skipped() {
        let original = "hello world";
        let old = "hello world";
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "simple"); // no escapes, should use simple
    }

    // ---- Pass 7: TrimmedBoundary ----

    #[test]
    fn test_trimmed_boundary() {
        // Test that trimmed boundary matching works. The old_content has leading/trailing
        // whitespace. Earlier passes (indentation_flexible etc.) may also match this,
        // so we verify a match is found and contains the right content.
        let original = "header\n  alpha_line\n  beta_line\nfooter";
        let old = "  \n  alpha_line\n  beta_line\n  ";
        let result = find_match(original, old).unwrap();
        assert!(result.actual.contains("alpha_line"));
        assert!(result.actual.contains("beta_line"));

        // Also test trimmed_boundary directly: trimmed content found in original
        let direct = trimmed_boundary_find("abc xyz def", "  xyz  ");
        assert_eq!(direct, Some("xyz".to_string()));
    }

    #[test]
    fn test_trimmed_boundary_no_trim_needed() {
        let original = "hello world";
        let old = "hello world"; // no trimming needed, falls to simple
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "simple");
    }

    // ---- Pass 8: ContextAware ----

    #[test]
    fn test_context_aware_match() {
        let original = "fn setup() {\n    init();\n}\n\nfn main() {\n    let x = compute();\n    println!(\"{}\", x);\n}";
        // Old content has matching anchors but slightly different middle
        let old = "fn main() {\n    let x = calculate();\n    println!(\"{}\", x);\n}";
        let result = find_match(original, old).unwrap();
        // block_anchor or context_aware can both match this — both use anchor lines
        assert!(
            result.pass_name == "block_anchor" || result.pass_name == "context_aware",
            "expected block_anchor or context_aware, got {}",
            result.pass_name
        );
        assert!(result.actual.contains("fn main()"));
    }

    // ---- Pass 9: MultiOccurrence ----

    #[test]
    fn test_multi_occurrence_trimmed_match() {
        let original = "    fn foo() {\n        bar();\n    }";
        // No exact match, but trimmed line-by-line matches
        let old = "  fn foo() {\n      bar();\n  }";
        let result = find_match(original, old).unwrap();
        // Should match via line_trimmed (earlier pass)
        assert!(result.pass_name == "line_trimmed" || result.pass_name == "multi_occurrence");
        assert_eq!(result.actual, "    fn foo() {\n        bar();\n    }");
    }

    // ---- Similarity helper ----

    #[test]
    fn test_similarity_identical() {
        assert!((similarity("hello", "hello") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_similarity_empty() {
        assert!((similarity("", "") - 1.0).abs() < f64::EPSILON);
        assert!((similarity("hello", "") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_similarity_partial() {
        let sim = similarity("abcdef", "abcxyz");
        assert!(sim > 0.0 && sim < 1.0);
    }

    // ---- Unified diff ----

    #[test]
    fn test_unified_diff_basic() {
        let original = "line1\nline2\nline3\n";
        let modified = "line1\nline2_modified\nline3\n";
        let diff = unified_diff("test.rs", original, modified, 3);
        assert!(diff.contains("--- a/test.rs"));
        assert!(diff.contains("+++ b/test.rs"));
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+line2_modified"));
    }

    #[test]
    fn test_unified_diff_no_changes() {
        let text = "line1\nline2\n";
        let diff = unified_diff("test.rs", text, text, 3);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_unified_diff_addition() {
        let original = "line1\nline3\n";
        let modified = "line1\nline2\nline3\n";
        let diff = unified_diff("test.rs", original, modified, 3);
        assert!(diff.contains("+line2"));
    }

    #[test]
    fn test_unified_diff_removal() {
        let original = "line1\nline2\nline3\n";
        let modified = "line1\nline3\n";
        let diff = unified_diff("test.rs", original, modified, 3);
        assert!(diff.contains("-line2"));
    }

    // ---- Line endings ----

    #[test]
    fn test_normalize_line_endings() {
        assert_eq!(normalize_line_endings("a\r\nb\rc\n"), "a\nb\nc\n");
    }

    // ---- find_match with CRLF ----

    #[test]
    fn test_find_match_crlf() {
        let original = "line1\r\nline2\r\nline3";
        let old = "line2";
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "simple");
        assert_eq!(result.actual, "line2");
    }

    // ---- Edge cases ----

    #[test]
    fn test_empty_old_content() {
        let original = "hello world";
        // Empty old_content should still match via simple (empty string is in any string)
        let result = find_match(original, "");
        assert!(result.is_some());
    }

    #[test]
    fn test_multiline_exact() {
        let original =
            "fn main() {\n    let x = 1;\n    let y = 2;\n    println!(\"{} {}\", x, y);\n}";
        let old = "    let x = 1;\n    let y = 2;";
        let result = find_match(original, old).unwrap();
        assert_eq!(result.pass_name, "simple");
        assert_eq!(result.actual, old);
    }

    // ---- Occurrence finding helper ----

    #[test]
    fn test_find_occurrence_line_numbers() {
        let content = "foo\nbar\nfoo\nbaz\nfoo";
        let positions = find_occurrence_positions(content, "foo");
        assert_eq!(positions, vec![1, 3, 5]);
    }

    #[test]
    fn test_find_occurrence_needle_at_end() {
        let positions = find_occurrence_positions("abc", "c");
        assert_eq!(positions, vec![1]);
    }

    #[test]
    fn test_find_occurrence_needle_is_entire_string() {
        let positions = find_occurrence_positions("abc", "abc");
        assert_eq!(positions, vec![1]);
    }

    #[test]
    fn test_find_occurrence_multibyte_utf8() {
        // 🌍 is 4 bytes; ensure we don't panic on char boundary
        let positions = find_occurrence_positions("a🌍b🌍c", "🌍");
        assert_eq!(positions, vec![1, 1]);
    }

    #[test]
    fn test_find_occurrence_empty_needle() {
        // Empty needle matches everywhere — just ensure no panic
        let positions = find_occurrence_positions("abc", "");
        assert!(!positions.is_empty());
    }

    #[test]
    fn test_find_occurrence_no_match() {
        let positions = find_occurrence_positions("abc", "xyz");
        assert_eq!(positions, Vec::<usize>::new());
    }
}

/// Find line numbers (1-indexed) of all occurrences of `needle` in `haystack`.
pub fn find_occurrence_positions(haystack: &str, needle: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut search_pos = 0;
    while let Some(slice) = haystack.get(search_pos..) {
        if let Some(pos) = slice.find(needle) {
            let abs_pos = search_pos + pos;
            let line_num = haystack[..abs_pos].matches('\n').count() + 1;
            positions.push(line_num);
            search_pos = abs_pos + 1;
            // Snap to next valid UTF-8 char boundary
            while search_pos < haystack.len() && !haystack.is_char_boundary(search_pos) {
                search_pos += 1;
            }
        } else {
            break;
        }
    }
    positions
}
