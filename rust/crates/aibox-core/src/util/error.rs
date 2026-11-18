//! Shared typed error for file-system helpers in `util`.

use std::io;
use std::path::{Path, PathBuf};

/// File operation failure with the operation and path that caused it, so
/// domain error types can wrap it via `#[from]` without losing context.
#[derive(Debug, thiserror::Error)]
#[error("failed to {op} {}: {source}", path.display())]
pub struct IoError {
    pub op: &'static str,
    pub path: PathBuf,
    #[source]
    pub source: io::Error,
}

impl IoError {
    pub fn new(op: &'static str, path: &Path, source: io::Error) -> Self {
        Self {
            op,
            path: path.to_path_buf(),
            source,
        }
    }
}

// Most callers still report errors as `String`; this keeps `?` working there.
impl From<IoError> for String {
    fn from(err: IoError) -> Self {
        err.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_op_path_and_cause() {
        let err = IoError::new(
            "write",
            Path::new("/tmp/x"),
            io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        );
        assert_eq!(err.to_string(), "failed to write /tmp/x: denied");
        let as_string: String = err.into();
        assert_eq!(as_string, "failed to write /tmp/x: denied");
    }
}
