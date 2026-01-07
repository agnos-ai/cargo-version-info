//! GitHub API integration for version queries.

use std::env;

use anyhow::{
    Context,
    Result,
};

use crate::version::{
    format_version,
    increment_patch,
    parse_version,
};

/// Get the latest published release version from GitHub.
///
/// Uses the GitHub API via octocrab. Works for public repos without a token
/// (with rate limits). For private repos, a token is required (automatically
/// detected from GITHUB_TOKEN env var if not provided).
#[allow(clippy::disallowed_methods)] // CLI tool needs direct env access
pub async fn get_latest_release_version(
    owner: &str,
    repo: &str,
    github_token: Option<&str>,
) -> Result<Option<String>> {
    // Auto-detect token from environment if not provided
    let env_token = env::var("GITHUB_TOKEN").ok();
    let token = github_token.or(env_token.as_deref());

    // Try with token first (required for private repos, better rate limits for
    // public)
    let result = if let Some(token) = token {
        get_latest_release_via_api(owner, repo, Some(token)).await
    } else {
        // Try without token (public repos only)
        get_latest_release_via_api(owner, repo, None).await
    };

    match result {
        Ok(version) => Ok(Some(version)),
        Err(e) => {
            let error_msg = e.to_string();
            // If no releases found, return None instead of error
            if error_msg.contains("No releases found") {
                Ok(None)
            } else if error_msg.contains("404") || error_msg.contains("Not Found") {
                // 404 could mean private repo without auth or repo doesn't exist
                if token.is_none() {
                    Err(anyhow::anyhow!(
                        "Repository not found or is private. For private repositories, \
                         set GITHUB_TOKEN environment variable or pass --github-token"
                    )
                    .context(error_msg))
                } else {
                    Err(e)
                }
            } else if error_msg.contains("403") || error_msg.contains("Forbidden") {
                // 403 usually means private repo or rate limit
                Err(anyhow::anyhow!(
                    "Access forbidden. This may be a private repository. \
                     Ensure GITHUB_TOKEN has appropriate permissions."
                )
                .context(error_msg))
            } else {
                Err(e)
            }
        }
    }
}

/// Get latest release via GitHub API.
///
/// Works for public repositories even without a token (with rate limits).
/// If a token is provided, uses it for authentication (higher rate limits).
async fn get_latest_release_via_api(
    owner: &str,
    repo: &str,
    token: Option<&str>,
) -> Result<String> {
    let octocrab = if let Some(token) = token {
        octocrab::OctocrabBuilder::new()
            .personal_token(token.to_string())
            .build()
            .context("Failed to create GitHub API client")?
    } else {
        // For public repos, we can use octocrab without a token
        octocrab::Octocrab::builder()
            .build()
            .context("Failed to create GitHub API client")?
    };

    let releases = octocrab
        .repos(owner, repo)
        .releases()
        .list()
        .per_page(1)
        .send()
        .await
        .context("Failed to query GitHub releases")?;

    let release = releases.items.first().context("No releases found")?;

    let tag_name = release.tag_name.as_str();
    let version = tag_name.strip_prefix('v').unwrap_or(tag_name);
    let version = version.strip_prefix('V').unwrap_or(version);

    Ok(version.to_string())
}

/// Calculate next patch version from latest GitHub release.
pub async fn calculate_next_version(
    owner: &str,
    repo: &str,
    github_token: Option<&str>,
) -> Result<(String, String)> {
    // Get latest release
    let latest_version_str = match get_latest_release_version(owner, repo, github_token).await? {
        Some(v) => v,
        None => {
            // No releases yet, start at 0.0.1
            return Ok(("0.0.0".to_string(), "0.0.1".to_string()));
        }
    };

    let (major, minor, patch) = parse_version(&latest_version_str)
        .with_context(|| format!("Failed to parse latest version: {}", latest_version_str))?;

    let (major, minor, patch) = increment_patch(major, minor, patch);
    let next_version = format_version(major, minor, patch);

    Ok((latest_version_str, next_version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_get_latest_release_via_api() {
        // This test requires network access
        // Only run manually
        if let Ok(Some(version)) = get_latest_release_version("rust-lang", "rust", None).await {
            println!("Latest rust release: {}", version);
        }
    }
}
