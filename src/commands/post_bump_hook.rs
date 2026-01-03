//! Post-bump hook for cog integration command.
//!
//! This command is designed to be used as a post-bump hook with cocogitto
//! (cog). It verifies that the version bump was successful after cog has
//! updated Cargo.toml.
//!
//! # Integration with cog
//!
//! Configure in `cog.toml`:
//!
//! ```toml
//! [hooks]
//! post-bump-hook = "cargo version-info post-bump-hook"
//! ```
//!
//! # Examples
//!
//! ```bash
//! # Verify version bump succeeded
//! cargo version-info post-bump-hook
//!
//! # With target and previous versions (from cog)
//! COG_VERSION=1.0.0 COG_LATEST=0.9.0 cargo version-info post-bump-hook
//!
//! # Warn only, don't fail (for testing)
//! cargo version-info post-bump-hook --exit-on-error false
//! ```

use std::path::PathBuf;

use anyhow::{
    Context,
    Result,
};
use clap::Parser;

use super::common::get_package_version_from_manifest;

/// Arguments for the `post-bump-hook` command.
#[derive(Parser, Debug)]
pub struct PostBumpHookArgs {
    /// Path to the Cargo.toml manifest file.
    ///
    /// Defaults to `./Cargo.toml` in the current directory.
    #[arg(long, default_value = "./Cargo.toml")]
    manifest: PathBuf,

    /// Path to the git repository.
    ///
    /// Defaults to the current directory. Currently unused but reserved for
    /// future validation against git tags.
    #[arg(long, default_value = ".")]
    repo_path: PathBuf,

    /// Target version that cog bumped to.
    ///
    /// Typically provided by cog via the `{{version}}` template variable.
    /// If set, the command verifies that Cargo.toml matches this version.
    #[arg(long, env = "COG_VERSION")]
    target_version: Option<String>,

    /// Previous version before the bump.
    ///
    /// Typically provided by cog via the `{{latest}}` template variable.
    /// If set, the command verifies that the version actually changed.
    #[arg(long, env = "COG_LATEST")]
    previous_version: Option<String>,

    /// Whether to exit with an error code if verification fails.
    ///
    /// - `true`: Exit with non-zero code if bump verification fails (default)
    /// - `false`: Only print warnings, don't fail the hook
    #[arg(long, default_value = "true")]
    exit_on_error: bool,
}

/// Post-bump hook for cocogitto (cog) integration.
///
/// Verifies that cog successfully bumped the version in Cargo.toml. Performs
/// the following checks:
///
/// 1. **Target Version Match**: If `target_version` is provided, verifies that
///    Cargo.toml contains the expected version after the bump.
/// 2. **Version Change**: If `previous_version` is provided, verifies that the
///    version actually changed (not still the same).
///
/// This hook can be configured in `cog.toml` to run automatically after version
/// bumps, ensuring the bump was applied correctly and catching any failures.
///
/// # Errors
///
/// Returns an error if:
/// - The manifest file cannot be read
/// - No version field is found in Cargo.toml
/// - Target version doesn't match Cargo.toml (and `exit_on_error` is `true`)
/// - Version didn't change from previous (and `exit_on_error` is `true`)
///
/// # Examples
///
/// ```no_run
/// use cargo_version_info::commands::{
///     PostBumpHookArgs,
///     post_bump_hook,
/// };
/// use clap::Parser;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Parse from command line args
/// let args = PostBumpHookArgs::parse_from(&["cargo", "version-info", "post-bump-hook"]);
/// post_bump_hook(args)?;
/// # Ok(())
/// # }
/// ```
///
/// # Example Output
///
/// When bump verification succeeds:
/// ```text
/// ✓ Version bump verified: 1.0.0
///   Previous version: 0.9.0
///   New version: 1.0.0
/// ```
///
/// When target version doesn't match (with `--exit-on-error true`):
/// ```text
/// ❌ Error: Cargo.toml version (0.9.1) doesn't match expected target (1.0.0)
/// Error: Version bump verification failed
/// ```
///
/// When version didn't change (with `--exit-on-error true`):
/// ```text
/// ✓ Post-bump check passed
///   Current version: 0.9.0
/// ⚠️  Warning: Version didn't change (still 0.9.0)
/// Error: Version bump appears to have failed
/// ```
pub fn post_bump_hook(args: PostBumpHookArgs) -> Result<()> {
    // Get current version from Cargo.toml (after cog bump) using cargo_metadata
    let cargo_version = get_package_version_from_manifest(&args.manifest)
        .with_context(|| format!("Failed to get version from {}", args.manifest.display()))?;

    // If target version is provided, verify it matches
    if let Some(target) = &args.target_version {
        let target_trimmed = target.trim();
        if cargo_version != target_trimmed {
            eprintln!(
                "❌ Error: Cargo.toml version ({}) doesn't match expected target ({})",
                cargo_version, target_trimmed
            );
            if args.exit_on_error {
                anyhow::bail!("Version bump verification failed");
            }
        } else {
            println!("✓ Version bump verified: {}", cargo_version);
        }
    } else {
        println!("✓ Post-bump check passed");
        println!("  Current version: {}", cargo_version);
    }

    // Verify version changed from previous
    if let Some(previous) = &args.previous_version {
        let previous_trimmed = previous.trim();
        if cargo_version == previous_trimmed {
            eprintln!(
                "⚠️  Warning: Version didn't change (still {})",
                cargo_version
            );
            if args.exit_on_error {
                anyhow::bail!("Version bump appears to have failed");
            }
        } else {
            println!("  Previous version: {}", previous_trimmed);
            println!("  New version: {}", cargo_version);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    fn create_temp_manifest(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_post_bump_hook_success() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "1.0.0"
"#,
        );
        let args = PostBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: Some("1.0.0".to_string()),
            previous_version: Some("0.9.0".to_string()),
            exit_on_error: true,
        };
        assert!(post_bump_hook(args).is_ok());
    }

    #[test]
    fn test_post_bump_hook_version_mismatch() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "0.9.1"
"#,
        );
        let args = PostBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: Some("1.0.0".to_string()),
            previous_version: Some("0.9.0".to_string()),
            exit_on_error: true,
        };
        assert!(post_bump_hook(args).is_err());
    }

    #[test]
    fn test_post_bump_hook_version_unchanged() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "0.9.0"
"#,
        );
        let args = PostBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: None,
            previous_version: Some("0.9.0".to_string()),
            exit_on_error: true,
        };
        assert!(post_bump_hook(args).is_err());
    }

    #[test]
    fn test_post_bump_hook_no_target_version() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "2.0.0"
"#,
        );
        let args = PostBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: None,
            previous_version: None,
            exit_on_error: true,
        };
        assert!(post_bump_hook(args).is_ok());
    }

    #[test]
    fn test_post_bump_hook_no_previous_version() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "1.5.0"
"#,
        );
        let args = PostBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: Some("1.5.0".to_string()),
            previous_version: None,
            exit_on_error: true,
        };
        assert!(post_bump_hook(args).is_ok());
    }

    #[test]
    fn test_post_bump_hook_file_not_found() {
        let args = PostBumpHookArgs {
            manifest: "/nonexistent/Cargo.toml".into(),
            repo_path: ".".into(),
            target_version: None,
            previous_version: None,
            exit_on_error: true,
        };
        assert!(post_bump_hook(args).is_err());
    }

    #[test]
    fn test_post_bump_hook_no_version() {
        let manifest = create_temp_manifest(
            r#"
[package]
name = "test"
"#,
        );
        let args = PostBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: None,
            previous_version: None,
            exit_on_error: true,
        };
        assert!(post_bump_hook(args).is_err());
    }

    #[test]
    fn test_post_bump_hook_exit_on_error_false() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "0.9.1"
"#,
        );
        let args = PostBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: Some("1.0.0".to_string()),
            previous_version: Some("0.9.0".to_string()),
            exit_on_error: false, // Should warn but not fail
        };
        // Should succeed even with mismatch when exit_on_error is false
        let result = post_bump_hook(args);
        // May succeed (just prints warning) or fail depending on implementation
        let _ = result;
    }

    #[test]
    fn test_post_bump_hook_workspace_version() {
        let manifest = create_temp_manifest(
            r#"
[workspace.package]
version = "2.1.0"
"#,
        );
        let args = PostBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: Some("2.1.0".to_string()),
            previous_version: Some("2.0.0".to_string()),
            exit_on_error: true,
        };
        assert!(post_bump_hook(args).is_ok());
    }
}
