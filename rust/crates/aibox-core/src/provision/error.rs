//! Typed validation and planning errors for environment provisioning.

/// Structured error returned when validating or planning provisioning operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProvisionError {
    #[error("Another operation is currently in progress. Please wait or cancel it first.")]
    Busy,
    #[error("Please enter a valid environment name (letters, numbers, hyphens).")]
    InvalidName,
    #[error("Environment '{0}' already exists.")]
    AlreadyExists(String),
    #[error("Environment '{0}' was not found.")]
    NotFound(String),
    #[error("Source environment '{0}' was not found.")]
    SourceNotFound(String),
    #[error("Select an environment first.")]
    EmptySelection,
    #[error("CPU cores must be between 1 and 1024.")]
    InvalidCpuCores,
    #[error("The disk quota is invalid. Use a value such as 35GiB.")]
    InvalidDiskQuota,
    #[error("The RAM limit is invalid. Use a value such as 6GiB.")]
    InvalidRamLimit,
    #[error("The process limit is invalid. Use an integer >= 10 (e.g. 512) or leave unlimited.")]
    InvalidProcessLimit,
    #[error("The disk I/O limit is invalid. Use a rate such as 200MB or 2500iops.")]
    InvalidIoLimit,
    #[error("The swap limit is invalid. Use Host, Off, or a size such as 4GiB.")]
    InvalidSwapLimit,
    #[error("{0}")]
    InvalidWorkspacePath(String),
    #[error("Cannot shrink disk quota ({target}) below current usage ({current}).")]
    DiskShrinkBelowUsage { target: String, current: String },
    #[error("There is no OS in the catalog. Add one in Configuration first.")]
    EmptyOsCatalog,
    #[error("There is no capability preset. Add one in Configuration first.")]
    EmptyPresetCatalog,
    #[error("Templates cannot be started directly. Clone or unmark as template first.")]
    TemplateStartNotAllowed,
    #[error("Unknown OS catalog entry: {0}")]
    UnknownOsEntry(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provision_error_messages_are_clear() {
        assert_eq!(
            ProvisionError::Busy.to_string(),
            "Another operation is currently in progress. Please wait or cancel it first."
        );
        assert_eq!(
            ProvisionError::AlreadyExists("dev".into()).to_string(),
            "Environment 'dev' already exists."
        );
        assert_eq!(
            ProvisionError::DiskShrinkBelowUsage {
                target: "10GiB".into(),
                current: "20.0 GiB".into(),
            }
            .to_string(),
            "Cannot shrink disk quota (10GiB) below current usage (20.0 GiB)."
        );
        assert_eq!(
            ProvisionError::InvalidSwapLimit.to_string(),
            "The swap limit is invalid. Use Host, Off, or a size such as 4GiB."
        );
    }
}
