use crate::approx::{ApproximateMatcher, ApproximateOptions};
use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use memchr::memmem;
use regex::bytes::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ApproxAlgorithm {
    Auto,
    Seeded,
    Dp,
}

#[derive(Clone, Debug)]
pub struct MatcherConfig {
    pub pattern: String,
    pub fixed_strings: bool,
    pub ignore_case: bool,
    pub smart_case: bool,
    pub edit_distance: u32,
    pub approx_algorithm: ApproxAlgorithm,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineMatch {
    pub range: Range<usize>,
    pub edit_distance: u32,
    pub score: f32,
}

impl LineMatch {
    pub fn exact(range: Range<usize>) -> Self {
        Self {
            range,
            edit_distance: 0,
            score: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum PatternMatcher {
    Literal {
        needle: Vec<u8>,
        ascii_case_insensitive: bool,
    },
    Regex(Regex),
    Approximate(ApproximateMatcher),
}

impl PatternMatcher {
    pub fn new(config: MatcherConfig) -> Result<Self> {
        let ignore_case = config.ignore_case
            || (config.smart_case && !config.pattern.bytes().any(|b| b.is_ascii_uppercase()));

        if config.edit_distance > 0 {
            if !config.fixed_strings {
                // Edit distance over a regular language is useful, but it is not the
                // Levenshtein pattern-matching problem this binary currently solves.
                // Treating the pattern literally keeps -k predictable and fast.
            }

            return Ok(Self::Approximate(ApproximateMatcher::new(
                config.pattern.into_bytes(),
                ApproximateOptions {
                    max_distance: config.edit_distance,
                    ascii_case_insensitive: ignore_case,
                    algorithm: config.approx_algorithm,
                },
            )?));
        }

        if config.fixed_strings {
            return Ok(Self::Literal {
                needle: config.pattern.into_bytes(),
                ascii_case_insensitive: ignore_case,
            });
        }

        if config.approx_algorithm != ApproxAlgorithm::Auto {
            bail!("--approx-algorithm can only be used with --edit-distance greater than zero");
        }

        let regex = RegexBuilder::new(&config.pattern)
            .case_insensitive(ignore_case)
            .build()
            .with_context(|| format!("failed to compile regex pattern {:?}", config.pattern))?;

        Ok(Self::Regex(regex))
    }

    pub fn find_iter(&self, haystack: &[u8]) -> Vec<LineMatch> {
        match self {
            Self::Literal {
                needle,
                ascii_case_insensitive,
            } if *ascii_case_insensitive => find_ascii_case_insensitive(haystack, needle),
            Self::Literal { needle, .. } => find_literal(haystack, needle),
            Self::Regex(regex) => regex
                .find_iter(haystack)
                .map(|m| LineMatch::exact(m.start()..m.end()))
                .collect(),
            Self::Approximate(matcher) => matcher.find_iter(haystack),
        }
    }

    pub fn index_query(&self) -> Option<IndexQuery> {
        match self {
            Self::Literal { needle, .. } if needle.len() >= 3 => Some(IndexQuery {
                trigrams: trigrams_for_index(needle),
                require_all: true,
            }),
            Self::Approximate(matcher) => matcher.index_query(),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct IndexQuery {
    pub trigrams: Vec<u32>,
    pub require_all: bool,
}

pub fn trigrams_for_index(bytes: &[u8]) -> Vec<u32> {
    if bytes.len() < 3 {
        return Vec::new();
    }

    let mut trigrams = bytes
        .windows(3)
        .map(|window| encode_trigram(window[0], window[1], window[2]))
        .collect::<Vec<_>>();
    trigrams.sort_unstable();
    trigrams.dedup();
    trigrams
}

pub fn encode_trigram(a: u8, b: u8, c: u8) -> u32 {
    ((a.to_ascii_lowercase() as u32) << 16)
        | ((b.to_ascii_lowercase() as u32) << 8)
        | c.to_ascii_lowercase() as u32
}

fn find_literal(haystack: &[u8], needle: &[u8]) -> Vec<LineMatch> {
    if needle.is_empty() {
        return vec![LineMatch::exact(0..0)];
    }

    memmem::find_iter(haystack, needle)
        .map(|start| LineMatch::exact(start..start + needle.len()))
        .collect()
}

fn find_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> Vec<LineMatch> {
    if needle.is_empty() {
        return vec![LineMatch::exact(0..0)];
    }

    if needle.len() > haystack.len() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let first = needle[0].to_ascii_lowercase();
    let last_start = haystack.len() - needle.len();

    for start in 0..=last_start {
        if haystack[start].to_ascii_lowercase() != first {
            continue;
        }

        if haystack[start..start + needle.len()].eq_ignore_ascii_case(needle) {
            ranges.push(LineMatch::exact(start..start + needle.len()));
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_config(pattern: &str, fixed_strings: bool) -> MatcherConfig {
        MatcherConfig {
            pattern: pattern.to_string(),
            fixed_strings,
            ignore_case: false,
            smart_case: false,
            edit_distance: 0,
            approx_algorithm: ApproxAlgorithm::Auto,
        }
    }

    #[test]
    fn literal_finds_all_non_overlapping_occurrences() {
        let matcher = PatternMatcher::new(exact_config("aba", true)).unwrap();

        let ranges: Vec<_> = matcher
            .find_iter(b"aba xx aba")
            .into_iter()
            .map(|m| m.range)
            .collect();
        assert_eq!(ranges, vec![0..3, 7..10]);
    }

    #[test]
    fn literal_ascii_case_insensitive_preserves_offsets() {
        let matcher = PatternMatcher::new(MatcherConfig {
            pattern: "needle".to_string(),
            fixed_strings: true,
            ignore_case: true,
            smart_case: false,
            edit_distance: 0,
            approx_algorithm: ApproxAlgorithm::Auto,
        })
        .unwrap();

        let ranges: Vec<_> = matcher
            .find_iter(b"a NEEDLE here")
            .into_iter()
            .map(|m| m.range)
            .collect();
        assert_eq!(ranges, vec![2..8]);
    }

    #[test]
    fn regex_uses_bytes_engine() {
        let matcher = PatternMatcher::new(exact_config(r"f[o0]{2}", false)).unwrap();

        let ranges: Vec<_> = matcher
            .find_iter(b"foo f00 far")
            .into_iter()
            .map(|m| m.range)
            .collect();
        assert_eq!(ranges, vec![0..3, 4..7]);
    }

    #[test]
    fn smart_case_ignores_case_only_without_uppercase_pattern_bytes() {
        let matcher = PatternMatcher::new(MatcherConfig {
            pattern: "needle".to_string(),
            fixed_strings: true,
            ignore_case: false,
            smart_case: true,
            edit_distance: 0,
            approx_algorithm: ApproxAlgorithm::Auto,
        })
        .unwrap();

        assert_eq!(matcher.find_iter(b"NEEDLE")[0].range, 0..6);

        let matcher = PatternMatcher::new(MatcherConfig {
            pattern: "Needle".to_string(),
            fixed_strings: true,
            ignore_case: false,
            smart_case: true,
            edit_distance: 0,
            approx_algorithm: ApproxAlgorithm::Auto,
        })
        .unwrap();

        assert!(matcher.find_iter(b"needle").is_empty());
    }

    #[test]
    fn approximate_matcher_finds_one_substitution() {
        let matcher = PatternMatcher::new(MatcherConfig {
            pattern: "needle".to_string(),
            fixed_strings: true,
            ignore_case: false,
            smart_case: false,
            edit_distance: 1,
            approx_algorithm: ApproxAlgorithm::Auto,
        })
        .unwrap();

        let matches = matcher.find_iter(b"find a nexdle in here");
        assert_eq!(matches[0].range, 7..13);
        assert_eq!(matches[0].edit_distance, 1);
    }
}
