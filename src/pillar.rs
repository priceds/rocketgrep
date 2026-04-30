use memchr::memmem;
use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommonExtension {
    pub text_offset: usize,
    pub pattern_offset: usize,
    pub len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Period {
    pub len: usize,
    pub exponent: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditVerification {
    pub distance: u32,
    pub consumed_text: usize,
}

pub fn lcp(left: &[u8], right: &[u8]) -> usize {
    let limit = left.len().min(right.len());
    let mut offset = 0;

    while offset + 8 <= limit && left[offset..offset + 8] == right[offset..offset + 8] {
        offset += 8;
    }

    while offset < limit && left[offset] == right[offset] {
        offset += 1;
    }

    offset
}

pub fn lcp_at(text: &[u8], text_offset: usize, pattern: &[u8], pattern_offset: usize) -> usize {
    if text_offset > text.len() || pattern_offset > pattern.len() {
        return 0;
    }
    lcp(&text[text_offset..], &pattern[pattern_offset..])
}

pub fn lcs(left: &[u8], right: &[u8]) -> usize {
    let limit = left.len().min(right.len());
    let mut offset = 0;

    while offset < limit && left[left.len() - 1 - offset] == right[right.len() - 1 - offset] {
        offset += 1;
    }

    offset
}

pub fn lcs_at(text: &[u8], text_end: usize, pattern: &[u8], pattern_end: usize) -> usize {
    if text_end > text.len() || pattern_end > pattern.len() {
        return 0;
    }
    lcs(&text[..text_end], &pattern[..pattern_end])
}

pub fn longest_common_extension(
    text: &[u8],
    text_offset: usize,
    pattern: &[u8],
    pattern_offset: usize,
) -> CommonExtension {
    CommonExtension {
        text_offset,
        pattern_offset,
        len: lcp_at(text, text_offset, pattern, pattern_offset),
    }
}

pub fn exact_occurrences(haystack: &[u8], needle: &[u8]) -> Vec<Range<usize>> {
    if needle.is_empty() {
        return vec![0..0];
    }

    memmem::find_iter(haystack, needle)
        .map(|start| start..start + needle.len())
        .collect()
}

pub fn primitive_period(bytes: &[u8]) -> Option<Period> {
    if bytes.is_empty() {
        return None;
    }

    let mut prefix = vec![0; bytes.len()];
    for i in 1..bytes.len() {
        let mut j = prefix[i - 1];
        while j > 0 && bytes[i] != bytes[j] {
            j = prefix[j - 1];
        }
        if bytes[i] == bytes[j] {
            j += 1;
        }
        prefix[i] = j;
    }

    let candidate = bytes.len() - prefix[bytes.len() - 1];
    let len = if candidate < bytes.len() && bytes.len() % candidate == 0 {
        candidate
    } else {
        bytes.len()
    };

    Some(Period {
        len,
        exponent: bytes.len() / len,
    })
}

pub fn has_period(bytes: &[u8], period: usize) -> bool {
    if period == 0 || period > bytes.len() {
        return false;
    }
    (period..bytes.len()).all(|index| bytes[index] == bytes[index - period])
}

pub fn is_highly_periodic(bytes: &[u8], max_period: usize) -> bool {
    primitive_period(bytes).is_some_and(|period| period.len <= max_period && period.exponent >= 2)
}

pub fn bounded_levenshtein(left: &[u8], right: &[u8], max_distance: u32) -> Option<u32> {
    let k = max_distance as usize;
    if left.len().abs_diff(right.len()) > k {
        return None;
    }

    if left.is_empty() {
        return (right.len() <= k).then_some(right.len() as u32);
    }
    if right.is_empty() {
        return (left.len() <= k).then_some(left.len() as u32);
    }

    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (i, &a) in left.iter().enumerate() {
        current[0] = i + 1;
        let mut row_min = current[0];
        let band_start = (i + 1).saturating_sub(k);
        let band_end = (i + 1 + k).min(right.len());

        for j in 1..=right.len() {
            if j < band_start || j > band_end {
                current[j] = k + 1;
                continue;
            }

            let substitution = previous[j - 1] + usize::from(a != right[j - 1]);
            let insertion = current[j - 1] + 1;
            let deletion = previous[j] + 1;
            current[j] = substitution.min(insertion.min(deletion));
            row_min = row_min.min(current[j]);
        }

        if row_min > k {
            return None;
        }

        std::mem::swap(&mut previous, &mut current);
    }

    (previous[right.len()] <= k).then_some(previous[right.len()] as u32)
}

pub fn kangaroo_verify(pattern: &[u8], text: &[u8], max_distance: u32) -> Option<EditVerification> {
    let k = max_distance as usize;
    let mut pattern_offset = 0;
    let mut text_offset = 0;
    let mut errors = 0;

    loop {
        let jump = lcp_at(text, text_offset, pattern, pattern_offset);
        pattern_offset += jump;
        text_offset += jump;

        if pattern_offset == pattern.len() {
            let trailing = text.len().saturating_sub(text_offset);
            if errors + trailing <= k {
                return Some(EditVerification {
                    distance: (errors + trailing) as u32,
                    consumed_text: text.len(),
                });
            }
            return None;
        }

        if text_offset == text.len() {
            let trailing = pattern.len().saturating_sub(pattern_offset);
            if errors + trailing <= k {
                return Some(EditVerification {
                    distance: (errors + trailing) as u32,
                    consumed_text: text_offset,
                });
            }
            return None;
        }

        errors += 1;
        if errors > k {
            return None;
        }

        pattern_offset += 1;
        text_offset += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcp_and_lcs_work_on_byte_slices() {
        assert_eq!(lcp(b"abcdef", b"abcxyz"), 3);
        assert_eq!(lcp_at(b"xxabcdef", 2, b"yyabcxyz", 2), 3);
        assert_eq!(lcs(b"xyzabc", b"123abc"), 3);
        assert_eq!(lcs_at(b"xyzabc!!", 6, b"123abc??", 6), 3);
    }

    #[test]
    fn primitive_period_detects_repetition() {
        assert_eq!(
            primitive_period(b"abababab"),
            Some(Period {
                len: 2,
                exponent: 4
            })
        );
        assert_eq!(primitive_period(b"abcdef").unwrap().len, 6);
        assert!(is_highly_periodic(b"abcabcabc", 3));
    }

    #[test]
    fn bounded_edit_distance_prunes_large_distances() {
        assert_eq!(bounded_levenshtein(b"kitten", b"sitten", 1), Some(1));
        assert_eq!(bounded_levenshtein(b"kitten", b"sitting", 3), Some(3));
        assert_eq!(bounded_levenshtein(b"kitten", b"sitting", 2), None);
    }

    #[test]
    fn kangaroo_verify_handles_substitutions() {
        assert_eq!(
            kangaroo_verify(b"needle", b"nexdle", 1),
            Some(EditVerification {
                distance: 1,
                consumed_text: 6
            })
        );
    }
}
