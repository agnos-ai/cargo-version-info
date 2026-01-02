//! Get Rust toolchain version from .rust-toolchain.toml command.
//!
//! This command extracts the Rust toolchain version from a
//! `.rust-toolchain.toml` file by reading the `channel` field.
//!
//! # Examples
//!
//! ```bash
//! # Get toolchain version (e.g., "1.91.0")
//! cargo version-info rust-toolchain
//!
//! # Get JSON output
//! cargo version-info rust-toolchain --format json
//!
//! # Use a different toolchain file
//! cargo version-info rust-toolchain --toolchain-file ./custom/.rust-toolchain.toml
//! ```

use std::path::PathBuf;

use anyhow::{
    Context,
    Result,
};
use clap::Parser;

/// Arguments for the `rust-toolchain` command.
#[derive(Parser, Debug)]
pub struct RustToolchainArgs {
    /// Path to the `.rust-toolchain.toml` file.
    ///
    /// Defaults to `./.rust-toolchain.toml` in the current directory.
    #[arg(long, default_value = "./.rust-toolchain.toml")]
    toolchain_file: PathBuf,

    /// Output format for the toolchain version.
    ///
    /// - `version`: Print just the version number (e.g., "1.91.0")
    /// - `json`: Print JSON with version field
    #[arg(long, default_value = "version")]
    format: String,
}

/// Get the Rust toolchain version from a `.rust-toolchain.toml` file.
///
/// Parses the toolchain file and extracts the `channel` field value, which
/// specifies the Rust version to use. Supports both double-quoted and
/// single-quoted strings.
///
/// # Errors
///
/// Returns an error if:
/// - The toolchain file cannot be read
/// - No `channel` field is found in the file
/// - The channel value cannot be parsed
///
/// # Examples
///
/// ```no_run
/// use cargo_version_info::commands::{
///     RustToolchainArgs,
///     rust_toolchain,
/// };
/// use clap::Parser;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Parse from command line args
/// let args = RustToolchainArgs::parse_from(&["cargo", "version-info", "rust-toolchain"]);
/// rust_toolchain(args)?;
/// # Ok(())
/// # }
/// ```
///
/// # Example Output
///
/// With `--format version`:
/// ```text
/// 1.91.0
/// ```
///
/// With `--format json`:
/// ```json
/// {"version":"1.91.0"}
/// ```
pub fn rust_toolchain(args: RustToolchainArgs) -> Result<()> {
    let content = std::fs::read_to_string(&args.toolchain_file)
        .with_context(|| format!("Failed to read {}", args.toolchain_file.display()))?;

    // Parse channel = "..." from .rust-toolchain.toml
    let version = content
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("channel") {
                // Match: channel = "1.91.0" or channel = '1.91.0'
                if let Some(quote_start) = trimmed.find('"') {
                    let after_quote = &trimmed[quote_start + 1..];
                    if let Some(quote_end) = after_quote.find('"') {
                        return Some(after_quote[..quote_end].to_string());
                    }
                } else if let Some(quote_start) = trimmed.find('\'') {
                    let after_quote = &trimmed[quote_start + 1..];
                    if let Some(quote_end) = after_quote.find('\'') {
                        return Some(after_quote[..quote_end].to_string());
                    }
                }
            }
            None
        })
        .with_context(|| format!("No channel found in {}", args.toolchain_file.display()))?;

    match args.format.as_str() {
        "version" => println!("{}", version),
        "json" => println!("{{\"version\":\"{}\"}}", version),
        _ => anyhow::bail!("Invalid format: {}", args.format),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    fn create_temp_toolchain(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_rust_toolchain_double_quotes() {
        let toolchain_file = create_temp_toolchain(r#"channel = "1.91.0""#);
        let args = RustToolchainArgs {
            toolchain_file: toolchain_file.path().to_path_buf(),
            format: "version".to_string(),
        };
        assert!(rust_toolchain(args).is_ok());
    }

    #[test]
    fn test_rust_toolchain_single_quotes() {
        let toolchain_file = create_temp_toolchain(r#"channel = '1.92.0'"#);
        let args = RustToolchainArgs {
            toolchain_file: toolchain_file.path().to_path_buf(),
            format: "version".to_string(),
        };
        assert!(rust_toolchain(args).is_ok());
    }

    #[test]
    fn test_rust_toolchain_json_format() {
        let toolchain_file = create_temp_toolchain(r#"channel = "2.0.0""#);
        let args = RustToolchainArgs {
            toolchain_file: toolchain_file.path().to_path_buf(),
            format: "json".to_string(),
        };
        assert!(rust_toolchain(args).is_ok());
    }

    #[test]
    fn test_rust_toolchain_no_channel() {
        let toolchain_file = create_temp_toolchain(r#"# No channel here"#);
        let args = RustToolchainArgs {
            toolchain_file: toolchain_file.path().to_path_buf(),
            format: "version".to_string(),
        };
        assert!(rust_toolchain(args).is_err());
    }

    #[test]
    fn test_rust_toolchain_file_not_found() {
        let args = RustToolchainArgs {
            toolchain_file: "/nonexistent/.rust-toolchain.toml".into(),
            format: "version".to_string(),
        };
        assert!(rust_toolchain(args).is_err());
    }

    #[test]
    fn test_rust_toolchain_invalid_format() {
        let toolchain_file = create_temp_toolchain(r#"channel = "1.0.0""#);
        let args = RustToolchainArgs {
            toolchain_file: toolchain_file.path().to_path_buf(),
            format: "invalid".to_string(),
        };
        assert!(rust_toolchain(args).is_err());
    }

    #[test]
    fn test_rust_toolchain_with_spaces() {
        let toolchain_file = create_temp_toolchain(r#"channel = "1.93.0"  "#);
        let args = RustToolchainArgs {
            toolchain_file: toolchain_file.path().to_path_buf(),
            format: "version".to_string(),
        };
        assert!(rust_toolchain(args).is_ok());
    }
}
