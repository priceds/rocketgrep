use crate::matcher::LineMatch;
use crate::searcher::{FileSearchResult, LineKind, OutputLine};
use anyhow::Result;
use clap::ValueEnum;
use serde_json::json;
use std::io::{self, IsTerminal, Write};
use std::ops::Range;

const ANSI_MATCH: &[u8] = b"\x1b[1;31m";
const ANSI_RESET: &[u8] = b"\x1b[0m";

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub color: ColorChoice,
    pub format: OutputFormat,
    pub line_number: bool,
    pub with_filename: bool,
    pub count: bool,
    pub files_with_matches: bool,
    pub emit_group_separators: bool,
    pub show_scores: bool,
}

pub fn render_results(results: &[FileSearchResult], options: &RenderOptions) -> Result<()> {
    let stdout = io::stdout();
    let color = match options.color {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => stdout.is_terminal(),
    };
    let mut out = stdout.lock();

    match options.format {
        OutputFormat::Human => render_human(&mut out, results, options, color),
        OutputFormat::Json => render_json(&mut out, results),
    }
}

fn render_human(
    out: &mut dyn Write,
    results: &[FileSearchResult],
    options: &RenderOptions,
    color: bool,
) -> Result<()> {
    if options.files_with_matches {
        for result in results.iter().filter(|result| result.has_match()) {
            writeln!(out, "{}", result.path.display())?;
        }
        return Ok(());
    }

    if options.count {
        for result in results.iter().filter(|result| result.has_match()) {
            if options.with_filename {
                write!(out, "{}:", result.path.display())?;
            }
            writeln!(out, "{}", result.match_count)?;
        }
        return Ok(());
    }

    for result in results.iter().filter(|result| result.has_match()) {
        render_file(out, result, options, color)?;
    }

    Ok(())
}

fn render_file(
    out: &mut dyn Write,
    result: &FileSearchResult,
    options: &RenderOptions,
    color: bool,
) -> Result<()> {
    let mut previous_line = None;

    for line in &result.lines {
        if options.emit_group_separators {
            if let Some(previous) = previous_line {
                if line.line_number > previous + 1 {
                    writeln!(out, "--")?;
                }
            }
        }

        write_prefix(out, result, line, options)?;
        write_highlighted(out, &line.bytes, &line.matches, color)?;
        if options.show_scores && line.kind == LineKind::Match {
            write_scores(out, &line.matches)?;
        }
        writeln!(out)?;
        previous_line = Some(line.line_number);
    }

    Ok(())
}

fn write_prefix(
    out: &mut dyn Write,
    result: &FileSearchResult,
    line: &OutputLine,
    options: &RenderOptions,
) -> Result<()> {
    let separator = match line.kind {
        LineKind::Match => ':',
        LineKind::Context => '-',
    };

    if options.with_filename {
        write!(out, "{}{}", result.path.display(), separator)?;
    }

    if options.line_number {
        write!(out, "{}{}", line.line_number, separator)?;
    }

    Ok(())
}

fn write_highlighted(
    out: &mut dyn Write,
    bytes: &[u8],
    matches: &[LineMatch],
    color: bool,
) -> Result<()> {
    if !color || matches.is_empty() {
        out.write_all(bytes)?;
        return Ok(());
    }

    let mut cursor = 0;
    for range in normalized_ranges(
        matches.iter().map(|line_match| line_match.range.clone()),
        bytes.len(),
    ) {
        if range.start > cursor {
            out.write_all(&bytes[cursor..range.start])?;
        }

        if range.start < range.end {
            out.write_all(ANSI_MATCH)?;
            out.write_all(&bytes[range.clone()])?;
            out.write_all(ANSI_RESET)?;
        }
        cursor = range.end;
    }

    if cursor < bytes.len() {
        out.write_all(&bytes[cursor..])?;
    }

    Ok(())
}

fn write_scores(out: &mut dyn Write, matches: &[LineMatch]) -> Result<()> {
    if matches.is_empty() {
        return Ok(());
    }

    let best_distance = matches
        .iter()
        .map(|line_match| line_match.edit_distance)
        .min()
        .unwrap_or(0);
    let best_score = matches
        .iter()
        .map(|line_match| line_match.score)
        .max_by(f32::total_cmp)
        .unwrap_or(1.0);

    write!(out, " [d={best_distance} score={best_score:.3}]")?;
    Ok(())
}

fn normalized_ranges(
    ranges: impl IntoIterator<Item = Range<usize>>,
    len: usize,
) -> Vec<Range<usize>> {
    let mut ranges: Vec<_> = ranges
        .into_iter()
        .filter_map(|range| {
            let start = range.start.min(len);
            let end = range.end.min(len);
            (start <= end).then_some(start..end)
        })
        .collect();
    ranges.sort_by_key(|range| (range.start, range.end));

    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        if range.start == range.end {
            merged.push(range);
            continue;
        }

        if let Some(last) = merged.last_mut() {
            if last.end >= range.start {
                last.end = last.end.max(range.end);
                continue;
            }
        }

        merged.push(range);
    }

    merged
}

fn render_json(out: &mut dyn Write, results: &[FileSearchResult]) -> Result<()> {
    for result in results.iter().filter(|result| result.has_match()) {
        for line in &result.lines {
            let submatches: Vec<_> = line
                .matches
                .iter()
                .map(|line_match| {
                    let range = &line_match.range;
                    json!({
                        "start": range.start,
                        "end": range.end,
                        "edit_distance": line_match.edit_distance,
                        "score": line_match.score,
                        "match": {
                            "text": String::from_utf8_lossy(&line.bytes[range.clone()]).into_owned(),
                        }
                    })
                })
                .collect();

            let value = json!({
                "type": match line.kind {
                    LineKind::Match => "match",
                    LineKind::Context => "context",
                },
                "data": {
                    "path": result.path.display().to_string(),
                    "line_number": line.line_number,
                    "byte_offset": line.byte_offset,
                    "best_edit_distance": result.best_edit_distance,
                    "best_score": result.best_score,
                    "lines": {
                        "text": String::from_utf8_lossy(&line.bytes).into_owned(),
                    },
                    "submatches": submatches,
                }
            });
            writeln!(out, "{}", serde_json::to_string(&value)?)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::LineMatch;
    use crate::searcher::{FileSearchResult, LineKind, OutputLine};
    use std::path::PathBuf;

    #[test]
    fn normalized_ranges_merges_overlaps() {
        assert_eq!(
            normalized_ranges(vec![0..2, 1..4, 7..9], 8),
            vec![0..4, 7..8]
        );
    }

    #[test]
    fn human_count_includes_filename_when_requested() {
        let result = FileSearchResult {
            path: PathBuf::from("sample.rs"),
            lines: vec![OutputLine {
                kind: LineKind::Match,
                line_number: 1,
                byte_offset: 0,
                bytes: b"needle".to_vec(),
                matches: vec![LineMatch::exact(0..6)],
            }],
            match_count: 1,
            searched_bytes: 6,
            best_edit_distance: Some(0),
            best_score: Some(1.0),
        };
        let mut out = Vec::new();
        render_human(
            &mut out,
            &[result],
            &RenderOptions {
                color: ColorChoice::Never,
                format: OutputFormat::Human,
                line_number: true,
                with_filename: true,
                count: true,
                files_with_matches: false,
                emit_group_separators: false,
                show_scores: false,
            },
            false,
        )
        .unwrap();

        assert_eq!(String::from_utf8(out).unwrap(), "sample.rs:1\n");
    }
}
