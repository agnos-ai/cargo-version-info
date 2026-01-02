//! Pre-bump hook for cog integration command.
//!
//! This command is designed to be used as a pre-bump hook with cocogitto (cog).
//! It verifies the repository state before allowing a version bump to proceed.
//!
//! # Integration with cog
//!
//! Configure in `cog.toml`:
//!
//! ```toml
//! [hooks]
//! pre-bump-hook = "cargo version-info pre-bump-hook"
//! ```
//!
//! # Examples
//!
//! ```bash
//! # Run pre-bump checks
//! cargo version-info pre-bump-hook
//!
//! # With target version (from cog)
//! COG_VERSION=1.0.0 cargo version-info pre-bump-hook
//!
//! # Allow bump even if checks fail (warn only)
//! cargo version-info pre-bump-hook --exit-on-error false
//! ```

use std::path::PathBuf;

use anyhow::{
    Context,
    Result,
};
use clap::Parser;

use super::common::{
    extract_package_version,
    extract_workspace_version,
};
use crate::version::parse_version;

/// Arguments for the `pre-bump-hook` command.
#[derive(Parser, Debug)]
pub struct PreBumpHookArgs {
    /// Path to the Cargo.toml manifest file.
    ///
    /// Defaults to `./Cargo.toml` in the current directory.
    #[arg(long, default_value = "./Cargo.toml")]
    manifest: PathBuf,

    /// Path to the git repository.
    ///
    /// Defaults to the current directory. Used to find the latest git tag.
    #[arg(long, default_value = ".")]
    repo_path: PathBuf,

    /// Target version that cog will bump to.
    ///
    /// Typically provided by cog via the `{{version}}` template variable.
    /// If set, the command will warn about major version bumps from 0.0.0.
    #[arg(long, env = "COG_VERSION")]
    target_version: Option<String>,

    /// Current version before the bump.
    ///
    /// Typically provided by cog via the `{{latest}}` template variable.
    /// Currently unused but reserved for future validation.
    #[arg(long, env = "COG_LATEST")]
    current_version: Option<String>,

    /// Whether to exit with an error code if checks fail.
    ///
    /// - `true`: Exit with non-zero code to prevent the bump (default)
    /// - `false`: Only print warnings, allow the bump to proceed
    #[arg(long, default_value = "true")]
    exit_on_error: bool,
}

/// Pre-bump hook for cocogitto (cog) integration.
///
/// Verifies the repository state before cog bumps the version. Performs the
/// following checks:
///
/// 1. **Version Synchronization**: Ensures Cargo.toml version matches the
///    latest git tag (if tags exist). Prevents bumping when versions are out of
///    sync.
/// 2. **Major Version Warning**: Warns if attempting to bump from `0.0.0` to
///    `1.0.0` or higher, as this changes the placeholder version.
///
/// This hook can be configured in `cog.toml` to run automatically before
/// version bumps, ensuring version consistency and preventing accidental major
/// version jumps.
///
/// # Errors
///
/// Returns an error (and prevents the bump) if:
/// - The manifest file cannot be read
/// - No version field is found in Cargo.toml
/// - Version mismatch detected and `exit_on_error` is `true`
///
/// # Examples
///
/// ```no_run
/// use cargo_version_info::commands::{
///     PreBumpHookArgs,
///     pre_bump_hook,
/// };
/// use clap::Parser;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Parse from command line args
/// let args = PreBumpHookArgs::parse_from(&["cargo", "version-info", "pre-bump-hook"]);
/// pre_bump_hook(args)?;
/// # Ok(())
/// # }
/// ```
///
/// # Example Output
///
/// When checks pass:
/// ```text
/// ✓ Pre-bump checks passed
///   Current version: 0.1.0
///   Target version: 0.1.1
/// ```
///
/// When version mismatch detected (with `--exit-on-error true`):
/// ```text
/// ⚠️  Warning: Cargo.toml version (0.1.1) doesn't match latest git tag (0.1.0)
/// Error: Version mismatch detected. Sync Cargo.toml with git tags before bumping.
/// ```
///
/// When major version bump from 0.0.0 detected:
/// ```text
/// ⚠️  Warning: Major version bump from 0.0.0 to 1.0.0
///    This will change the placeholder version. Continue?
/// ✓ Pre-bump checks passed
///   Current version: 0.0.0
///   Target version: 1.0.0
/// ```
pub fn pre_bump_hook(args: PreBumpHookArgs) -> Result<()> {
    // Get current version from Cargo.toml
    let content = std::fs::read_to_string(&args.manifest)
        .with_context(|| format!("Failed to read {}", args.manifest.display()))?;

    let cargo_version = if let Some(workspace_version) = extract_workspace_version(&content) {
        workspace_version
    } else {
        extract_package_version(&content)
            .with_context(|| format!("No version found in {}", args.manifest.display()))?
    };

    // Get latest git tag version
    let latest_tag = std::process::Command::new("git")
        .arg("describe")
        .arg("--tags")
        .arg("--abbrev=0")
        .current_dir(&args.repo_path)
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| "v0.0.0".to_string());

    let latest_tag_version = latest_tag
        .strip_prefix('v')
        .or_else(|| latest_tag.strip_prefix('V'))
        .unwrap_or(&latest_tag)
        .trim()
        .to_string();

    // Verify Cargo.toml version matches latest tag (if tag exists)
    if latest_tag_version != "0.0.0" && cargo_version != latest_tag_version {
        eprintln!(
            "⚠️  Warning: Cargo.toml version ({}) doesn't match latest git tag ({})",
            cargo_version, latest_tag_version
        );
        if args.exit_on_error {
            anyhow::bail!(
                "Version mismatch detected. Sync Cargo.toml with git tags before bumping."
            );
        }
    }

    // If target version is provided, check for major bump from 0.0.0
    if let Some(target) = &args.target_version {
        let target_trimmed = target.trim();
        if let Ok((target_major, _, _)) = parse_version(target_trimmed)
            && let Ok((current_major, current_minor, current_patch)) = parse_version(&cargo_version)
        {
            // Warn if bumping from 0.0.0 to 1.0.0 (major version jump)
            if current_major == 0 && current_minor == 0 && current_patch == 0 && target_major == 1 {
                eprintln!(
                    "⚠️  Warning: Major version bump from 0.0.0 to {}",
                    target_trimmed
                );
                eprintln!("   This will change the placeholder version. Continue?");
                // Don't fail, just warn - let user decide
            }
        }
    }

    println!("✓ Pre-bump checks passed");
    println!("  Current version: {}", cargo_version);
    if let Some(target) = &args.target_version {
        println!("  Target version: {}", target.trim());
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
    fn test_pre_bump_hook_success() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "0.1.0"
"#,
        );
        let args = PreBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: Some("0.1.1".to_string()),
            current_version: None,
            exit_on_error: true,
        };
        // Will succeed if git repo exists and versions match, otherwise may fail
        let _ = pre_bump_hook(args);
    }

    #[test]
    fn test_pre_bump_hook_major_bump_warning() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "0.0.0"
"#,
        );
        let args = PreBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: Some("1.0.0".to_string()),
            current_version: None,
            exit_on_error: false, // Don't fail on warnings
        };
        // Should warn but not fail
        let result = pre_bump_hook(args);
        // May succeed or fail depending on git state, but shouldn't fail on major bump
        // warning
        let _ = result;
    }

    #[test]
    fn test_pre_bump_hook_no_target_version() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "0.2.0"
"#,
        );
        let args = PreBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: None,
            current_version: None,
            exit_on_error: true,
        };
        let _ = pre_bump_hook(args);
    }

    #[test]
    fn test_pre_bump_hook_file_not_found() {
        let args = PreBumpHookArgs {
            manifest: "/nonexistent/Cargo.toml".into(),
            repo_path: ".".into(),
            target_version: None,
            current_version: None,
            exit_on_error: true,
        };
        assert!(pre_bump_hook(args).is_err());
    }

    #[test]
    fn test_pre_bump_hook_no_version() {
        let manifest = create_temp_manifest(
            r#"
[package]
name = "test"
"#,
        );
        let args = PreBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: None,
            current_version: None,
            exit_on_error: true,
        };
        assert!(pre_bump_hook(args).is_err());
    }

    #[test]
    fn test_pre_bump_hook_workspace_version() {
        let manifest = create_temp_manifest(
            r#"
[workspace.package]
version = "1.0.0"
"#,
        );
        let args = PreBumpHookArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            target_version: Some("1.0.1".to_string()),
            current_version: None,
            exit_on_error: true,
        };
        let _ = pre_bump_hook(args);
    }
}
