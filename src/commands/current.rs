//! Get current version from Cargo.toml command.
//!
//! This command extracts the version from a Cargo.toml file, checking both
//! `[workspace.package]` and `[package]` sections.
//!
//! # Examples
//!
//! ```bash
//! # Get version from current directory's Cargo.toml
//! cargo version-info current
//!
//! # Get version from a specific Cargo.toml
//! cargo version-info current --manifest ./path/to/Cargo.toml
//!
//! # Get JSON output
//! cargo version-info current --format json
//!
//! # Use in GitHub Actions
//! cargo version-info current --format github-actions
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

/// Arguments for the `current` command.
#[derive(Parser, Debug)]
pub struct CurrentArgs {
    /// Path to the Cargo.toml manifest file.
    ///
    /// Defaults to `./Cargo.toml` in the current directory.
    #[arg(long, default_value = "./Cargo.toml")]
    manifest: PathBuf,

    /// Output format for the version.
    ///
    /// - `version`: Print just the version number (e.g., "0.1.2")
    /// - `json`: Print JSON with version field
    /// - `github-actions`: Write to GITHUB_OUTPUT file in GitHub Actions format
    #[arg(long, default_value = "version")]
    format: String,

    /// Path to GitHub Actions output file.
    ///
    /// Only used when `--format github-actions` is specified.
    /// Defaults to the `GITHUB_OUTPUT` environment variable or stdout.
    #[arg(long, env = "GITHUB_OUTPUT")]
    github_output: Option<String>,
}

/// Get the current version from a Cargo.toml manifest file.
///
/// Extracts the version from the manifest, checking `[workspace.package]`
/// first (for workspace members), then falling back to `[package]`.
///
/// # Errors
///
/// Returns an error if:
/// - The manifest file cannot be read
/// - No version field is found in either `[workspace.package]` or `[package]`
/// - The output file cannot be written (for github-actions format)
///
/// # Examples
///
/// ```no_run
/// use cargo_version_info::commands::{
///     CurrentArgs,
///     current,
/// };
/// use clap::Parser;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Parse from command line args
/// let args = CurrentArgs::parse_from(&["cargo", "version-info", "current"]);
/// current(args)?;
/// # Ok(())
/// # }
/// ```
///
/// # Example Output
///
/// With `--format version`:
/// ```text
/// 0.1.2
/// ```
///
/// With `--format json`:
/// ```json
/// {"version":"0.1.2"}
/// ```
///
/// With `--format github-actions` (writes to GITHUB_OUTPUT):
/// ```text
/// version=0.1.2
/// ```
pub fn current(args: CurrentArgs) -> Result<()> {
    let content = std::fs::read_to_string(&args.manifest)
        .with_context(|| format!("Failed to read {}", args.manifest.display()))?;

    // Try workspace.package.version first
    let version = if let Some(workspace_version) = extract_workspace_version(&content) {
        workspace_version
    } else {
        // Fall back to package.version
        extract_package_version(&content)
            .with_context(|| format!("No version found in {}", args.manifest.display()))?
    };

    match args.format.as_str() {
        "version" => println!("{}", version),
        "json" => println!("{{\"version\":\"{}\"}}", version),
        "github-actions" => {
            let output_file = args.github_output.as_deref().unwrap_or("/dev/stdout");
            let output = format!("version={}\n", version);
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
    fn test_current_workspace_version() {
        let manifest = create_temp_manifest(
            r#"
[workspace.package]
version = "0.1.2"
"#,
        );
        let args = CurrentArgs {
            manifest: manifest.path().to_path_buf(),
            format: "version".to_string(),
            github_output: None,
        };
        assert!(current(args).is_ok());
    }

    #[test]
    fn test_current_package_version() {
        let manifest = create_temp_manifest(
            r#"
[package]
name = "test"
version = "1.2.3"
"#,
        );
        let args = CurrentArgs {
            manifest: manifest.path().to_path_buf(),
            format: "version".to_string(),
            github_output: None,
        };
        assert!(current(args).is_ok());
    }

    #[test]
    fn test_current_json_format() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "0.5.0"
"#,
        );
        let args = CurrentArgs {
            manifest: manifest.path().to_path_buf(),
            format: "json".to_string(),
            github_output: None,
        };
        assert!(current(args).is_ok());
    }

    #[test]
    fn test_current_github_actions_format() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "2.0.0"
"#,
        );
        let output_file = NamedTempFile::new().unwrap();
        let args = CurrentArgs {
            manifest: manifest.path().to_path_buf(),
            format: "github-actions".to_string(),
            github_output: Some(output_file.path().to_string_lossy().to_string()),
        };
        assert!(current(args).is_ok());

        let content = std::fs::read_to_string(output_file.path()).unwrap();
        assert!(content.contains("version=2.0.0"));
    }

    #[test]
    fn test_current_invalid_format() {
        let manifest = create_temp_manifest(
            r#"
[package]
version = "1.0.0"
"#,
        );
        let args = CurrentArgs {
            manifest: manifest.path().to_path_buf(),
            format: "invalid".to_string(),
            github_output: None,
        };
        assert!(current(args).is_err());
    }

    #[test]
    fn test_current_file_not_found() {
        let args = CurrentArgs {
            manifest: "/nonexistent/Cargo.toml".into(),
            format: "version".to_string(),
            github_output: None,
        };
        assert!(current(args).is_err());
    }

    #[test]
    fn test_current_no_version() {
        let manifest = create_temp_manifest(
            r#"
[package]
name = "test"
"#,
        );
        let args = CurrentArgs {
            manifest: manifest.path().to_path_buf(),
            format: "version".to_string(),
            github_output: None,
        };
        assert!(current(args).is_err());
    }
}
