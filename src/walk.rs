use anyhow::{Context, Result};
use ignore::overrides::OverrideBuilder;
use ignore::types::TypesBuilder;
use ignore::WalkBuilder;
use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct WalkOptions {
    pub paths: Vec<PathBuf>,
    pub globs: Vec<String>,
    pub types: Vec<String>,
    pub type_not: Vec<String>,
    pub hidden: bool,
    pub no_ignore: bool,
    pub follow_links: bool,
}

#[derive(Clone, Debug)]
pub struct WalkOutput {
    pub paths: Vec<PathBuf>,
    pub errors: Vec<String>,
}

pub fn collect_files(options: &WalkOptions) -> Result<WalkOutput> {
    let roots = if options.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        options.paths.clone()
    };

    let mut builder = WalkBuilder::new(&roots[0]);
    for root in roots.iter().skip(1) {
        builder.add(root);
    }

    builder.hidden(!options.hidden);
    builder.follow_links(options.follow_links);
    builder.require_git(false);

    if options.no_ignore {
        builder
            .ignore(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .parents(false);
    }

    if !options.globs.is_empty() {
        let current_dir = env::current_dir().context("failed to read current directory")?;
        let mut overrides = OverrideBuilder::new(current_dir);
        for glob in &options.globs {
            overrides
                .add(glob)
                .with_context(|| format!("invalid glob override {glob:?}"))?;
        }
        builder.overrides(overrides.build()?);
    }

    if !options.types.is_empty() || !options.type_not.is_empty() {
        let mut types = TypesBuilder::new();
        types.add_defaults();
        for selected in &options.types {
            types.select(selected);
        }
        for negated in &options.type_not {
            types.negate(negated);
        }
        builder.types(types.build()?);
    }

    let mut paths = Vec::new();
    let mut errors = Vec::new();

    for entry in builder.build() {
        match entry {
            Ok(entry) => {
                if entry
                    .file_type()
                    .is_some_and(|file_type| file_type.is_file())
                {
                    paths.push(entry.into_path());
                }
            }
            Err(err) => errors.push(err.to_string()),
        }
    }

    paths.sort();
    paths.dedup();

    Ok(WalkOutput { paths, errors })
}
