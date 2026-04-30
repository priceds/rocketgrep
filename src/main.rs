use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rayon::prelude::*;
use rocketgrep::{
    build_index, collect_files, load_index, render_results, save_index, ApproxAlgorithm,
    ColorChoice, IndexBuildOptions, MatcherConfig, OutputFormat, PatternMatcher, RenderOptions,
    SearchOptions, WalkOptions,
};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::UNIX_EPOCH;

#[derive(Debug, Parser)]
#[command(name = "rocketgrep")]
#[command(about = "rocketgrep: fast exact grep plus native edit-distance search")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    search: SearchArgs,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Build a sparse trigram index for repeated searches.
    Index(IndexArgs),
}

#[derive(Clone, Debug, Args)]
struct SearchArgs {
    /// Regex pattern by default. With -k, the pattern is matched literally under edit distance.
    pattern: Option<String>,

    /// Files or directories to search. Defaults to the current directory.
    paths: Vec<PathBuf>,

    /// Treat the pattern as a literal string instead of a regex.
    #[arg(short = 'F', long = "fixed-strings")]
    fixed_strings: bool,

    /// Maximum Levenshtein edit distance for literal approximate search.
    #[arg(short = 'k', long = "edit-distance", default_value_t = 0)]
    edit_distance: u32,

    /// Approximate matching backend.
    #[arg(long = "approx-algorithm", value_enum, default_value_t = ApproxAlgorithm::Auto)]
    approx_algorithm: ApproxAlgorithm,

    /// Case-insensitive search.
    #[arg(short = 'i', long = "ignore-case")]
    ignore_case: bool,

    /// Case-insensitive unless the pattern contains an uppercase ASCII byte.
    #[arg(short = 'S', long = "smart-case")]
    smart_case: bool,

    /// Include or exclude files/directories using gitignore-style glob overrides.
    #[arg(short = 'g', long = "glob", value_name = "GLOB")]
    globs: Vec<String>,

    /// Only search files matching a known type, e.g. rust, js, py.
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    types: Vec<String>,

    /// Exclude files matching a known type.
    #[arg(long = "type-not", value_name = "TYPE")]
    type_not: Vec<String>,

    /// Search hidden files and directories.
    #[arg(long = "hidden")]
    hidden: bool,

    /// Do not respect .gitignore, .ignore, or global ignore files.
    #[arg(long = "no-ignore")]
    no_ignore: bool,

    /// Follow symbolic links.
    #[arg(short = 'L', long = "follow")]
    follow_links: bool,

    /// Accepted for grep compatibility; recursive search is already the default.
    #[arg(short = 'r', long = "recursive")]
    recursive: bool,

    /// Show N lines after each match.
    #[arg(short = 'A', long = "after-context", default_value_t = 0)]
    after_context: usize,

    /// Show N lines before each match.
    #[arg(short = 'B', long = "before-context", default_value_t = 0)]
    before_context: usize,

    /// Show N lines before and after each match.
    #[arg(short = 'C', long = "context")]
    context: Option<usize>,

    /// Print line numbers. Enabled by default; provided for ripgrep familiarity.
    #[arg(short = 'n', long = "line-number")]
    line_number: bool,

    /// Do not print line numbers.
    #[arg(long = "no-line-number")]
    no_line_number: bool,

    /// Always print file paths.
    #[arg(short = 'H', long = "with-filename")]
    with_filename: bool,

    /// Never print file paths.
    #[arg(long = "no-filename")]
    no_filename: bool,

    /// Print one JSON object per output line.
    #[arg(long = "json")]
    json: bool,

    /// Configure ANSI color output.
    #[arg(long = "color", value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,

    /// Print only the number of matching lines per file.
    #[arg(short = 'c', long = "count")]
    count: bool,

    /// Print only paths that contain at least one match.
    #[arg(short = 'l', long = "files-with-matches")]
    files_with_matches: bool,

    /// Suppress normal output and use only the exit code.
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Search binary files as text.
    #[arg(long = "text")]
    text: bool,

    /// Number of worker threads.
    #[arg(short = 'j', long = "threads")]
    threads: Option<usize>,

    /// Use a prebuilt rocketgrep trigram index to prefilter files.
    #[arg(long = "index", value_name = "PATH")]
    index: Option<PathBuf>,

    /// Sort output by path or best fuzzy score.
    #[arg(long = "sort", value_enum, default_value_t = SortMode::Path)]
    sort: SortMode,

    /// Append best edit distance and score to human output.
    #[arg(long = "scores")]
    scores: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SortMode {
    Path,
    Score,
}

#[derive(Clone, Debug, Args)]
struct IndexArgs {
    /// Files or directories to index. Defaults to the current directory.
    paths: Vec<PathBuf>,

    /// Where to write the index.
    #[arg(short = 'o', long = "output", default_value = ".rocketgrep-index.json")]
    output: PathBuf,

    /// Include or exclude files/directories using gitignore-style glob overrides.
    #[arg(short = 'g', long = "glob", value_name = "GLOB")]
    globs: Vec<String>,

    /// Only index files matching a known type, e.g. rust, js, py.
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    types: Vec<String>,

    /// Exclude files matching a known type.
    #[arg(long = "type-not", value_name = "TYPE")]
    type_not: Vec<String>,

    /// Index hidden files and directories.
    #[arg(long = "hidden")]
    hidden: bool,

    /// Do not respect .gitignore, .ignore, or global ignore files.
    #[arg(long = "no-ignore")]
    no_ignore: bool,

    /// Follow symbolic links.
    #[arg(short = 'L', long = "follow")]
    follow_links: bool,

    /// Index binary files as text.
    #[arg(long = "text")]
    text: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("rocketgrep: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Index(args)) => run_index(args),
        None => run_search(cli.search),
    }
}

fn run_index(args: IndexArgs) -> Result<ExitCode> {
    let index = build_index(&IndexBuildOptions {
        walk: WalkOptions {
            paths: args.paths,
            globs: args.globs,
            types: args.types,
            type_not: args.type_not,
            hidden: args.hidden,
            no_ignore: args.no_ignore,
            follow_links: args.follow_links,
        },
        text: args.text,
    })?;

    save_index(&index, &args.output)?;
    println!(
        "Indexed {} files into {}",
        index.files.len(),
        args.output.display()
    );
    Ok(ExitCode::SUCCESS)
}

fn run_search(args: SearchArgs) -> Result<ExitCode> {
    if let Some(threads) = args.threads {
        if threads == 0 {
            bail!("--threads must be greater than zero");
        }
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .context("failed to initialize rayon thread pool")?;
    }

    let pattern = args
        .pattern
        .clone()
        .context("missing pattern; use `rocketgrep PATTERN [PATH ...]` or `rocketgrep index`")?;

    let matcher = PatternMatcher::new(MatcherConfig {
        pattern,
        fixed_strings: args.fixed_strings,
        ignore_case: args.ignore_case,
        smart_case: args.smart_case,
        edit_distance: args.edit_distance,
        approx_algorithm: args.approx_algorithm,
    })?;

    let single_explicit_file = args.paths.len() == 1 && args.paths[0].is_file();
    let walk_output = collect_files(&WalkOptions {
        paths: args.paths,
        globs: args.globs,
        types: args.types,
        type_not: args.type_not,
        hidden: args.hidden,
        no_ignore: args.no_ignore,
        follow_links: args.follow_links,
    })?;

    let mut paths = walk_output.paths;
    if let Some(index_path) = &args.index {
        paths = apply_index_filter(paths, index_path, &matcher)?;
    }

    let context = args.context;
    let before_context = context.unwrap_or(args.before_context);
    let after_context = context.unwrap_or(args.after_context);
    let search_options = SearchOptions {
        before_context,
        after_context,
        text: args.text,
    };

    let searched = paths
        .par_iter()
        .map(|path| search_one(path, &matcher, &search_options))
        .collect::<Vec<_>>();

    let mut results = Vec::new();
    let mut errors = walk_output.errors;

    for item in searched {
        match item {
            Ok(result) => results.push(result),
            Err(err) => errors.push(err),
        }
    }

    sort_results(&mut results, args.sort);
    let has_match = results.iter().any(|result| result.has_match());

    if !args.quiet {
        for err in &errors {
            eprintln!("rocketgrep: {err}");
        }

        let with_filename = if args.no_filename {
            false
        } else {
            args.with_filename || !single_explicit_file
        };

        render_results(
            &results,
            &RenderOptions {
                color: args.color,
                format: if args.json {
                    OutputFormat::Json
                } else {
                    OutputFormat::Human
                },
                line_number: !args.no_line_number || args.line_number,
                with_filename,
                count: args.count,
                files_with_matches: args.files_with_matches,
                emit_group_separators: before_context > 0 || after_context > 0,
                show_scores: args.scores,
            },
        )?;
    }

    if !errors.is_empty() {
        Ok(ExitCode::from(2))
    } else if has_match {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn apply_index_filter(
    paths: Vec<PathBuf>,
    index_path: &Path,
    matcher: &PatternMatcher,
) -> Result<Vec<PathBuf>> {
    let Some(query) = matcher.index_query() else {
        return Ok(paths);
    };
    let index = load_index(index_path)?;
    let Some(candidates) = index.candidate_paths(&query) else {
        return Ok(paths);
    };
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let canonical_candidates: HashSet<PathBuf> = candidates
        .into_iter()
        .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
        .collect();
    let indexed_files: HashMap<PathBuf, (u64, Option<u64>)> = index
        .files
        .iter()
        .map(|file| {
            (
                std::fs::canonicalize(&file.path).unwrap_or_else(|_| file.path.clone()),
                (file.len, file.modified_unix_secs),
            )
        })
        .collect();

    Ok(paths
        .into_iter()
        .filter(|path| {
            let Ok(canonical) = std::fs::canonicalize(path) else {
                return true;
            };

            let Some(indexed) = indexed_files.get(&canonical) else {
                return true;
            };

            if !index_entry_is_fresh(path, *indexed) {
                return true;
            }

            canonical_candidates.contains(&canonical)
        })
        .collect())
}

fn index_entry_is_fresh(path: &Path, indexed: (u64, Option<u64>)) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.len() != indexed.0 {
        return false;
    }

    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs());
    modified == indexed.1
}

fn sort_results(results: &mut [rocketgrep::FileSearchResult], sort: SortMode) {
    match sort {
        SortMode::Path => results.sort_by(|left, right| left.path.cmp(&right.path)),
        SortMode::Score => results.sort_by(|left, right| {
            let distance_order = left
                .best_edit_distance
                .unwrap_or(u32::MAX)
                .cmp(&right.best_edit_distance.unwrap_or(u32::MAX));
            if distance_order != Ordering::Equal {
                return distance_order;
            }

            let score_order = right
                .best_score
                .unwrap_or(0.0)
                .total_cmp(&left.best_score.unwrap_or(0.0));
            if score_order != Ordering::Equal {
                return score_order;
            }

            left.path.cmp(&right.path)
        }),
    }
}

fn search_one(
    path: &PathBuf,
    matcher: &PatternMatcher,
    options: &SearchOptions,
) -> Result<rocketgrep::FileSearchResult, String> {
    rocketgrep::search_path(path, matcher, options)
        .map_err(|err| format!("{}: {err:#}", path.display()))
}
