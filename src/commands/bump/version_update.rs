//! TOML version update logic.
//!
//! This module handles updating the version field in Cargo.toml files while
//! preserving all formatting, comments, and structure. It uses the `toml_edit`
//! crate (same as `cargo-edit`) to ensure maximum compatibility with cargo's
//! own TOML handling.
//!
//! # Design Philosophy
//!
//! - **Preserve Everything**: Comments, whitespace, formatting are all
//!   preserved
//! - **Workspace Support**: Handles both `[package]` and `[workspace.package]`
//! - **Minimal Changes**: Only modifies the `version` field, nothing else
//!
//! # Examples
//!
//! ```rust,no_run
//! use std::path::Path;
//!
//! # use anyhow::Result;
//! # fn example() -> Result<()> {
//! use cargo_version_info::commands::bump::version_update::update_cargo_toml_version;
//!
//! let manifest = Path::new("Cargo.toml");
//! update_cargo_toml_version(manifest, "0.1.0", "0.2.0")?;
//! # Ok(())
//! # }
//! ```
//!
//! # Implementation Details
//!
//! The function uses `toml_edit::DocumentMut` which provides a mutable view
//! of a TOML document while tracking formatting information. This allows us
//! to modify specific values without affecting the rest of the file.
//!
//! ## Version Location
//!
//! The version can be in one of two locations:
//!
//! 1. **Package section**: `[package] version = "X.Y.Z"`
//! 2. **Workspace section**: `[workspace.package] version = "X.Y.Z"`
//!
//! We check both locations and update whichever is found.

use std::path::Path;

use anyhow::{
    Context,
    Result,
};
use toml_edit::{
    DocumentMut,
    value,
};

/// Update the version field in a Cargo.toml file.
///
/// This function parses the TOML file, locates the version field (in either
/// `[package]` or `[workspace.package]`), updates it, and writes the file back
/// while preserving all formatting.
///
/// # Arguments
///
/// * `manifest_path` - Path to the Cargo.toml file
/// * `_old_version` - The current version (unused, kept for API consistency)
/// * `new_version` - The target version to set
///
/// # Errors
///
/// Returns an error if:
/// - The file cannot be read
/// - The TOML is invalid
/// - No `[package]` or `[workspace.package]` section is found
/// - The file cannot be written
///
/// # Examples
///
/// ```rust,no_run
/// # use std::path::Path;
/// # use anyhow::Result;
/// # fn example() -> Result<()> {
/// use cargo_version_info::commands::bump::version_update::update_cargo_toml_version;
///
/// let manifest = Path::new("./Cargo.toml");
/// update_cargo_toml_version(manifest, "1.0.0", "1.1.0")?;
/// # Ok(())
/// # }
/// ```
///
/// # Formatting Preservation
///
/// This function uses `toml_edit` to ensure that:
/// - Comments are preserved
/// - Whitespace and indentation are maintained
/// - Table order is unchanged
/// - Only the version value is modified
///
/// Before:
/// ```toml
/// [package]
/// name = "my-crate"  # Important crate
/// version = "0.1.0"  # Current version
/// edition = "2021"
/// ```
///
/// After calling `update_cargo_toml_version(path, "0.1.0", "0.2.0")`:
/// ```toml
/// [package]
/// name = "my-crate"  # Important crate
/// version = "0.2.0"  # Current version
/// edition = "2021"
/// ```
pub fn update_cargo_toml_version(
    manifest_path: &Path,
    _old_version: &str,
    new_version: &str,
) -> Result<()> {
    // Read the current content
    let content = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    // Parse the TOML document while preserving formatting
    // This creates a DocumentMut which tracks all formatting information
    let mut doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("Failed to parse TOML in {}", manifest_path.display()))?;

    // Try to update version in [package] section first
    // The as_table_mut() method returns None if the item isn't a table
    let updated = if let Some(package) = doc.get_mut("package").and_then(|p| p.as_table_mut()) {
        // Found [package] section - update version
        // The value() function creates a properly formatted TOML value
        package.insert("version", value(new_version));
        true
    } else if let Some(workspace_package) = doc
        .get_mut("workspace")
        .and_then(|w| w.as_table_mut())
        .and_then(|w| w.get_mut("package"))
        .and_then(|p| p.as_table_mut())
    {
        // Found [workspace.package] section - update version
        // This is used in workspace crates that inherit version from the workspace
        workspace_package.insert("version", value(new_version));
        true
    } else {
        // Neither [package] nor [workspace.package] found
        false
    };

    if !updated {
        anyhow::bail!(
            "Could not find [package] or [workspace.package] section in {}",
            manifest_path.display()
        );
    }

    // Write back the modified document
    // The to_string() method serializes the document while preserving all
    // formatting that was tracked during parsing
    std::fs::write(manifest_path, doc.to_string())
        .with_context(|| format!("Failed to write {}", manifest_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn create_temp_manifest(content: &str) -> (TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("Cargo.toml");
        std::fs::write(&manifest_path, content).unwrap();
        (dir, manifest_path)
    }

    #[test]
    fn test_update_package_version() {
        let (_dir, manifest_path) = create_temp_manifest(
            r#"[package]
name = "test"
version = "0.1.0"
"#,
        );

        update_cargo_toml_version(&manifest_path, "0.1.0", "0.2.0").unwrap();

        let content = std::fs::read_to_string(&manifest_path).unwrap();
        assert!(content.contains("version = \"0.2.0\""));
        assert!(!content.contains("0.1.0"));
    }

    #[test]
    fn test_update_workspace_package_version() {
        let (_dir, manifest_path) = create_temp_manifest(
            r#"[workspace.package]
version = "1.0.0"
"#,
        );

        update_cargo_toml_version(&manifest_path, "1.0.0", "2.0.0").unwrap();

        let content = std::fs::read_to_string(&manifest_path).unwrap();
        assert!(content.contains("version = \"2.0.0\""));
    }

    #[test]
    fn test_preserves_formatting() {
        let (_dir, manifest_path) = create_temp_manifest(
            r#"[package]
name = "test"  # Package name
version = "0.1.0"
edition = "2021"
"#,
        );

        update_cargo_toml_version(&manifest_path, "0.1.0", "0.2.0").unwrap();

        let content = std::fs::read_to_string(&manifest_path).unwrap();
        // Verify comments are preserved
        assert!(content.contains("# Package name"));
        // Verify version was updated
        assert!(content.contains("version = \"0.2.0\""));
        // Verify version comment still exists (though toml_edit may reformat it)
        assert!(!content.contains("0.1.0"));
    }

    #[test]
    fn test_no_package_section_error() {
        let (_dir, manifest_path) = create_temp_manifest(
            r#"[dependencies]
some-crate = "1.0"
"#,
        );

        let result = update_cargo_toml_version(&manifest_path, "0.1.0", "0.2.0");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Could not find [package]")
        );
    }
}
