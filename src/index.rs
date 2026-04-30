use crate::matcher::{encode_trigram, IndexQuery};
use crate::walk::{collect_files, WalkOptions};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const INDEX_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct IndexBuildOptions {
    pub walk: WalkOptions,
    pub text: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RocketIndex {
    pub version: u32,
    pub files: Vec<IndexedFile>,
    pub trigram_files: HashMap<u32, Vec<usize>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IndexedFile {
    pub path: PathBuf,
    pub len: u64,
    pub modified_unix_secs: Option<u64>,
    pub trigrams: Vec<u32>,
}

impl RocketIndex {
    pub fn candidate_paths(&self, query: &IndexQuery) -> Option<HashSet<PathBuf>> {
        if query.trigrams.is_empty() {
            return None;
        }

        if query.require_all {
            self.paths_containing_all(&query.trigrams)
        } else {
            self.paths_containing_any(&query.trigrams)
        }
    }

    fn paths_containing_any(&self, trigrams: &[u32]) -> Option<HashSet<PathBuf>> {
        let mut candidates = HashSet::new();
        for trigram in trigrams {
            if let Some(file_ids) = self.trigram_files.get(trigram) {
                for &file_id in file_ids {
                    if let Some(file) = self.files.get(file_id) {
                        candidates.insert(file.path.clone());
                    }
                }
            }
        }
        Some(candidates)
    }

    fn paths_containing_all(&self, trigrams: &[u32]) -> Option<HashSet<PathBuf>> {
        let mut iter = trigrams.iter();
        let first = iter.next()?;
        let first_ids = self.trigram_files.get(first)?;
        let mut candidates: HashSet<usize> = first_ids.iter().copied().collect();

        for trigram in iter {
            let Some(ids) = self.trigram_files.get(trigram) else {
                return Some(HashSet::new());
            };
            let ids: HashSet<usize> = ids.iter().copied().collect();
            candidates.retain(|id| ids.contains(id));
            if candidates.is_empty() {
                break;
            }
        }

        Some(
            candidates
                .into_iter()
                .filter_map(|file_id| self.files.get(file_id).map(|file| file.path.clone()))
                .collect(),
        )
    }
}

pub fn build_index(options: &IndexBuildOptions) -> Result<RocketIndex> {
    let walk_output = collect_files(&options.walk)?;
    let mut files = Vec::new();
    let mut trigram_files: HashMap<u32, Vec<usize>> = HashMap::new();

    for err in walk_output.errors {
        eprintln!("rocketgrep index: {err}");
    }

    for path in walk_output.paths {
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        if !options.text && bytes.contains(&0) {
            continue;
        }

        let mut trigrams = collect_trigrams(&bytes);
        trigrams.sort_unstable();
        trigrams.dedup();

        let canonical = fs::canonicalize(&path).unwrap_or(path);
        let file_id = files.len();
        for trigram in &trigrams {
            trigram_files.entry(*trigram).or_default().push(file_id);
        }

        files.push(IndexedFile {
            path: canonical,
            len: metadata.len(),
            modified_unix_secs: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs()),
            trigrams,
        });
    }

    Ok(RocketIndex {
        version: INDEX_VERSION,
        files,
        trigram_files,
    })
}

pub fn save_index(index: &RocketIndex, path: &Path) -> Result<()> {
    let json = serde_json::to_vec_pretty(index)?;
    fs::write(path, json).with_context(|| format!("failed to write index {}", path.display()))
}

pub fn load_index(path: &Path) -> Result<RocketIndex> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read index {}", path.display()))?;
    let index: RocketIndex = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse index {}", path.display()))?;
    if index.version != INDEX_VERSION {
        anyhow::bail!(
            "index {} has version {}, expected {}",
            path.display(),
            index.version,
            INDEX_VERSION
        );
    }
    Ok(index)
}

fn collect_trigrams(bytes: &[u8]) -> Vec<u32> {
    bytes
        .windows(3)
        .map(|window| encode_trigram(window[0], window[1], window[2]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::trigrams_for_index;
    use tempfile::TempDir;

    #[test]
    fn index_builds_and_filters_candidate_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "needle here").unwrap();
        fs::write(dir.path().join("b.txt"), "nothing").unwrap();

        let index = build_index(&IndexBuildOptions {
            walk: WalkOptions {
                paths: vec![dir.path().to_path_buf()],
                globs: Vec::new(),
                types: Vec::new(),
                type_not: Vec::new(),
                hidden: true,
                no_ignore: false,
                follow_links: false,
            },
            text: false,
        })
        .unwrap();

        let candidates = index
            .candidate_paths(&IndexQuery {
                trigrams: trigrams_for_index(b"needle"),
                require_all: true,
            })
            .unwrap();

        assert_eq!(candidates.len(), 1);
        assert!(candidates.iter().any(|path| path.ends_with("a.txt")));
    }
}
