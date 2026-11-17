pub fn core_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_version_matches_package() {
        assert_eq!(core_version(), "0.2.0");
    }
}
