# rocketgrep: initial public release

## What this is

This PR introduces `rocketgrep`, a Rust CLI search tool inspired by `ripgrep` with native Levenshtein edit-distance search.

The goal is practical fuzzy code search for developer workflows: finding identifiers through typos, renamed symbols, approximate spellings, and noisy monorepo search spaces. Exact regex and fixed-string search are supported, and fuzzy literal search is available through `-k/--edit-distance`.

## What is included

- Parallel recursive walking with `.gitignore` support.
- Memory-mapped scanning with a read fallback.
- Regex search and fixed-string search.
- Practical approximate search using seed filtering plus bounded Levenshtein verification.
- Context output, colors, counts, files-with-matches mode, score display, score sorting, and JSON output.
- Sparse trigram indexing through `rocketgrep index` and `--index`.
- PILLAR-style primitives and a small seaweed monoid scaffold for future research-backed matching work.

## Honest scope

This is not yet a full implementation of Charalampopoulos, Kociumaka, and Wellnitz's Dynamic Puzzle Matching / seaweed monoid algorithm. The current production fuzzy matcher is pragmatic and designed to be useful immediately. The theoretical pieces are present as scaffolding for future work, not as a claim that the full paper has been implemented.

## Verification run

```text
cargo test --all
20 library tests passed, 6 CLI integration tests passed

cargo build --release --bin rocketgrep
release build passed
```

The tests cover exact search, regex search, approximate matching, ASCII-insensitive fuzzy search, context output, JSON score metadata, ignore handling, binary skipping, sparse index filtering, stale-index safety, PILLAR primitives, and seaweed monoid laws.

## How to test locally

```powershell
cargo test --all
cargo build --release --bin rocketgrep
target/release/rocketgrep -F "needle" .
target/release/rocketgrep -k 1 --scores "neddle" .
target/release/rocketgrep index -o .rocketgrep-index.json .
target/release/rocketgrep -F --index .rocketgrep-index.json "needle" .
```

If `hyperfine` and `ripgrep` are installed:

```powershell
benchmarks/hyperfine_rocketgrep.ps1 -Corpus C:\path\to\repo -Pattern needle
```

## Contribution ideas

- Add larger real-world benchmark reports.
- Improve fuzzy search on short or highly periodic patterns.
- Add more `ripgrep` CLI compatibility.
- Expand JSON schema documentation.
- Implement a faithful CKW-inspired backend behind a separate algorithm flag.
- Test and tune behavior on Linux and macOS.
