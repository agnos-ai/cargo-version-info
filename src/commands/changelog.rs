//! Generate changelog from conventional commits.
//!
//! This command generates a changelog from git commits using conventional
//! commit format, organized by scope for better readability.
//!
//! # Examples
//!
//! ```bash
//! # Generate changelog since last tag
//! cargo version-info changelog
//!
//! # Generate changelog for specific tag
//! cargo version-info changelog --at v0.1.0
//!
//! # Generate changelog for commit range
//! cargo version-info changelog --range v0.1.0..v0.2.0
//!
//! # Output to file
//! cargo version-info changelog --output CHANGELOG.md
//! ```

use std::collections::HashMap;

use anyhow::{
    Context,
    Result,
};
use bstr::{
    BString,
    ByteSlice,
};
use cargo_plugin_utils::common::get_owner_repo;
use clap::Parser;
use regex::Regex;

/// Arguments for the `changelog` command.
#[derive(Parser, Debug)]
pub struct ChangelogArgs {
    /// Generate changelog for a specific git tag.
    #[arg(long)]
    pub at: Option<String>,

    /// Generate changelog for a commit range (e.g., v0.1.0..v0.2.0).
    #[arg(long)]
    pub range: Option<String>,

    /// Output file path (default: stdout).
    #[arg(short, long)]
    pub output: Option<String>,

    /// GitHub repository owner (for linking commits/PRs).
    #[arg(long)]
    pub owner: Option<String>,

    /// GitHub repository name (for linking commits/PRs).
    #[arg(long)]
    pub repo: Option<String>,
}

/// Commit information parsed from git log.
#[derive(Debug, Clone)]
struct Commit {
    sha: String,
    short_sha: String,
    commit_type: String,
    scope: Option<String>,
    breaking: bool,
    subject: String,
    body: Option<String>,
}

/// Parse a conventional commit message.
fn parse_conventional_commit(message: &str) -> Option<Commit> {
    // Pattern: type(scope): subject
    // or: type!: subject (breaking change)
    // or: type(scope)!: subject (breaking change with scope)
    let re = Regex::new(
        r"^(?P<type>[a-z]+)(?:\((?P<scope>[^)]+)\))?(?P<breaking>!)?:\s*(?P<subject>.+)$",
    )
    .ok()?;

    let first_line = message.lines().next()?;
    let caps = re.captures(first_line)?;

    let commit_type = caps.name("type")?.as_str().to_string();
    let scope = caps.name("scope").map(|m| m.as_str().to_string());
    let breaking = caps.name("breaking").is_some();
    let subject = caps.name("subject")?.as_str().to_string();

    // Extract body (everything after first line, skipping blank line)
    let body = message
        .lines()
        .skip(1)
        .skip_while(|l| l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let body = if body.is_empty() { None } else { Some(body) };

    // Extract SHA from message if available, otherwise use placeholder
    // For now, we'll get SHA from git commit object
    Some(Commit {
        sha: String::new(),       // Will be filled in later
        short_sha: String::new(), // Will be filled in later
        commit_type,
        scope,
        breaking,
        subject,
        body,
    })
}

/// Get commit type display title.
fn commit_type_title(commit_type: &str) -> &str {
    match commit_type {
        "feat" => "Features",
        "fix" => "Bug Fixes",
        "docs" => "Documentation",
        "style" => "Styling",
        "refactor" => "Refactoring",
        "perf" => "Performance",
        "test" => "Tests",
        "build" => "Build",
        "ci" => "CI/CD",
        "chore" => "Chores",
        "revert" => "Reverts",
        _ => "Other Changes",
    }
}

/// Check if commit type should be included in changelog.
fn include_in_changelog(commit_type: &str) -> bool {
    matches!(
        commit_type,
        "feat" | "fix" | "docs" | "refactor" | "perf" | "revert"
    )
}

/// Format a single commit as a changelog entry.
fn format_commit_entry(commit: &Commit, owner: &str, repo: &str) -> String {
    let breaking_marker = if commit.breaking { " **BREAKING**" } else { "" };
    let commit_link = format!(
        "[{}](https://github.com/{}/{}/commit/{})",
        commit.short_sha, owner, repo, commit.sha
    );
    let mut output = format!("- {}{}: {}\n", commit_link, breaking_marker, commit.subject);

    // Add body if present
    if let Some(body) = &commit.body {
        let body_lines: Vec<&str> = body.lines().collect();
        if !body_lines.is_empty() {
            for line in body_lines {
                output.push_str(&format!("  {}\n", line));
            }
        }
    }

    output
}

/// Generate changelog to a writer.
pub fn generate_changelog_to_writer(
    writer: &mut dyn std::io::Write,
    args: ChangelogArgs,
) -> Result<()> {
    let (owner, repo) = get_owner_repo(args.owner.clone(), args.repo.clone())?;

    // Discover git repository
    let git_repo = gix::discover(".").context("Failed to discover git repository")?;

    // Determine start commit for range
    let (start_oid, end_oid) = if let Some(range) = &args.range {
        // Parse range like "v0.1.0..v0.2.0" or "v0.1.0..HEAD"
        let parts: Vec<&str> = range.split("..").collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid range format. Expected: <start>..<end>");
        }
        let start_ref = parts[0].trim();
        let end_ref = parts[1].trim();

        // Resolve references using rev_parse
        let start_bstr: BString = start_ref.into();
        let start_spec = git_repo
            .rev_parse(start_bstr.as_bstr())
            .with_context(|| format!("Failed to resolve start reference: {}", start_ref))?;
        let start_oid = start_spec
            .single()
            .context("Start reference resolved to multiple objects")?;

        let end_bstr: BString = end_ref.into();
        let end_spec = git_repo
            .rev_parse(end_bstr.as_bstr())
            .with_context(|| format!("Failed to resolve end reference: {}", end_ref))?;
        let end_oid = end_spec
            .single()
            .context("End reference resolved to multiple objects")?;

        (Some(start_oid), end_oid)
    } else if let Some(tag) = &args.at {
        // Generate changelog for commits up to this tag
        let tag_bstr: BString = tag.as_str().into();
        let tag_spec = git_repo
            .rev_parse(tag_bstr.as_bstr())
            .with_context(|| format!("Failed to resolve tag: {}", tag))?;
        let tag_oid = tag_spec
            .single()
            .context("Tag resolved to multiple objects")?;

        // Get HEAD for end
        let head = git_repo.head().context("Failed to read HEAD")?;
        let head_oid = head.id().context("HEAD does not point to a commit")?;

        (Some(tag_oid), head_oid)
    } else {
        // Default: since last tag
        // Find a tag by iterating through references
        let mut latest_tag_oid: Option<gix::Id> = None;

        if let Ok(refs) = git_repo.references() {
            for reference in refs.all()? {
                let Ok(reference) = reference else {
                    continue;
                };
                let name_str = reference.name().as_bstr().to_string();
                let Some(name) = name_str.strip_prefix("refs/tags/") else {
                    continue;
                };
                let name_bstr: BString = name.to_string().into();
                let Ok(spec) = git_repo.rev_parse(name_bstr.as_bstr()) else {
                    continue;
                };
                let Some(oid) = spec.single() else {
                    continue;
                };
                // Store the first tag we find
                latest_tag_oid = Some(oid);
                break; // Found a tag, no need to continue
            }
        }

        // Get HEAD for end
        let head = git_repo.head().context("Failed to read HEAD")?;
        let head_oid = head.id().context("HEAD does not point to a commit")?;

        (latest_tag_oid, head_oid)
    };

    // Walk commits using gix rev_walk
    let walk = git_repo.rev_walk([end_oid]);
    let walk_iter = walk.all()?;

    // If we have a start point, we need to stop at it
    // For now, we'll walk all commits and filter by checking if we've reached
    // start_oid
    let mut commits: Vec<Commit> = Vec::new();

    for info_result in walk_iter {
        let info = info_result?;
        let oid = info.id();

        // Stop if we've reached the start commit
        if let Some(start) = start_oid
            && oid == start
        {
            break;
        }

        // Get commit object
        let commit_obj = git_repo
            .find_object(oid)
            .context("Failed to find commit object")?;
        let commit = commit_obj
            .try_into_commit()
            .context("Object is not a commit")?;

        // Get commit message
        let message_raw = commit
            .message_raw()
            .context("Failed to read raw commit message")?;
        // Convert message to UTF-8, tolerating invalid bytes
        let message_str = String::from_utf8_lossy(message_raw.as_ref()).into_owned();

        // Parse conventional commit format
        if let Some(mut parsed) = parse_conventional_commit(&message_str) {
            // Only include commits that should be in changelog
            if include_in_changelog(&parsed.commit_type) {
                let short_sha = oid.shorten().context("Failed to shorten commit SHA")?;
                parsed.sha = oid.to_string();
                parsed.short_sha = short_sha.to_string();

                // Extract body from message (everything after first line)
                let body_lines: Vec<&str> = message_str.lines().skip(1).collect();
                let body_text: String = body_lines.join("\n").trim().to_string();
                parsed.body = if body_text.is_empty() {
                    None
                } else {
                    Some(body_text)
                };

                commits.push(parsed);
            }
        }
    }

    // Group commits by type, then by scope
    let mut by_type: HashMap<String, HashMap<Option<String>, Vec<Commit>>> = HashMap::new();

    for commit in commits {
        by_type
            .entry(commit.commit_type.clone())
            .or_default()
            .entry(commit.scope.clone())
            .or_default()
            .push(commit);
    }

    // Generate markdown
    let mut output = String::new();

    // Header
    if let Some(tag) = &args.at {
        output.push_str(&format!("# Changelog - {}\n\n", tag));
    } else {
        output.push_str("# Changelog\n\n");
    }

    // Order commit types
    let type_order = [
        "feat", "fix", "perf", "refactor", "docs", "revert", "build", "ci", "test", "style",
        "chore",
    ];

    for commit_type in type_order {
        if let Some(by_scope) = by_type.get(commit_type) {
            output.push_str(&format!("## {}\n\n", commit_type_title(commit_type)));

            // Group by scope
            let mut scopes: Vec<_> = by_scope.keys().collect();
            scopes.sort(); // None (no scope) will come first

            for scope in scopes {
                let scope_commits = &by_scope[scope];

                // Scope header if present
                if let Some(scope_name) = scope {
                    output.push_str(&format!("### {}\n\n", scope_name));
                }

                // List commits
                for commit in scope_commits {
                    output.push_str(&format_commit_entry(commit, &owner, &repo));
                }

                output.push('\n');
            }
        }
    }

    if output.trim().ends_with("# Changelog\n\n") {
        output.push_str("No changes found.\n");
    }

    // Write to the provided writer
    write!(writer, "{}", output)?;

    Ok(())
}

/// Generate changelog from git commits.
pub fn changelog(args: ChangelogArgs) -> Result<()> {
    let output_path = args.output.clone();

    if let Some(ref path) = output_path {
        // Write to file
        let mut file = std::fs::File::create(path)
            .with_context(|| format!("Failed to create file {}", path))?;
        generate_changelog_to_writer(&mut file, args)?;
    } else {
        // Write to stdout
        let mut stdout = std::io::stdout();
        generate_changelog_to_writer(&mut stdout, args)?;
    }

    Ok(())
}
