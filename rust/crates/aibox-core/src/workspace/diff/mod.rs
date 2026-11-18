mod file;
mod restore;
mod rsync;

pub use crate::config::defaults::default_diff_excludes;
pub use crate::config::settings::parse_diff_excludes;
pub use file::{
    cap_diff_lines, is_likely_binary_data, is_safe_relative_path, prepare_file_diff,
    shape_file_diff_output, FileDiffOutput, FileDiffPrep, BINARY_SAMPLE_BYTES,
    DEFAULT_MAX_DIFF_LINES, MAX_PREVIEW_BYTES,
};
pub use restore::restore_single_file;
pub use rsync::{
    build_rsync_exclude_args, build_workspace_diff_args, parse_rsync_itemize,
    process_workspace_diff_output, workspace_root_mtime_nanos, DiffEntry, DiffSummary,
    ParsedRsyncItemize, StreamingRsyncParser, WorkspaceDiffCache, WorkspaceDiffResult,
    DEFAULT_MAX_DIFF_ENTRIES,
};
