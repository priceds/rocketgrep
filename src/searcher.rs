use crate::matcher::{LineMatch, PatternMatcher};
use anyhow::{Context, Result};
use memchr::memchr;
use memmap2::{Mmap, MmapOptions};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub before_context: usize,
    pub after_context: usize,
    pub text: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineKind {
    Match,
    Context,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutputLine {
    pub kind: LineKind,
    pub line_number: usize,
    pub byte_offset: u64,
    pub bytes: Vec<u8>,
    pub matches: Vec<LineMatch>,
}

#[derive(Clone, Debug)]
pub struct FileSearchResult {
    pub path: PathBuf,
    pub lines: Vec<OutputLine>,
    pub match_count: usize,
    pub searched_bytes: u64,
    pub best_edit_distance: Option<u32>,
    pub best_score: Option<f32>,
}

impl FileSearchResult {
    pub fn empty(path: impl Into<PathBuf>, searched_bytes: u64) -> Self {
        Self {
            path: path.into(),
            lines: Vec::new(),
            match_count: 0,
            searched_bytes,
            best_edit_distance: None,
            best_score: None,
        }
    }

    pub fn has_match(&self) -> bool {
        self.match_count > 0
    }
}

enum FileBytes {
    Mmap(Mmap),
    Buffer(Vec<u8>),
}

impl AsRef<[u8]> for FileBytes {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Mmap(mmap) => mmap.as_ref(),
            Self::Buffer(buffer) => buffer.as_slice(),
        }
    }
}

pub fn search_path(
    path: &Path,
    matcher: &PatternMatcher,
    options: &SearchOptions,
) -> Result<FileSearchResult> {
    let data = read_file_bytes(path)?;
    let bytes = data.as_ref();
    let searched_bytes = bytes.len() as u64;

    if bytes.is_empty() {
        return Ok(FileSearchResult::empty(path, searched_bytes));
    }

    if !options.text && memchr(0, bytes).is_some() {
        return Ok(FileSearchResult::empty(path, searched_bytes));
    }

    let line_ranges = split_line_ranges(bytes);
    let mut matched_lines: BTreeMap<usize, Vec<LineMatch>> = BTreeMap::new();

    for (line_index, range) in line_ranges.iter().enumerate() {
        let local_ranges = matcher.find_iter(&bytes[range.clone()]);
        if !local_ranges.is_empty() {
            matched_lines.insert(line_index, local_ranges);
        }
    }

    if matched_lines.is_empty() {
        return Ok(FileSearchResult::empty(path, searched_bytes));
    }

    let selected = select_output_lines(
        line_ranges.len(),
        matched_lines.keys().copied(),
        options.before_context,
        options.after_context,
    );

    let mut lines = Vec::with_capacity(selected.len());
    for line_index in selected {
        let range = &line_ranges[line_index];
        let matches = matched_lines
            .get(&line_index)
            .cloned()
            .unwrap_or_else(Vec::new);
        let kind = if matches.is_empty() {
            LineKind::Context
        } else {
            LineKind::Match
        };

        lines.push(OutputLine {
            kind,
            line_number: line_index + 1,
            byte_offset: range.start as u64,
            bytes: bytes[range.clone()].to_vec(),
            matches,
        });
    }

    let best_edit_distance = matched_lines
        .values()
        .flatten()
        .map(|line_match| line_match.edit_distance)
        .min();
    let best_score = matched_lines
        .values()
        .flatten()
        .map(|line_match| line_match.score)
        .max_by(f32::total_cmp);

    Ok(FileSearchResult {
        path: path.to_path_buf(),
        lines,
        match_count: matched_lines.len(),
        searched_bytes,
        best_edit_distance,
        best_score,
    })
}

fn read_file_bytes(path: &Path) -> Result<FileBytes> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("failed to stat {}", path.display()))?;

    if metadata.len() == 0 {
        return Ok(FileBytes::Buffer(Vec::new()));
    }

    // memmap2 exposes mapping as unsafe because external file mutation can violate
    // Rust's aliasing model. rocketgrep treats mapped files as immutable snapshots and
    // never writes through the mapping.
    match unsafe { MmapOptions::new().map(&file) } {
        Ok(mmap) => Ok(FileBytes::Mmap(mmap)),
        Err(_) => {
            let mut file =
                File::open(path).with_context(|| format!("failed to reopen {}", path.display()))?;
            let mut buffer = Vec::with_capacity(metadata.len() as usize);
            file.read_to_end(&mut buffer)
                .with_context(|| format!("failed to read {}", path.display()))?;
            Ok(FileBytes::Buffer(buffer))
        }
    }
}

fn split_line_ranges(bytes: &[u8]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;

    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            let mut end = index;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            ranges.push(start..end);
            start = index + 1;
        }
    }

    if start < bytes.len() {
        let mut end = bytes.len();
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        ranges.push(start..end);
    }

    ranges
}

fn select_output_lines(
    line_count: usize,
    matches: impl Iterator<Item = usize>,
    before_context: usize,
    after_context: usize,
) -> BTreeSet<usize> {
    let mut selected = BTreeSet::new();

    for line_index in matches {
        let start = line_index.saturating_sub(before_context);
        let end = (line_index + after_context + 1).min(line_count);
        selected.extend(start..end);
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::{ApproxAlgorithm, MatcherConfig};
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn search_path_returns_matches_with_context() {
        let file = NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            b"alpha\nbefore\nneedle here\nafter\nomega\nneedle again\n",
        )
        .unwrap();

        let matcher = PatternMatcher::new(MatcherConfig {
            pattern: "needle".to_string(),
            fixed_strings: true,
            ignore_case: false,
            smart_case: false,
            edit_distance: 0,
            approx_algorithm: ApproxAlgorithm::Auto,
        })
        .unwrap();
        let result = search_path(
            file.path(),
            &matcher,
            &SearchOptions {
                before_context: 1,
                after_context: 1,
                text: false,
            },
        )
        .unwrap();

        let rendered: Vec<_> = result
            .lines
            .iter()
            .map(|line| (line.kind.clone(), line.line_number, line.bytes.as_slice()))
            .collect();

        assert_eq!(result.match_count, 2);
        assert_eq!(
            rendered,
            vec![
                (LineKind::Context, 2, b"before".as_slice()),
                (LineKind::Match, 3, b"needle here".as_slice()),
                (LineKind::Context, 4, b"after".as_slice()),
                (LineKind::Context, 5, b"omega".as_slice()),
                (LineKind::Match, 6, b"needle again".as_slice()),
            ]
        );
    }

    #[test]
    fn binary_files_are_skipped_unless_text_is_set() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), b"abc\0needle\n").unwrap();

        let matcher = PatternMatcher::new(MatcherConfig {
            pattern: "needle".to_string(),
            fixed_strings: true,
            ignore_case: false,
            smart_case: false,
            edit_distance: 0,
            approx_algorithm: ApproxAlgorithm::Auto,
        })
        .unwrap();

        let skipped = search_path(
            file.path(),
            &matcher,
            &SearchOptions {
                before_context: 0,
                after_context: 0,
                text: false,
            },
        )
        .unwrap();
        assert_eq!(skipped.match_count, 0);

        let searched = search_path(
            file.path(),
            &matcher,
            &SearchOptions {
                before_context: 0,
                after_context: 0,
                text: true,
            },
        )
        .unwrap();
        assert_eq!(searched.match_count, 1);
    }
}
