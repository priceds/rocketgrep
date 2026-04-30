<p align="center">
  <img src="docs/rocketgrep.png" alt="rocketgrep logo" width="380">
</p>

<h1 align="center">rocketgrep</h1>

<p align="center">
  <strong>Exact grep when you know the text. Fuzzy grep when you almost do.</strong>
</p>

<p align="center">
  <a href="https://github.com/priceds/rocketgrep/actions/workflows/release.yml"><img alt="release" src="https://img.shields.io/github/actions/workflow/status/priceds/rocketgrep/release.yml?branch=main&label=release&style=for-the-badge&labelColor=1f2937"></a>
  <a href="https://github.com/priceds/rocketgrep/releases"><img alt="version" src="https://img.shields.io/github/v/release/priceds/rocketgrep?style=for-the-badge&label=release&labelColor=1f2937&color=ff6b35"></a>
  <a href="https://www.rust-lang.org/"><img alt="rust" src="https://img.shields.io/badge/Rust-2021-f74c00?style=for-the-badge&labelColor=1f2937"></a>
  <a href="https://arxiv.org/abs/2204.03087"><img alt="paper" src="https://img.shields.io/badge/arXiv-2204.03087-b31b1b?style=for-the-badge&labelColor=1f2937"></a>
  <a href="https://github.com/priceds/rocketgrep/blob/main/Cargo.toml"><img alt="license" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-2563eb?style=for-the-badge&labelColor=1f2937"></a>
</p>

<p align="center">
  <a href="#why-rocketgrep">Why</a>
  · <a href="#quick-start">Quick Start</a>
  · <a href="#highlights">Highlights</a>
  · <a href="#how-fuzzy-search-works">How Fuzzy Search Works</a>
  · <a href="#indexing">Indexing</a>
  · <a href="#testing">Testing</a>
  · <a href="#contributing">Contributing</a>
</p>

---

`rocketgrep` is a Rust command-line search tool inspired by `ripgrep`. It searches code and text quickly, and it also supports approximate matching with Levenshtein edit distance through `-k/--edit-distance`.

If you search for the wrong spelling:

```powershell
rocketgrep -k 1 "neddle" .
```

`rocketgrep` can still find `needle`, because it is one edit away.

## Why rocketgrep

Classic grep tools are excellent when you know the exact text or regex. Real code search is often messier.

You might half-remember an identifier. A symbol might have been renamed. Generated code might contain noisy variants. Or you might simply typo a function name while moving fast.

`rocketgrep` explores that space: keep exact search fast, then add native fuzzy search that works directly in the terminal.

## Highlights

- **Fast exact search** — regex search by default and fixed-string search with `-F`.
- **Native fuzzy search** — use `-k 1`, `-k 2`, or another edit-distance threshold.
- **Developer-project aware** — respects `.gitignore`, hidden-file rules, globs, and file types.
- **Parallel scanning** — walks and searches files using Rayon-powered parallelism.
- **Memory-mapped IO** — uses `memmap2` when possible with a normal-read fallback.
- **Useful output modes** — context lines, colors, counts, files-with-matches, scores, ranking, and JSON.
- **Sparse indexing** — optional trigram index for repeated exact or fuzzy searches.
- **Research path included** — PILLAR-style primitives and seaweed-monoid scaffolding are present for future algorithm work.

## Quick Start

Install from crates.io:

```powershell
cargo install rocketgrep
rocketgrep --version
```

That is the recommended install path for most users.

Build from source:

```powershell
git clone https://github.com/priceds/rocketgrep.git
cd rocketgrep
cargo build --release
```

Run the binary:

```powershell
target/release/rocketgrep --version
```

On Windows:

```powershell
target\release\rocketgrep.exe --version
```

Search with a regex:

```powershell
cargo run --bin rocketgrep -- "fn main" src
```

Search for an exact string:

```powershell
cargo run --bin rocketgrep -- -F "literal_needle" .
```

Search with one allowed edit:

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

## How It Works

At a high level, `rocketgrep` has four moving parts.

**1. Walk the project.** It uses the Rust `ignore` crate, so it understands `.gitignore`, hidden files, file types, globs, and common developer-project rules.

**2. Read files efficiently.** It uses memory-mapped IO through `memmap2` when possible and falls back to normal file reads when mapping fails.

**3. Choose a matcher.** Regex search uses Rust's byte-oriented regex engine. Fixed-string search uses fast byte substring search. Fuzzy search uses a practical edit-distance pipeline.

**4. Render results.** Output can be human-readable, colored, scored, ranked, counted, or emitted as newline-delimited JSON.

## How Fuzzy Search Works

For approximate search, `rocketgrep` currently uses a pragmatic algorithm:

1. Split the pattern into smaller exact pieces.
2. Search quickly for those pieces.
3. Use those hits to guess candidate match locations.
4. Verify each candidate with bounded Levenshtein distance.
5. Keep the best non-overlapping matches and attach a score.

This works well for many developer searches because identifiers and code tokens usually contain selective substrings.

For very short patterns or weak filters, `rocketgrep` can fall back to a direct dynamic-programming check. That is slower, but safer.

## Research Credit

`rocketgrep` is inspired by:

**"Faster Pattern Matching under Edit Distance"** by Panagiotis Charalampopoulos, Tomasz Kociumaka, and Philip Wellnitz.

- arXiv abstract: https://arxiv.org/abs/2204.03087
- PDF: https://arxiv.org/pdf/2204.03087.pdf

The paper develops a faster theoretical algorithm for pattern matching under edit distance using the PILLAR model, periodicity, Dynamic Puzzle Matching, and the seaweed monoid.

Important honesty note: `rocketgrep` does **not** yet claim to fully implement the entire paper. The current release uses a practical fuzzy-search engine designed for real CLI use, while also including early PILLAR-style primitives and seaweed-monoid scaffolding for future research-backed work.

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

## Testing

Current verification:

```text
cargo test --all
20 library tests passed, 6 CLI integration tests passed

cargo build --release --bin rocketgrep
release build passed
```

The tests cover exact literal search, regex search, approximate one-error search, ASCII-insensitive fuzzy search, context output, JSON score metadata, `.gitignore` handling, binary-file skipping, sparse index filtering, stale-index safety, PILLAR primitives, and seaweed monoid laws.

## Local Results vs ripgrep

We also ran small warm-cache smoke benchmarks on Windows using `Measure-Command`. These are not a replacement for `hyperfine`, but they are useful early signal.

Environment:

- OS: Windows
- `ripgrep`: `15.1.0`
- `rocketgrep`: release build
- Output redirected to null
- Cache state: warm local filesystem

Small repo search over `src`, pattern `fn`:

| Command | Average | Minimum | Maximum |
| --- | ---: | ---: | ---: |
| `rocketgrep -F fn src` | `15.52ms` | `12.50ms` | `51.05ms` |
| `rg -F fn src` | `23.33ms` | `21.07ms` | `40.03ms` |
| `rocketgrep fn src` | `14.03ms` | `12.45ms` | `26.58ms` |
| `rg fn src` | `23.39ms` | `19.54ms` | `38.78ms` |

Synthetic 2000-file corpus, pattern `needle`:

| Command | Average | Minimum | Maximum |
| --- | ---: | ---: | ---: |
| `rocketgrep -F needle corpus` | `54.39ms` | `49.47ms` | `77.21ms` |
| `rg -F needle corpus` | `103.18ms` | `96.59ms` | `109.34ms` |
| `rocketgrep needle corpus` | `54.77ms` | `49.75ms` | `66.67ms` |
| `rg needle corpus` | `100.22ms` | `93.72ms` | `109.82ms` |
| `rocketgrep -k 1 neodle corpus` | `56.21ms` | `51.27ms` | `68.78ms` |

These results are encouraging: `rocketgrep` was faster than `ripgrep` in these simple local exact-search cases, and fuzzy `-k 1` was close to exact-search time on the synthetic corpus. They are **not** a universal performance claim. Larger real-world benchmarks are still needed across cold cache, huge monorepos, complex regexes, binary-heavy trees, massive output, Linux/macOS, and adversarial fuzzy cases.

## Ripgrep-Style Benchmark Reproduction

We also reproduced the shape of the benchmark shown in the `ripgrep` README: search a kernel-like tree for word matches of `[A-Z]+_SUSPEND`. The original benchmark uses a real built Linux kernel tree on Linux hardware. This reproduction uses a generated C/H corpus on Windows, so treat it as local signal, not a replacement for the original benchmark.

Local setup:

- OS: Windows
- Corpus: generated kernel-like C/H tree under `C:\tmp`
- Pattern: `[A-Z]+_SUSPEND`
- Runs: 20 measured runs after warmup
- Timing tool: PowerShell `Measure-Command`
- Output: redirected to null
- Missing tools: `ack` was not installed; `hypergrep` release artifact was Linux-only in this environment

Tool colors:

<p>
  <span style="background:#ff6b35;color:white;padding:2px 8px;border-radius:999px;">rocketgrep</span>
  <span style="background:#2563eb;color:white;padding:2px 8px;border-radius:999px;">ripgrep</span>
  <span style="background:#16a34a;color:white;padding:2px 8px;border-radius:999px;">ugrep</span>
  <span style="background:#7c3aed;color:white;padding:2px 8px;border-radius:999px;">git grep</span>
  <span style="background:#64748b;color:white;padding:2px 8px;border-radius:999px;">ag / grep</span>
</p>

Default ignore-aware search:

| Tool | Command | Lines | Average | Minimum | Maximum |
| --- | --- | ---: | ---: | ---: | ---: |
| <span style="color:#16a34a;"><strong>ugrep</strong></span> | `ugrep -r --ignore-files --no-hidden -I -w '[A-Z]+_SUSPEND'` | 536 | `42.01ms` | `39.63ms` | `52.90ms` |
| <span style="color:#ff6b35;"><strong>rocketgrep</strong></span> | `rocketgrep '\b[A-Z]+_SUSPEND\b'` | 536 | `64.98ms` | `60.92ms` | `71.76ms` |
| <span style="color:#7c3aed;"><strong>git grep</strong></span> | `git grep -P -n -w '[A-Z]+_SUSPEND'` | 536 | `68.86ms` | `65.92ms` | `75.33ms` |
| <span style="color:#7c3aed;"><strong>git grep</strong></span> | `git grep -E -n -w '[A-Z]+_SUSPEND'` | 536 | `69.30ms` | `67.05ms` | `72.23ms` |
| <span style="color:#2563eb;"><strong>ripgrep</strong></span> | `rg -n -w '[A-Z]+_SUSPEND'` | 536 | `84.02ms` | `77.46ms` | `91.53ms` |
| <span style="color:#64748b;"><strong>ag</strong></span> | `ag --nocolor -w '[A-Z]+_SUSPEND'` | 536 | `137.46ms` | `132.08ms` | `147.58ms` |

Whitelist / no-ignore C-H search:

| Tool | Command | Lines | Average | Minimum | Maximum |
| --- | --- | ---: | ---: | ---: | ---: |
| <span style="color:#16a34a;"><strong>ugrep</strong></span> | `ugrep -r -n --include='*.c' --include='*.h' -w '[A-Z]+_SUSPEND'` | 736 | `50.80ms` | `49.16ms` | `56.14ms` |
| <span style="color:#ff6b35;"><strong>rocketgrep</strong></span> | `rocketgrep --no-ignore --hidden --text -t c '\b[A-Z]+_SUSPEND\b'` | 736 | `83.65ms` | `77.82ms` | `89.31ms` |
| <span style="color:#2563eb;"><strong>ripgrep</strong></span> | `rg -uuu -tc -n -w '[A-Z]+_SUSPEND'` | 736 | `89.91ms` | `87.14ms` | `93.36ms` |
| <span style="color:#64748b;"><strong>Git grep.exe</strong></span> | `grep -E -r -n --include='*.c' --include='*.h' -w '[A-Z]+_SUSPEND'` | 736 | `355.91ms` | `345.35ms` | `364.69ms` |

In this local reproduction, `ugrep` was fastest overall. `rocketgrep` was faster than `ripgrep` in both tested modes, and landed close to `git grep` in the default ignore-aware search. The result is promising, but still only one benchmark on one Windows machine.

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
