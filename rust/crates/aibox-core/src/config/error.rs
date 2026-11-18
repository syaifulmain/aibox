//! Typed error for configuration loading, validation, and persistence.

use std::path::PathBuf;

use crate::util::error::IoError;

/// Structured error returned by configuration and settings operations.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] IoError),
    #[error("failed to parse config {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{reason}")]
    Invalid { field: &'static str, reason: String },
}

impl ConfigError {
    /// Constructs a `ConfigError::Invalid` for a specific configuration field.
    pub fn invalid(field: &'static str, reason: impl Into<String>) -> Self {
        Self::Invalid {
            field,
            reason: reason.into(),
        }
    }

    /// Constructs a `ConfigError::Parse` for a JSON syntax or type failure.
    pub fn parse(path: impl Into<PathBuf>, source: serde_json::Error) -> Self {
        Self::Parse {
            path: path.into(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::path::Path;

    #[test]
    fn error_variants_format_clearly() {
        let io_err = ConfigError::from(IoError::new(
            "read",
            Path::new("/tmp/config.json"),
            io::Error::new(io::ErrorKind::NotFound, "not found"),
        ));
        assert!(matches!(io_err, ConfigError::Io(_)));
        assert!(io_err.to_string().contains("/tmp/config.json"));

        let serde_err = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let parse_err = ConfigError::parse("/tmp/broken.json", serde_err);
        assert!(matches!(parse_err, ConfigError::Parse { .. }));
        assert!(parse_err.to_string().contains("/tmp/broken.json"));

        let invalid_err = ConfigError::invalid("staleDays", "Stale threshold out of range.");
        assert_eq!(invalid_err.to_string(), "Stale threshold out of range.");
    }
}
