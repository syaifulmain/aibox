//! Typed error for secrets storage, state, and injection operations.
//!
//! `Display` and `Debug` implementations must never include secret values —
//! only secret names, file paths, permission modes, or static messages.

use std::path::PathBuf;

use crate::util::error::IoError;

/// Structured error returned by secrets store, state, and injection operations.
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error(transparent)]
    Io(#[from] IoError),
    #[error("Invalid secrets store path.")]
    InvalidPath,
    #[error("Invalid secret name '{name}'. Use ^[A-Z_][A-Z0-9_]*$.")]
    InvalidSecretName { name: String },
    #[error("Secret name must match ^[A-Z_][A-Z0-9_]*$.")]
    InvalidSecretNameFormat,
    #[error(
        "Refusing to read {} with insecure permissions ({mode:o}). Run: chmod 600 {}",
        path.display(),
        path.display()
    )]
    InsecurePermissions { path: PathBuf, mode: u32 },
    #[error("Invalid JSON in secrets file {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("Could not serialize secrets store: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("Select an environment before {action}.")]
    NoEnvironmentSelected { action: &'static str },
    #[error("{message}")]
    GlobalSecretReadOnly { name: String, message: String },
    #[error("{}", secret_not_found_msg(name, env.as_deref()))]
    SecretNotFound { name: String, env: Option<String> },
    #[error("{0}")]
    TempFile(&'static str),
    #[error("{0}")]
    InjectFailed(String),
}

fn secret_not_found_msg(name: &str, env: Option<&str>) -> String {
    match env {
        Some(env_name) => format!("Secret '{name}' does not belong to environment '{env_name}'."),
        None => format!("Secret '{name}' was not found."),
    }
}

impl From<SecretsError> for String {
    fn from(err: SecretsError) -> Self {
        err.to_string()
    }
}
