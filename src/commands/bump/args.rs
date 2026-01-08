//! Command-line arguments for the bump command.
//!
//! This module defines the [`BumpArgs`] struct which represents all possible
//! command-line arguments for the `cargo version-info bump` subcommand.
//!
//! # Version Selection
//!
//! The bump command supports multiple mutually-exclusive ways to determine
//! the target version:
//!
//! - **Manual**: `--version X.Y.Z` - Explicitly set the version
//! - **Auto**: `--auto` - Automatically suggest from GitHub releases
//! - **Major**: `--major` - Increment major version (X.0.0)
//! - **Minor**: `--minor` - Increment minor version (X.Y.0)
//! - **Patch**: `--patch` - Increment patch version (X.Y.Z)
//!
//! # Examples
//!
//! ```bash
//! # Bump patch version (most common)
//! cargo version-info bump --patch
//!
//! # Bump minor version (new features)
//! cargo version-info bump --minor
//!
//! # Bump major version (breaking changes)
//! cargo version-info bump --major
//!
//! # Set specific version
//! cargo version-info bump --version 2.0.0
//!
//! # Auto-suggest from GitHub releases
//! cargo version-info bump --auto --github-token $TOKEN
//! ```

use std::path::PathBuf;

use clap::Parser;

/// Arguments for the `bump` command.
///
/// This struct uses `clap`'s derive macros to automatically parse command-line
/// arguments. The conflicts_with_all attributes ensure that only one version
/// selection method can be used at a time.
#[derive(Parser, Debug)]
pub struct BumpArgs {
    /// Path to the Cargo.toml manifest file (standard cargo flag).
    ///
    /// When running as a cargo subcommand, this is automatically handled by
    /// cargo itself. When running standalone, you can specify a custom path.
    ///
    /// # Examples
    ///
    /// ```bash
    /// cargo version-info bump --manifest-path ../other-crate/Cargo.toml --patch
    /// ```
    #[arg(long)]
    pub manifest_path: Option<PathBuf>,

    /// Manually set the target version.
    ///
    /// Use this to set an explicit version number. The version must be a valid
    /// semantic version (e.g., "1.2.3").
    ///
    /// This option conflicts with all other version selection methods.
    #[arg(long, conflicts_with_all = ["auto", "major", "minor", "patch"])]
    pub version: Option<String>,

    /// Automatically suggest the target version from GitHub releases.
    ///
    /// This queries the GitHub API to find the latest release and suggests
    /// the next appropriate version. Requires `--owner` and `--repo` (or
    /// the `GITHUB_REPOSITORY` environment variable).
    ///
    /// Optionally use `--github-token` or `GITHUB_TOKEN` env var for
    /// authenticated requests (higher rate limits).
    #[arg(short = 'a', long, conflicts_with_all = ["version", "major", "minor", "patch"])]
    pub auto: bool,

    /// Increment the major version (X.0.0).
    ///
    /// This resets minor and patch to 0. Use for breaking changes.
    ///
    /// # Examples
    ///
    /// ```text
    /// 1.2.3 -> 2.0.0
    /// 0.5.2 -> 1.0.0
    /// ```
    #[arg(short = 'M', long, conflicts_with_all = ["version", "auto", "minor", "patch"])]
    pub major: bool,

    /// Increment the minor version (X.Y.0).
    ///
    /// This resets patch to 0. Use for new features (non-breaking).
    ///
    /// # Examples
    ///
    /// ```text
    /// 1.2.3 -> 1.3.0
    /// 0.5.2 -> 0.6.0
    /// ```
    #[arg(short = 'm', long, conflicts_with_all = ["version", "auto", "major", "patch"])]
    pub minor: bool,

    /// Increment the patch version (X.Y.Z).
    ///
    /// This is the most common operation for bug fixes and minor updates.
    /// If no version selection flag is provided, this is the default.
    ///
    /// # Examples
    ///
    /// ```text
    /// 1.2.3 -> 1.2.4
    /// 0.5.2 -> 0.5.3
    /// ```
    #[arg(short = 'p', long, conflicts_with_all = ["version", "auto", "major", "minor"])]
    pub patch: bool,

    /// GitHub repository owner (for --auto).
    ///
    /// Defaults to `GITHUB_REPOSITORY` environment variable (format:
    /// "owner/repo") or auto-detected from the current git remote.
    #[arg(long)]
    pub owner: Option<String>,

    /// GitHub repository name (for --auto).
    ///
    /// Defaults to `GITHUB_REPOSITORY` environment variable (format:
    /// "owner/repo") or auto-detected from the current git remote.
    #[arg(long)]
    pub repo: Option<String>,

    /// GitHub personal access token for API authentication (for --auto).
    ///
    /// Defaults to `GITHUB_TOKEN` environment variable. Using a token increases
    /// the GitHub API rate limit from 60 to 5000 requests per hour.
    #[arg(long, env = "GITHUB_TOKEN")]
    pub github_token: Option<String>,

    /// Don't commit changes, just update files.
    ///
    /// When this flag is set, the version will be updated in Cargo.toml but
    /// no git commit will be created. Useful for manual review or when
    /// committing separately.
    #[arg(long)]
    pub no_commit: bool,
}
