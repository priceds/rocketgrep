pub mod approx;
pub mod index;
pub mod matcher;
pub mod output;
pub mod pillar;
pub mod searcher;
pub mod seaweed;
pub mod walk;

pub use index::{build_index, load_index, save_index, IndexBuildOptions, RocketIndex};
pub use matcher::{ApproxAlgorithm, LineMatch, MatcherConfig, PatternMatcher};
pub use output::{render_results, ColorChoice, OutputFormat, RenderOptions};
pub use searcher::{search_path, FileSearchResult, LineKind, OutputLine, SearchOptions};
pub use walk::{collect_files, WalkOptions, WalkOutput};
