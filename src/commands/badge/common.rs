//! Common utilities for badge generation.

use std::path::PathBuf;

use anyhow::{
    Context,
    Result,
};

/// Heuristically guess if a crate is likely published on crates.io/docs.rs.
///
/// Checks:
/// - `publish` field in Cargo.toml (if false, definitely not published)
/// - Whether any GitHub workflow files contain "cargo publish"
/// - Whether LICENSE file exists (indicates readiness for publication)
pub async fn guess_if_published(package: &cargo_metadata::Package) -> Result<bool> {
    // Check publish field - if Some(vec) and vec is empty, not published
    if let Some(ref publish) = package.publish
        && publish.is_empty()
    {
        return Ok(false);
    }

    // Check if LICENSE is specified in package metadata
    let has_license_in_metadata = package.license.is_some() || package.license_file.is_some();

    // Check if LICENSE file exists (relative to manifest directory)
    let manifest_path = package.manifest_path.as_std_path();
    let manifest_dir = manifest_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let has_license = tokio::fs::metadata(manifest_dir.join("LICENSE"))
        .await
        .is_ok()
        || tokio::fs::metadata(manifest_dir.join("LICENSE-MIT"))
            .await
            .is_ok()
        || tokio::fs::metadata(manifest_dir.join("LICENSE-APACHE"))
            .await
            .is_ok();

    // Check GitHub workflows for "cargo publish" (relative to manifest directory)
    let workflows_dir = manifest_dir.join(".github/workflows");
    let has_publish_in_workflows = if tokio::fs::metadata(&workflows_dir).await.is_err() {
        false
    } else {
        let mut entries = match tokio::fs::read_dir(&workflows_dir).await {
            Ok(entries) => entries,
            Err(_) => return Ok(false),
        };
        let mut found = false;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let ext = path.extension();
            if ext != Some("yml".as_ref()) && ext != Some("yaml".as_ref()) {
                continue;
            }
            if let Ok(content) = tokio::fs::read_to_string(&path).await
                && content.contains("cargo publish")
            {
                found = true;
                break;
            }
        }
        found
    };

    // Best-effort guess: if has license (in metadata or file), or has publish in
    // workflows
    let likely_published = has_license_in_metadata || has_license || has_publish_in_workflows;

    Ok(likely_published)
}

/// Compute cache key for invalidation.
/// Uses git commit hash if available, otherwise falls back to Cargo.toml mtime.
pub async fn compute_cache_key(package: &cargo_metadata::Package) -> Result<String> {
    // Try git commit hash first
    let git_hash = tokio::task::spawn_blocking(|| {
        let repo = match gix::discover(".") {
            Ok(r) => r,
            Err(_) => return None,
        };

        match repo.head_id() {
            Ok(id) => Some(id.to_hex().to_string()),
            Err(_) => None,
        }
    })
    .await
    .context("Failed to spawn blocking task")?;

    if let Some(hash) = git_hash {
        return Ok(hash);
    }

    // Fall back to Cargo.toml modification time
    let manifest_path = package.manifest_path.as_std_path();
    let mtime = tokio::task::spawn_blocking({
        let path = manifest_path.to_path_buf();
        move || {
            std::fs::metadata(&path)
                .ok()
                .and_then(|meta| meta.modified().ok())
                .map(|time| {
                    time.duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        .to_string()
                })
        }
    })
    .await
    .context("Failed to spawn blocking task")?;

    Ok(mtime.unwrap_or_else(|| "unknown".to_string()))
}

/// Get cache file path for badge caches.
pub fn get_badge_cache_path(cache_name: &str) -> Result<PathBuf> {
    let target_dir = if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        PathBuf::from(dir)
    } else {
        // Try to find target directory relative to current dir
        let mut path = std::env::current_dir()?;
        let mut found = None;
        loop {
            let target = path.join("target");
            if target.exists() {
                found = Some(target);
                break;
            }
            if let Some(parent) = path.parent() {
                path = parent.to_path_buf();
            } else {
                break;
            }
        }
        // Fallback to current dir
        found.unwrap_or_else(|| std::env::current_dir().unwrap().join("target"))
    };

    Ok(target_dir.join(format!(".cargo-version-info-{}-cache.json", cache_name)))
}
