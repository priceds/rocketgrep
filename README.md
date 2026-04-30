<p align="center">
  <img src="docs/rocketgrep.png" alt="rocketgrep logo" width="220">
</p>

<h1 align="center">rocketgrep</h1>

<p align="center">
  A Rust search tool for exact grep and practical fuzzy code search.
</p>

`rocketgrep` is a command-line search tool inspired by `ripgrep`. It searches code and text quickly, and it also supports approximate matching with Levenshtein edit distance through `-k/--edit-distance`.

That means you can search even when the text is slightly wrong:

```powershell
rocketgrep -k 1 "neddle" .
```

This can find `needle`, because it is one edit away from `neddle`.

## Why This Exists

Traditional grep tools are excellent when you know the exact text or regex. Real code search is often messier:

- You half-remember an identifier.
- A symbol was renamed.
- Generated code has noisy variants.
- You typo a function name.
- You want "close enough" matches without opening a full IDE index.

`rocketgrep` explores that space: keep exact search fast, then add native fuzzy search that works directly in the terminal.

## Research Credit

This project is inspired by the paper:

**"Faster Pattern Matching under Edit Distance"** by Panagiotis Charalampopoulos, Tomasz Kociumaka, and Philip Wellnitz.

Links:

- arXiv abstract: https://arxiv.org/abs/2204.03087
- PDF: https://arxiv.org/pdf/2204.03087.pdf

The paper develops a faster theoretical algorithm for pattern matching under edit distance using the PILLAR model, periodicity, Dynamic Puzzle Matching, and the seaweed monoid.

Important honesty note: `rocketgrep` does not yet claim to fully implement the entire paper. The current release uses a practical fuzzy-search engine designed for real CLI use, while also including early PILLAR-style primitives and seaweed-monoid scaffolding for future research-backed work.

## What It Can Do Today

- Search with regex patterns.
- Search fixed strings with `-F`.
- Search approximately with `-k`, for example `-k 1` or `-k 2`.
- Respect `.gitignore` and common ignore rules.
- Search files in parallel.
- Use memory mapping for fast file reads.
- Show context lines with `-A`, `-B`, and `-C`.
- Print JSON with `--json`.
- Show fuzzy scores with `--scores`.
- Rank fuzzy results with `--sort score`.
- Build a sparse trigram index for repeated searches.

## How It Works

At a high level, `rocketgrep` has four pieces.

First, it walks the directory tree. It uses the Rust `ignore` crate, so it understands `.gitignore`, hidden files, file types, globs, and common developer-project rules.

Second, it reads files efficiently. It uses memory-mapped IO through `memmap2` when possible and falls back to normal file reads when mapping fails.

Third, it chooses a matcher:

- Regex search uses Rust's byte-oriented regex engine.
- Exact fixed-string search uses fast byte substring search.
- Approximate search uses a practical edit-distance pipeline.

Fourth, it renders results as human-readable colored output or newline-delimited JSON.

## How Fuzzy Search Works

For approximate search, `rocketgrep` currently uses a pragmatic algorithm:

1. Split the pattern into several smaller exact pieces.
2. Search for those pieces quickly.
3. Use the matching pieces to guess candidate locations.
4. Verify each candidate with bounded Levenshtein distance.
5. Keep the best non-overlapping matches and attach a score.

This works well for many developer searches because identifiers and code tokens usually contain selective substrings.

For very short patterns or weak filters, `rocketgrep` can fall back to a direct dynamic-programming check. That is slower, but safer.

## Install From Source

```powershell
git clone https://github.com/priceds/rocketgrep.git
cd rocketgrep
cargo build --release
target/release/rocketgrep --version
```

On Windows, the binary will be:

```powershell
target\release\rocketgrep.exe
```

## Quick Start

Regex search:

```powershell
cargo run --bin rocketgrep -- "fn main" src
```

Fixed-string search:

```powershell
cargo run --bin rocketgrep -- -F "literal_needle" .
```

Fuzzy search with edit distance 1:

```powershell
cargo run --bin rocketgrep -- -k 1 "neddle" .
```

Show fuzzy scores and rank better matches first:

```powershell
cargo run --bin rocketgrep -- -k 1 --scores --sort score "needle" .
```

Emit newline-delimited JSON:

```powershell
cargo run --bin rocketgrep -- --json -k 1 "needle" src
```

## Indexing

Build a sparse trigram index for repeated searches:

```powershell
cargo run --bin rocketgrep -- index -o .rocketgrep-index.json .
```

Use it for exact or fuzzy search:

```powershell
cargo run --bin rocketgrep -- -F --index .rocketgrep-index.json "needle" .
cargo run --bin rocketgrep -- -k 1 --index .rocketgrep-index.json "needle" .
```

The index is only a prefilter. Every result is still checked by the real matcher, so broad index hits cannot create false matches. Files missing from the index or changed since indexing are scanned normally.

## Current Limitations

- The full theoretical algorithm from the paper is not implemented yet.
- Fuzzy search is literal-pattern based, not fuzzy regex.
- Very short fuzzy patterns can be expensive.
- Highly repetitive text can create many candidate matches.
- The CLI is not yet fully compatible with every `ripgrep` flag.

## What We Tested

Current verification:

```text
cargo test --all
20 library tests passed, 6 CLI integration tests passed

cargo build --release --bin rocketgrep
release build passed
```

The tests cover exact literal search, regex search, approximate one-error search, ASCII-insensitive fuzzy search, context output, JSON score metadata, `.gitignore` handling, binary-file skipping, sparse index filtering, stale-index safety, PILLAR primitives, and seaweed monoid laws.

Local warm-cache benchmark smoke tests on Windows showed `rocketgrep` beating `ripgrep` for simple exact searches on this repository and on a synthetic 2000-file corpus. That is encouraging, but it is not a universal performance claim. Larger real-world benchmarks are still needed.

## Benchmarking

If you have `hyperfine` and `ripgrep` installed:

```powershell
benchmarks/hyperfine_rocketgrep.ps1 -Corpus C:\path\to\repo -Pattern needle
```

Useful comparisons:

- `rocketgrep -F` vs `rg -F`.
- `rocketgrep` regex mode vs `rg`.
- `rocketgrep -k 1` and `rocketgrep -k 2` on real typo/symbol-search workloads.
- Indexed vs non-indexed repeated searches.

## Contributing

Contributions are welcome.

Especially useful contributions:

- Real benchmark reports from large repositories.
- More `ripgrep`-compatible CLI flags.
- Better fuzzy matching for short or repetitive patterns.
- Linux and macOS testing.
- JSON schema documentation.
- A faithful CKW-inspired backend behind a separate algorithm flag.

For algorithmic changes, please include tests against a simple dynamic-programming verifier. Search tools must be especially careful about false negatives.
