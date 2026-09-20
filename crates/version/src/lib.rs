//! S00 placeholder crate: proves the workspace builds and tests run
//! on a clean checkout. Replaced/augmented by boot/kernel crates in S01+.

/// Workspace version string for build identification / benchmark records.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Short build identifier used in benchmark reports.
pub fn build_id() -> String {
    format!("baseos-version-{}", version())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_set() {
        assert!(!version().is_empty());
    }

    #[test]
    fn build_id_contains_version() {
        assert!(build_id().contains(version()));
    }
}
