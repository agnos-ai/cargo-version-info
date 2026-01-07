//! Generate a complete release page combining badges, PR log, and changelog.
//!
//! This command combines multiple outputs into a single release page document
//! that can be used for GitHub releases or other documentation.
//!
//! # Examples
//!
//! ```bash
//! # Generate complete release page
//! cargo version-info release-page
//!
//! # Generate since specific tag
//! cargo version-info release-page --since-tag v0.1.0
//!
//! # Skip network requests for badges
//! cargo version-info release-page --no-network
//!
//! # Output to file
//! cargo version-info release-page --output RELEASE.md
//! ```

use std::io::Write;

use anyhow::{
    Context,
    Result,
};
use clap::Parser;

/// Arguments for the `release-page` command.
#[derive(Parser, Debug)]
pub struct ReleasePageArgs {
    /// Tag to compare from (default: latest tag).
    #[arg(long)]
    pub since_tag: Option<String>,

    /// Generate changelog for a commit range (e.g., v0.1.0..v0.2.0).
    #[arg(long)]
    pub range: Option<String>,

    /// Output file path (default: stdout).
    #[arg(short, long)]
    pub output: Option<String>,

    /// Skip network requests and use heuristics for badges.
    #[arg(long)]
    pub no_network: bool,

    /// GitHub repository owner (for linking commits/PRs).
    #[arg(long)]
    pub owner: Option<String>,

    /// GitHub repository name (for linking commits/PRs).
    #[arg(long)]
    pub repo: Option<String>,
}

/// Generate a complete release page.
pub fn release_page(args: ReleasePageArgs) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
    rt.block_on(release_page_async(args))
}

/// Async entry point for release page generation.
async fn release_page_async(args: ReleasePageArgs) -> Result<()> {
    // Create logger - status messages go to stderr, release page to stdout
    let mut logger = cargo_plugin_utils::logger::Logger::new();

    logger.status("Generating", "release page");

    // Find the package
    let package = super::badge::find_package().await?;

    // Prepare output buffer
    let mut output = Vec::new();

    // Section 1: Badges
    logger.status("Generating", "badges");
    writeln!(&mut output, "# {} v{}\n", package.name, package.version)?;
    super::badge::badge_all(&mut output, &package, args.no_network).await?;
    writeln!(&mut output)?;

    // Section 2: PR Log
    logger.status("Generating", "PR log");
    writeln!(&mut output, "## Pull Requests\n")?;
    if let Err(e) = generate_pr_log(&mut output, &args).await {
        writeln!(&mut output, "_PR log generation failed: {}_\n", e)?;
        logger.warning("Warning", &format!("PR log generation failed: {}", e));
    }
    writeln!(&mut output)?;

    // Section 3: Changelog
    logger.status("Generating", "changelog");
    writeln!(&mut output, "## Changelog\n")?;
    generate_changelog(&mut output, &args)?;

    logger.finish();

    // Write output to file or stdout
    if let Some(output_path) = args.output {
        std::fs::write(&output_path, output)
            .with_context(|| format!("Failed to write release page to {}", output_path))?;
        println!("Release page written to {}", output_path);
    } else {
        std::io::stdout().write_all(&output)?;
    }

    Ok(())
}

/// Generate PR log section (stub for now).
async fn generate_pr_log(_writer: &mut dyn Write, args: &ReleasePageArgs) -> Result<()> {
    // Build arguments for pr_log command
    let pr_log_args = crate::commands::PrLogArgs {
        since_tag: args.since_tag.clone(),
        output: None, // We handle output ourselves
        owner: args.owner.clone(),
        repo: args.repo.clone(),
    };

    // Call pr_log - currently returns an error as it's not implemented
    crate::commands::pr_log(pr_log_args)?;

    Ok(())
}

/// Generate changelog section.
fn generate_changelog(writer: &mut dyn Write, args: &ReleasePageArgs) -> Result<()> {
    // Build arguments for changelog command
    let changelog_args = crate::commands::ChangelogArgs {
        at: args.since_tag.clone(),
        range: args.range.clone(),
        output: None, // We handle output ourselves
        owner: args.owner.clone(),
        repo: args.repo.clone(),
    };

    // Generate changelog to our writer
    crate::commands::changelog::generate_changelog_to_writer(writer, changelog_args)?;

    Ok(())
}
