<p align="center">
  <img src="docs/rocketgrep.png" alt="rocketgrep logo" width="220">
</p>

<h1 align="center">rocketgrep</h1>

<p align="center">
  Fast exact grep today, practical edit-distance grep next to it.
</p>

`rocketgrep` is a Rust CLI search tool inspired by `ripgrep`, with a focus on developer workflows where approximate/fuzzy code search is useful: misspelled identifiers, renamed symbols, noisy generated code, and large trees where exact search is not quite enough.

It currently provides:

- Fast exact regex and fixed-string search.
- Native Levenshtein edit-distance search with `-k/--edit-distance`.
- Parallel directory walking with `.gitignore` support.
- Memory-mapped file scanning with a safe read fallback.
- Context lines, colors, count mode, files-with-matches mode, and JSON output.
- Optional sparse trigram indexing for repeated searches.

## Honest Status

This is an early research-flavored tool, not a drop-in replacement for every `ripgrep` workflow yet.

The exact-search path is already competitive in local warm-cache tests. The fuzzy path is intentionally practical: it uses seed filtering plus thresholded Levenshtein verification, with DP fallback for short patterns. The repository also contains PILLAR-style primitives and a small seaweed monoid scaffold for future work inspired by Charalampopoulos, Kociumaka, and Wellnitz's "Faster Pattern Matching under Edit Distance", but it does not claim to fully implement that paper's Dynamic Puzzle Matching construction yet.

## Install From Source

```powershell
git clone https://github.com/priceds/rocketgrep.git
cd rocketgrep
cargo build --release
target/release/rocketgrep --version
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

The index is a safe prefilter only. Search results are still verified by the matcher, so broad index hits cannot create false matches. Files missing from the index or changed since indexing are scanned normally. If a query is too short or too fuzzy for a safe trigram prefilter, `rocketgrep` scans normally.

## What We Tested

Current verification:

```text
cargo test --all
20 library tests passed, 6 CLI integration tests passed

cargo build --release --bin rocketgrep
release build passed
```

The tests cover exact literal search, regex search, approximate one-error search, ASCII-insensitive fuzzy search, context output, JSON score metadata, `.gitignore` handling, binary-file skipping, sparse index filtering, stale-index safety, PILLAR primitives, and seaweed monoid laws.

Local warm-cache benchmark smoke tests on Windows showed `rocketgrep` beating `ripgrep` for simple exact searches on this repository and on a synthetic 2000-file corpus. That is encouraging, but not a universal performance claim. Larger real-world benchmarks are still needed across cold cache, giant monorepos, complex regexes, binary-heavy trees, huge output volumes, and adversarial fuzzy cases.

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

Contributions are welcome, especially if they come with correctness tests and realistic benchmarks.

Good first areas:

- Add benchmark corpora and reproducible performance reports.
- Improve fuzzy matching on short patterns and repetitive text.
- Add more `ripgrep`-compatible CLI flags.
- Improve output parity and JSON schema documentation.
- Build a more faithful CKW/PILLAR/Dynamic Puzzle Matching backend behind a separate algorithm flag.
- Test on Linux and macOS, especially large repositories.

For algorithmic changes, please include differential tests against a simple dynamic-programming verifier. False negatives are the one thing a search tool must not casually risk.
