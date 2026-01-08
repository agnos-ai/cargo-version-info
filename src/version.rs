//! Version parsing and manipulation utilities.

use anyhow::{
    Context,
    Result,
};

/// Parse a semantic version string (e.g., "0.1.2" or "v0.1.2").
pub fn parse_version(version_str: &str) -> Result<(u32, u32, u32)> {
    // Strip optional v/V prefix
    let version_str = version_str.strip_prefix('v').unwrap_or(version_str);
    let version_str = version_str.strip_prefix('V').unwrap_or(version_str);

    let parts: Vec<&str> = version_str.split('.').collect();
    if parts.len() < 3 {
        anyhow::bail!(
            "Version must have at least 3 parts (major.minor.patch), got: {}",
            version_str
        );
    }

    let major = parts[0]
        .parse::<u32>()
        .with_context(|| format!("Invalid major version: {}", parts[0]))?;
    let minor = parts[1]
        .parse::<u32>()
        .with_context(|| format!("Invalid minor version: {}", parts[1]))?;
    let patch = parts[2]
        .split('-')
        .next()
        .unwrap_or(parts[2])
        .parse::<u32>()
        .with_context(|| format!("Invalid patch version: {}", parts[2]))?;

    Ok((major, minor, patch))
}

/// Increment patch version.
pub fn increment_patch(major: u32, minor: u32, patch: u32) -> (u32, u32, u32) {
    (major, minor, patch + 1)
}

/// Increment minor version (resets patch to 0).
pub fn increment_minor(major: u32, minor: u32, _patch: u32) -> (u32, u32, u32) {
    (major, minor + 1, 0)
}

/// Increment major version (resets minor and patch to 0).
pub fn increment_major(major: u32, _minor: u32, _patch: u32) -> (u32, u32, u32) {
    (major + 1, 0, 0)
}

/// Format version as string.
pub fn format_version(major: u32, minor: u32, patch: u32) -> String {
    format!("{}.{}.{}", major, minor, patch)
}

/// Format version as tag (with v prefix).
pub fn format_tag(major: u32, minor: u32, patch: u32) -> String {
    format!("v{}.{}.{}", major, minor, patch)
}

/// Compare two versions.
///
/// Returns:
/// - `Ok(Some(true))` if version1 > version2
/// - `Ok(Some(false))` if version1 < version2
/// - `Ok(None)` if version1 == version2
pub fn compare_versions(version1: &str, version2: &str) -> Result<Option<bool>> {
    let (major1, minor1, patch1) = parse_version(version1)?;
    let (major2, minor2, patch2) = parse_version(version2)?;

    if major1 != major2 {
        return Ok(Some(major1 > major2));
    }
    if minor1 != minor2 {
        return Ok(Some(minor1 > minor2));
    }
    if patch1 != patch2 {
        return Ok(Some(patch1 > patch2));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("0.1.2").unwrap(), (0, 1, 2));
        assert_eq!(parse_version("v0.1.2").unwrap(), (0, 1, 2));
        assert_eq!(parse_version("V1.2.3").unwrap(), (1, 2, 3));
        assert_eq!(parse_version("10.20.30").unwrap(), (10, 20, 30));
    }

    #[test]
    fn test_increment_patch() {
        assert_eq!(increment_patch(0, 1, 2), (0, 1, 3));
        assert_eq!(increment_patch(1, 0, 0), (1, 0, 1));
    }

    #[test]
    fn test_increment_minor() {
        assert_eq!(increment_minor(0, 1, 2), (0, 2, 0));
        assert_eq!(increment_minor(1, 0, 5), (1, 1, 0));
    }

    #[test]
    fn test_increment_major() {
        assert_eq!(increment_major(0, 1, 2), (1, 0, 0));
        assert_eq!(increment_major(1, 5, 10), (2, 0, 0));
    }

    #[test]
    fn test_format_version() {
        assert_eq!(format_version(0, 1, 2), "0.1.2");
        assert_eq!(format_version(10, 20, 30), "10.20.30");
    }

    #[test]
    fn test_format_tag() {
        assert_eq!(format_tag(0, 1, 2), "v0.1.2");
    }

    #[test]
    fn test_compare_versions() {
        assert_eq!(compare_versions("0.1.2", "0.1.3").unwrap(), Some(false));
        assert_eq!(compare_versions("0.1.3", "0.1.2").unwrap(), Some(true));
        assert_eq!(compare_versions("0.1.2", "0.1.2").unwrap(), None);
        assert_eq!(compare_versions("1.0.0", "0.9.9").unwrap(), Some(true));
    }
}
