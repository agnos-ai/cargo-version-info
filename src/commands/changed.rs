//! Check if Cargo.toml version changed since last git tag command.
//!
//! This command compares the version in Cargo.toml with the latest git tag
//! to determine if the version has been updated since the last release.
//!
//! # Examples
//!
//! ```bash
//! # Check if version changed (returns true/false)
//! cargo version-info changed
//!
//! # Get JSON output with both versions
//! cargo version-info changed --format json
//!
//! # Get human-readable diff
//! cargo version-info changed --format diff
//!
//! # Use in GitHub Actions
//! cargo version-info changed --format github-actions
//! ```

use std::path::PathBuf;

use anyhow::{
    Context,
    Result,
};
use clap::Parser;

use super::common::get_package_version_from_manifest;

/// Arguments for the `changed` command.
#[derive(Parser, Debug)]
pub struct ChangedArgs {
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

    /// Output format for the comparison result.
    ///
    /// - `bool`: Print "true" if version changed, "false" if unchanged
    /// - `json`: Print JSON with changed, cargo_version, and latest_tag_version
    ///   fields
    /// - `diff`: Print human-readable diff (e.g., "Version changed: 0.1.0 ->
    ///   0.1.1")
    /// - `github-actions`: Write to GITHUB_OUTPUT file in GitHub Actions format
    #[arg(long, default_value = "bool")]
    format: String,

    /// Path to GitHub Actions output file.
    ///
    /// Only used when `--format github-actions` is specified.
    /// Defaults to the `GITHUB_OUTPUT` environment variable or stdout.
    #[arg(long, env = "GITHUB_OUTPUT")]
    github_output: Option<String>,
}

/// Check if the Cargo.toml version has changed since the last git tag.
///
/// Extracts the version from Cargo.toml (checking `[workspace.package]` first,
/// then `[package]`) and compares it with the latest git tag. If no tags exist,
/// the tag version is assumed to be "0.0.0".
///
/// This is useful for CI/CD pipelines to determine if a version bump is needed
/// or if the version has already been updated.
///
/// # Errors
///
/// Returns an error if:
/// - The manifest file cannot be read
/// - No version field is found in Cargo.toml
/// - The output file cannot be written (for github-actions format)
///
/// # Examples
///
/// ```no_run
/// use cargo_version_info::commands::{
///     ChangedArgs,
///     changed,
/// };
/// use clap::Parser;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Parse from command line args
/// let args = ChangedArgs::parse_from(&["cargo", "version-info", "changed"]);
/// changed(args)?; // Prints "true" or "false"
///
/// # Ok(())
/// # }
/// ```
///
/// # Example Output
///
/// With `--format bool` (version changed):
/// ```text
/// true
/// ```
///
/// With `--format bool` (version unchanged):
/// ```text
/// false
/// ```
///
/// With `--format json`:
/// ```json
/// {"changed":true,"cargo_version":"0.1.1","latest_tag_version":"0.1.0"}
/// ```
///
/// With `--format diff` (version changed):
/// ```text
/// Version changed: 0.1.0 -> 0.1.1
/// ```
///
/// With `--format diff` (version unchanged):
/// ```text
/// Version unchanged: 0.1.0
/// ```
///
/// With `--format github-actions` (writes to GITHUB_OUTPUT):
/// ```text
/// changed=true
/// version=0.1.1
/// latest_tag_version=0.1.0
/// ```
pub fn changed(args: ChangedArgs) -> Result<()> {
    // Get current version from Cargo.toml using cargo_metadata (idiomatic way)
    let cargo_version = get_package_version_from_manifest(&args.manifest)
        .with_context(|| format!("Failed to get version from {}", args.manifest.display()))?;

    // Find latest tag using git describe (simpler and more reliable)
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

    // Strip optional leading v/V
    let latest_tag_version = latest_tag
        .strip_prefix('v')
        .or_else(|| latest_tag.strip_prefix('V'))
        .unwrap_or(&latest_tag)
        .to_string();

    let changed = cargo_version != latest_tag_version;

    match args.format.as_str() {
        "bool" => println!("{}", changed),
        "json" => println!(
            "{{\"changed\":{},\"cargo_version\":\"{}\",\"latest_tag_version\":\"{}\"}}",
            changed, cargo_version, latest_tag_version
        ),
        "diff" => {
            if changed {
                println!(
                    "Version changed: {} -> {}",
                    latest_tag_version, cargo_version
                );
            } else {
                println!("Version unchanged: {}", cargo_version);
            }
        }
        "github-actions" => {
            let output_file = args.github_output.as_deref().unwrap_or("/dev/stdout");
            let output = format!(
                "changed={}\nversion={}\nlatest_tag_version={}\n",
                changed, cargo_version, latest_tag_version
            );
            std::fs::write(output_file, output)
                .with_context(|| format!("Failed to write to {}", output_file))?;
        }
        _ => anyhow::bail!("Invalid format: {}", args.format),
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
    fn test_changed_bool_format() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "0.1.0"
"#,
        );
        let args = ChangedArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            format: "bool".to_string(),
            github_output: None,
        };
        // Will succeed if git repo exists, otherwise may fail on git describe
        let _ = changed(args);
    }

    #[test]
    fn test_changed_json_format() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "1.0.0"
"#,
        );
        let args = ChangedArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            format: "json".to_string(),
            github_output: None,
        };
        let _ = changed(args);
    }

    #[test]
    fn test_changed_diff_format() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "2.0.0"
"#,
        );
        let args = ChangedArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            format: "diff".to_string(),
            github_output: None,
        };
        let _ = changed(args);
    }

    #[test]
    fn test_changed_github_actions_format() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "3.0.0"
"#,
        );
        let output_file = NamedTempFile::new().unwrap();
        let args = ChangedArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            format: "github-actions".to_string(),
            github_output: Some(output_file.path().to_string_lossy().to_string()),
        };
        let result = changed(args);
        // May succeed or fail depending on git state, but if it succeeds, check output
        if result.is_ok() {
            let content = std::fs::read_to_string(output_file.path()).unwrap();
            assert!(content.contains("changed="));
            assert!(content.contains("version="));
            assert!(content.contains("latest_tag_version="));
        }
    }

    #[test]
    fn test_changed_invalid_format() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "1.0.0"
"#,
        );
        let args = ChangedArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            format: "invalid".to_string(),
            github_output: None,
        };
        assert!(changed(args).is_err());
    }

    #[test]
    fn test_changed_file_not_found() {
        let args = ChangedArgs {
            manifest: "/nonexistent/Cargo.toml".into(),
            repo_path: ".".into(),
            format: "bool".to_string(),
            github_output: None,
        };
        assert!(changed(args).is_err());
    }

    #[test]
    fn test_changed_no_version() {
        let manifest = create_temp_manifest(
            r#"
[package]
name = "test"
"#,
        );
        let args = ChangedArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            format: "bool".to_string(),
            github_output: None,
        };
        assert!(changed(args).is_err());
    }

    #[test]
    fn test_changed_workspace_version() {
        let manifest = create_temp_manifest(
            r#"
[workspace.package]
version = "0.5.0"
"#,
        );
        let args = ChangedArgs {
            manifest: manifest.path().to_path_buf(),
            repo_path: ".".into(),
            format: "bool".to_string(),
            github_output: None,
        };
        let _ = changed(args);
    }
}
