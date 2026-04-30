use crate::matcher::{trigrams_for_index, ApproxAlgorithm, IndexQuery, LineMatch};
use crate::pillar::bounded_levenshtein;
use anyhow::{bail, Result};
use memchr::memmem;
use std::collections::BTreeSet;
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct ApproximateOptions {
    pub max_distance: u32,
    pub ascii_case_insensitive: bool,
    pub algorithm: ApproxAlgorithm,
}

#[derive(Clone, Debug)]
pub struct ApproximateMatcher {
    pattern: Vec<u8>,
    normalized_pattern: Vec<u8>,
    max_distance: u32,
    ascii_case_insensitive: bool,
    algorithm: ApproxAlgorithm,
    seeds: Vec<Seed>,
}

#[derive(Clone, Debug)]
struct Seed {
    pattern_range: Range<usize>,
    bytes: Vec<u8>,
}

impl ApproximateMatcher {
    pub fn new(pattern: Vec<u8>, options: ApproximateOptions) -> Result<Self> {
        if options.max_distance == 0 {
            bail!("ApproximateMatcher requires max_distance greater than zero");
        }

        let normalized_pattern = normalize_bytes(&pattern, options.ascii_case_insensitive);
        let seeds = partition_seeds(&normalized_pattern, options.max_distance as usize);

        Ok(Self {
            pattern,
            normalized_pattern,
            max_distance: options.max_distance,
            ascii_case_insensitive: options.ascii_case_insensitive,
            algorithm: options.algorithm,
            seeds,
        })
    }

    pub fn find_iter(&self, haystack: &[u8]) -> Vec<LineMatch> {
        if self.pattern.is_empty() {
            return vec![LineMatch::exact(0..0)];
        }

        let normalized_haystack = normalize_bytes(haystack, self.ascii_case_insensitive);
        let candidate_starts = match self.algorithm {
            ApproxAlgorithm::Dp => all_starts(normalized_haystack.len()),
            ApproxAlgorithm::Auto | ApproxAlgorithm::Seeded => {
                let seeded = self.seed_candidates(&normalized_haystack);
                if seeded.is_empty() {
                    BTreeSet::new()
                } else {
                    seeded
                }
            }
        };

        let starts = if candidate_starts.is_empty()
            && (self.algorithm == ApproxAlgorithm::Dp || self.seeds_are_too_weak())
        {
            all_starts(normalized_haystack.len())
        } else {
            candidate_starts
        };

        let mut matches = Vec::new();
        for start in starts {
            if let Some(found) = self.best_match_at(&normalized_haystack, start) {
                matches.push(found);
            }
        }

        suppress_redundant_matches(matches)
    }

    pub fn index_query(&self) -> Option<IndexQuery> {
        let trigrams = trigrams_for_index(&self.normalized_pattern);
        if trigrams.is_empty() {
            return None;
        }

        let max_destroyed = self.max_distance as usize * 3;
        if trigrams.len() <= max_destroyed {
            return None;
        }

        Some(IndexQuery {
            trigrams,
            require_all: false,
        })
    }

    fn seed_candidates(&self, haystack: &[u8]) -> BTreeSet<usize> {
        let mut starts = BTreeSet::new();
        let k = self.max_distance as usize;

        for seed in self.seeds.iter().filter(|seed| !seed.bytes.is_empty()) {
            for occurrence in memmem::find_iter(haystack, &seed.bytes) {
                let min_start = occurrence.saturating_sub(seed.pattern_range.start + k);
                let max_start = occurrence
                    .saturating_sub(seed.pattern_range.start)
                    .saturating_add(k)
                    .min(haystack.len());
                starts.extend(min_start..=max_start);
            }
        }

        starts
    }

    fn best_match_at(&self, haystack: &[u8], start: usize) -> Option<LineMatch> {
        if start > haystack.len() {
            return None;
        }

        let k = self.max_distance as usize;
        let min_len = self.normalized_pattern.len().saturating_sub(k);
        let max_len = (self.normalized_pattern.len() + k).min(haystack.len() - start);
        if min_len > max_len {
            return None;
        }

        let mut best: Option<(u32, usize)> = None;
        for len in min_len..=max_len {
            let end = start + len;
            let distance = bounded_levenshtein(
                &self.normalized_pattern,
                &haystack[start..end],
                self.max_distance,
            );
            let Some(distance) = distance else {
                continue;
            };

            let replace = best.is_none_or(|(best_distance, best_len)| {
                distance < best_distance
                    || (distance == best_distance
                        && len.abs_diff(self.normalized_pattern.len())
                            < best_len.abs_diff(self.normalized_pattern.len()))
            });

            if replace {
                best = Some((distance, len));
            }
        }

        best.map(|(distance, len)| LineMatch {
            range: start..start + len,
            edit_distance: distance,
            score: score(distance, self.normalized_pattern.len().max(len)),
        })
    }

    fn seeds_are_too_weak(&self) -> bool {
        self.seeds.iter().all(|seed| seed.bytes.len() < 2)
    }
}

fn normalize_bytes(bytes: &[u8], ascii_case_insensitive: bool) -> Vec<u8> {
    if ascii_case_insensitive {
        bytes.iter().map(u8::to_ascii_lowercase).collect()
    } else {
        bytes.to_vec()
    }
}

fn partition_seeds(pattern: &[u8], max_distance: usize) -> Vec<Seed> {
    if pattern.is_empty() {
        return Vec::new();
    }

    let pieces = (max_distance + 1).min(pattern.len());
    let base = pattern.len() / pieces;
    let mut remainder = pattern.len() % pieces;
    let mut offset = 0;
    let mut seeds = Vec::with_capacity(pieces);

    for _ in 0..pieces {
        let extra = usize::from(remainder > 0);
        remainder = remainder.saturating_sub(1);
        let len = base + extra;
        let end = offset + len;
        seeds.push(Seed {
            pattern_range: offset..end,
            bytes: pattern[offset..end].to_vec(),
        });
        offset = end;
    }

    seeds
}

fn all_starts(len: usize) -> BTreeSet<usize> {
    (0..=len).collect()
}

fn score(distance: u32, len: usize) -> f32 {
    if len == 0 {
        1.0
    } else {
        1.0 - (distance as f32 / len as f32)
    }
}

fn suppress_redundant_matches(mut matches: Vec<LineMatch>) -> Vec<LineMatch> {
    matches.sort_by(|left, right| {
        left.range
            .start
            .cmp(&right.range.start)
            .then(left.edit_distance.cmp(&right.edit_distance))
            .then(left.range.end.cmp(&right.range.end))
    });

    let mut kept: Vec<LineMatch> = Vec::new();
    for candidate in matches {
        if let Some(last) = kept.last_mut() {
            if ranges_overlap(&last.range, &candidate.range) {
                let candidate_is_better = candidate.edit_distance < last.edit_distance
                    || (candidate.edit_distance == last.edit_distance
                        && candidate.range.len() > last.range.len());
                if candidate_is_better {
                    *last = candidate;
                }
                continue;
            }
        }
        kept.push(candidate);
    }

    kept
}

fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_approximate_search_finds_substitution() {
        let matcher = ApproximateMatcher::new(
            b"needle".to_vec(),
            ApproximateOptions {
                max_distance: 1,
                ascii_case_insensitive: false,
                algorithm: ApproxAlgorithm::Auto,
            },
        )
        .unwrap();

        let matches = matcher.find_iter(b"a nexdle appears");
        assert_eq!(matches[0].range, 2..8);
        assert_eq!(matches[0].edit_distance, 1);
    }

    #[test]
    fn dp_search_handles_short_patterns() {
        let matcher = ApproximateMatcher::new(
            b"ab".to_vec(),
            ApproximateOptions {
                max_distance: 1,
                ascii_case_insensitive: false,
                algorithm: ApproxAlgorithm::Auto,
            },
        )
        .unwrap();

        let matches = matcher.find_iter(b"xxacxx");
        assert!(matches
            .iter()
            .any(|m| m.range == (2..4) && m.edit_distance == 1));
    }

    #[test]
    fn approximate_search_can_ignore_ascii_case() {
        let matcher = ApproximateMatcher::new(
            b"needle".to_vec(),
            ApproximateOptions {
                max_distance: 1,
                ascii_case_insensitive: true,
                algorithm: ApproxAlgorithm::Auto,
            },
        )
        .unwrap();

        let matches = matcher.find_iter(b"NEEDLe");
        assert_eq!(matches[0].edit_distance, 0);
    }

    #[test]
    fn index_query_is_safe_for_long_patterns() {
        let matcher = ApproximateMatcher::new(
            b"verylongneedle".to_vec(),
            ApproximateOptions {
                max_distance: 1,
                ascii_case_insensitive: false,
                algorithm: ApproxAlgorithm::Auto,
            },
        )
        .unwrap();

        assert!(matcher.index_query().is_some());
    }
}
