//! Version bumping command.
//!
//! This module implements the `cargo version-info bump` subcommand, which
//! intelligently bumps version numbers in Cargo.toml and creates focused
//! git commits containing only the version changes.
//!
//! # Overview
//!
//! The bump command solves a common problem in version management: how to
//! commit version changes without including other uncommitted work. Unlike
//! tools like `cocogitto` (which also manages tags and changelogs), this
//! command focuses solely on version bumping and commit creation.
//!
//! # Architecture
//!
//! The module is split into focused sub-modules:
//!
//! - [`args`] - Command-line argument definitions
//! - [`version_update`] - TOML file manipulation
//! - [`index`] - Git index (staging area) operations
//! - [`tree`] - Git tree building from index
//! - [`commit`] - Commit orchestration and creation
//!
//! # Usage Examples
//!
//! ```bash
//! # Bump patch version (most common)
//! cargo version-info bump --patch
//!
//! # Bump minor version for new features
//! cargo version-info bump --minor
//!
//! # Bump major version for breaking changes
//! cargo version-info bump --major
//!
//! # Set specific version
//! cargo version-info bump --version 2.0.0
//!
//! # Auto-suggest from GitHub releases
//! cargo version-info bump --auto --github-token $TOKEN
//!
//! # Update but don't commit
//! cargo version-info bump --patch --no-commit
//! ```
//!
//! # Workflow
//!
//! 1. **Calculate Target Version**
//!    - From explicit `--version` flag
//!    - From GitHub API (`--auto`)
//!    - From semantic version increment (`--major`, `--minor`, `--patch`)
//!
//! 2. **Update Cargo.toml**
//!    - Parse TOML while preserving formatting
//!    - Update version field
//!    - Write back to disk
//!
//! 3. **Create Commit** (unless `--no-commit`)
//!    - Verify version changes
//!    - Stage only the modified file
//!    - Build git tree from staged files
//!    - Create commit object
//!    - Update HEAD reference
//!
//! # Design Philosophy
//!
//! ## No Tags
//!
//! Unlike `cog bump`, this command does NOT create git tags. Tag creation
//! is left to CI/CD pipelines which can:
//! - Run tests before tagging
//! - Include release metadata
//! - Trigger deployment workflows
//! - Handle tag signing
//!
//! ## Selective Staging
//!
//! The command stages only the version changes, leaving other uncommitted
//! work untouched. This prevents accidental inclusion of WIP code in version
//! bump commits.
//!
//! ## Pure Rust Git Operations
//!
//! All git operations use `gix` (gitoxide) instead of shelling out to the
//! git binary. This provides:
//! - Better type safety
//! - No process spawning overhead
//! - Consistent error handling
//! - Easier testing
//!
//! # Implementation Notes
//!
//! ## Conventional Commits
//!
//! Commit messages follow the conventional commits format:
//! ```text
//! chore: bump version from X.Y.Z to X.Y.Z
//! ```
//!
//! The `chore` type indicates this is a maintenance task, not a feature or fix.
//!
//! ## Workspace Support
//!
//! The command handles both:
//! - Regular crates with `[package] version`
//! - Workspace members with `[workspace.package] version`
//!
//! ## Error Handling
//!
//! All operations use `anyhow::Result` for consistent error handling with
//! context. Errors are bubbled up with descriptive messages about what failed
//! and why.

pub mod args;
pub mod commit;
pub mod diff;
pub mod index;
pub mod tree;
pub mod version_update;

#[cfg(test)]
mod tests;

// Re-export public API
use anyhow::{
    Context,
    Result,
};
pub use args::BumpArgs;
use cargo_plugin_utils::common::{
    find_package,
    get_owner_repo,
};

use crate::github;
use crate::version::{
    format_version,
    increment_major,
    increment_minor,
    increment_patch,
    parse_version,
};

/// Bump the version in Cargo.toml and commit only version-related changes.
///
/// This is the main entry point for the bump command. It orchestrates the
/// entire version bump process from calculation through commit.
///
/// # Process
///
/// 1. **Read Current Version**
///    - Use cargo_metadata to parse Cargo.toml
///    - Extract current version from package metadata
///
/// 2. **Calculate Target Version**
///    - Manual: Use `--version` argument directly
///    - Auto: Query GitHub API for latest release and suggest next
///    - Increment: Parse current version and apply semantic version rules
///
/// 3. **Update Files**
///    - Modify Cargo.toml with new version
///    - Preserve all formatting and comments
///
/// 4. **Create Commit** (unless `--no-commit`)
///    - Stage only the version changes
///    - Build tree from staged index
///    - Create commit object with conventional message
///    - Update HEAD to new commit
///
/// # Arguments
///
/// * `args` - Parsed command-line arguments (see [`BumpArgs`])
///
/// # Returns
///
/// Returns `Ok(())` on success.
///
/// # Errors
///
/// Returns an error if:
/// - Cargo.toml cannot be read or parsed
/// - Current version cannot be determined
/// - Target version calculation fails
/// - File updates fail
/// - Git operations fail (when committing)
/// - Current version equals target version (nothing to bump)
///
/// # Examples
///
/// ```no_run
/// use cargo_version_info::commands::{
///     BumpArgs,
///     bump,
/// };
/// use clap::Parser;
///
/// # fn main() -> anyhow::Result<()> {
/// // Parse command-line arguments
/// let args = BumpArgs::parse_from(&["cargo", "version-info", "bump", "--patch"]);
///
/// // Execute the bump
/// bump(args)?;
/// # Ok(())
/// # }
/// ```
///
/// # Version Calculation
///
/// ## Semantic Versioning
///
/// Versions follow SemVer (MAJOR.MINOR.PATCH):
/// - MAJOR: Breaking changes (resets MINOR and PATCH to 0)
/// - MINOR: New features (resets PATCH to 0)
/// - PATCH: Bug fixes
///
/// ## Auto Mode
///
/// The `--auto` flag queries the GitHub Releases API to find the latest
/// published version and suggests the next appropriate version. This is
/// useful in CI/CD pipelines where you want automated version suggestions.
///
/// # Commit Format
///
/// Commits use the conventional commits format:
/// ```text
/// chore: bump version from 0.1.0 to 0.2.0
/// ```
///
/// This format:
/// - Is machine-parseable for changelog generation
/// - Clearly indicates the type of change
/// - Includes both old and new versions for context
///
/// # No-Commit Mode
///
/// The `--no-commit` flag allows updating the version without creating a
/// commit. This is useful when:
/// - You want to review changes first
/// - You're making multiple related changes
/// - You prefer manual commit control
pub fn bump(args: BumpArgs) -> Result<()> {
    let mut logger = cargo_plugin_utils::logger::Logger::new();

    // Step 1: Get current version from Cargo.toml
    logger.status("Reading", "current version");
    let package = find_package(args.manifest_path.as_deref())?;
    let current_version = package.version.to_string();
    logger.finish();

    // Step 2: Calculate target version based on command args
    logger.status("Calculating", "target version");
    let target_version = calculate_target_version(&args, &current_version)?;
    logger.finish();

    // Step 3: Verify version is changing
    if current_version == target_version {
        anyhow::bail!(
            "Current version ({}) is already the target version. Nothing to bump.",
            current_version
        );
    }

    logger.print_message(&format!(
        "Bumping version: {} -> {}",
        current_version, target_version
    ));

    // Step 4: Update Cargo.toml
    logger.status("Updating", "Cargo.toml");
    let manifest_path = args
        .manifest_path
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("./Cargo.toml"));
    version_update::update_cargo_toml_version(manifest_path, &current_version, &target_version)?;
    logger.finish();

    // Step 5: Commit changes (unless --no-commit)
    if !args.no_commit {
        logger.status("Committing", "version changes");
        commit::commit_version_changes(manifest_path, &current_version, &target_version)?;
        logger.finish();
        logger.print_message(&format!(
            "✓ Committed version bump: {} -> {}",
            current_version, target_version
        ));
    } else {
        logger.print_message(&format!(
            "✓ Updated version to {} (not committed)",
            target_version
        ));
    }

    Ok(())
}

/// Calculate the target version based on command arguments.
///
/// This function implements the version selection logic for all supported
/// modes:
/// - Manual version specification
/// - Automatic suggestion from GitHub
/// - Semantic version increments (major/minor/patch)
///
/// # Arguments
///
/// * `args` - Command-line arguments containing version selection flags
/// * `current_version` - The current version string (e.g., "0.1.0")
///
/// # Returns
///
/// Returns the calculated target version as a string.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub API query fails (in auto mode)
/// - Version parsing fails
/// - Network requests fail
fn calculate_target_version(args: &BumpArgs, current_version: &str) -> Result<String> {
    if let Some(version) = &args.version {
        // Manual version specified
        Ok(version.trim().to_string())
    } else if args.auto {
        // Auto-suggest from GitHub releases
        let (owner, repo) = get_owner_repo(args.owner.clone(), args.repo.clone())?;
        let github_token = args.github_token.as_deref();
        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        let (_latest, next) =
            rt.block_on(github::calculate_next_version(&owner, &repo, github_token))?;
        Ok(next)
    } else {
        // Semantic version increment
        let (major, minor, patch) = parse_version(current_version)?;
        let (new_major, new_minor, new_patch) = if args.major {
            increment_major(major, minor, patch)
        } else if args.minor {
            increment_minor(major, minor, patch)
        } else if args.patch {
            increment_patch(major, minor, patch)
        } else {
            // Default to patch if no flag specified
            increment_patch(major, minor, patch)
        };
        Ok(format_version(new_major, new_minor, new_patch))
    }
}
