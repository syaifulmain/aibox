pub mod backup;
pub mod config;
pub mod health;
pub mod history;
pub mod i18n;
pub mod incus;
pub mod network;
pub mod provision;
pub mod secrets;
pub mod session;
pub mod snapshot;
pub mod theme;
pub mod util;
pub mod workspace;

pub fn core_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_version_is_non_empty() {
        assert!(!core_version().is_empty());
    }
}
