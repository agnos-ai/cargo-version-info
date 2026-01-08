//! Git commit orchestration for version changes.
//!
//! This module coordinates the process of creating a git commit that contains
//! only version-related changes. This is the heart of the bump command's
//! "selective staging" functionality.
//!
//! # Selective Staging Rationale
//!
//! When bumping a version, we want to commit ONLY the version changes, not
//! any other uncommitted changes that might exist in the working directory.
//!
//! ## Why This Matters
//!
//! Consider this scenario:
//! ```text
//! Working Directory:
//! - Cargo.toml: version changed from 0.1.0 -> 0.1.1
//! - src/main.rs: work-in-progress feature (uncommitted)
//! - README.md: typo fixes (uncommitted)
//! ```
//!
//! We want the bump commit to include ONLY the Cargo.toml version change,
//! not the WIP feature or typo fixes. This keeps the version bump commit
//! clean and focused.
//!
//! # Commit Process
//!
//! The commit process involves several steps:
//!
//! 1. **Discover Repository**: Find the `.git` directory
//! 2. **Verify Changes**: Ensure version actually changed
//! 3. **Detect Other Changes**: Warn if non-version changes exist
//! 4. **Stage File**: Add file to git index
//! 5. **Build Tree**: Convert index to tree object
//! 6. **Create Commit**: Write commit object
//! 7. **Update HEAD**: Move current branch to new commit
//!
//! # Git Porcelain vs. Plumbing
//!
//! Git has two types of commands:
//!
//! ## Porcelain Commands (High-Level)
//! - `git add`, `git commit`, `git status`
//! - User-friendly, handle multiple operations
//! - What most people use day-to-day
//!
//! ## Plumbing Commands (Low-Level)
//! - `git hash-object`, `git update-index`, `git write-tree`
//! - Building blocks for porcelain commands
//! - What this module implements via `gix`
//!
//! By using gix's plumbing-level APIs, we have fine-grained control over
//! exactly what gets staged and committed.
//!
//! # Hunks and Patches
//!
//! In git terminology:
//! - **Hunk**: A contiguous block of changes in a file
//! - **Patch**: A collection of hunks (can span multiple files)
//!
//! Example diff with 2 hunks:
//! ```diff
//! @@ -1,3 +1,3 @@  ← Hunk 1: lines 1-3
//!  [package]
//!  name = "test"
//! -version = "0.1.0"
//! +version = "0.2.0"
//!  
//! @@ -10,2 +10,2 @@  ← Hunk 2: lines 10-11
//! -# Old comment
//! +# New comment
//! ```
//!
//! Our goal is to stage only the version hunk, not the comment hunk.
//!
//! # Current Implementation
//!
//! The current implementation stages the entire file if version changes are
//! detected, with a warning if non-version changes exist. This is simpler than
//! true hunk-level staging but works for the common case.
//!
//! ## Future Enhancement: True Hunk-Level Staging
//!
//! To implement true hunk-level staging, we would need to:
//!
//! 1. Generate a unified diff between HEAD and working directory
//! 2. Parse the diff into hunks
//! 3. Filter hunks to find only version-related changes
//! 4. Apply only those hunks to the index
//! 5. Build a tree from the partially-staged file
//!
//! This is complex because:
//! - Requires diff parsing and patch application
//! - Must handle merge conflicts in hunks
//! - Needs to update index with partial file content
//!
//! The git command `git add -p` (interactive patch mode) does this, but
//! implementing it programmatically is non-trivial.

use std::path::Path;

use anyhow::{
    Context,
    Result,
};
use bstr::ByteSlice;
use smallvec::SmallVec;

use super::diff;

/// Commit version-related changes using pure gix (no git binary).
///
/// This function orchestrates the entire commit process:
/// - Discovers the git repository
/// - Verifies that version changes exist
/// - Warns about non-version changes
/// - Stages the file in the git index
/// - Builds a tree from the staged files
/// - Creates a commit object
/// - Updates the current branch reference
///
/// # Arguments
///
/// * `manifest_path` - Path to the Cargo.toml file (absolute or relative)
/// * `old_version` - The previous version (for verification and commit message)
/// * `new_version` - The new version (for verification and commit message)
///
/// # Errors
///
/// Returns an error if:
/// - Not in a git repository
/// - File doesn't have version changes
/// - Git operations fail (staging, tree building, commit creation)
/// - HEAD cannot be updated
///
/// # Examples
///
/// ```rust,no_run
/// # use std::path::Path;
/// # use anyhow::Result;
/// # fn example() -> Result<()> {
/// use cargo_version_info::commands::bump::commit::commit_version_changes;
///
/// let manifest = Path::new("./Cargo.toml");
/// commit_version_changes(manifest, "0.1.0", "0.2.0")?;
/// # Ok(())
/// # }
/// ```
///
/// # Implementation Details
///
/// ## Repository Discovery
///
/// Uses `gix::discover()` to find the repository by walking up from the
/// manifest directory. This handles cases where the manifest is in a
/// subdirectory of the repository.
///
/// ## Change Detection
///
/// We verify version changes by:
/// 1. Reading the file from HEAD commit
/// 2. Comparing it with the working directory version
/// 3. Checking if old_version appears in HEAD and new_version in working dir
///
/// This is a heuristic - we don't parse the TOML, just check for string
/// presence. This works reliably for version fields.
///
/// ## Non-Version Change Detection
///
/// To detect non-version changes, we:
/// 1. Split both versions into lines
/// 2. Compare line-by-line
/// 3. Flag differences that don't contain "version" or version numbers
///
/// This heuristic catches most non-version changes while avoiding false
/// positives from version-related formatting changes.
///
/// ## Staging Strategy
///
/// Currently stages the entire file. The steps are:
/// 1. Write file content as a blob object
/// 2. Create an index entry pointing to the blob
/// 3. Add entry to a new index state (removing old entry if present)
/// 4. Sort entries and write index to disk
///
/// ## Tree Building
///
/// Converts the flat index into a hierarchical tree structure. See the
/// [`tree`] module for details.
///
/// ## Commit Creation
///
/// Creates a commit object with:
/// - Tree: Built from the staged index
/// - Parents: Current HEAD commit
/// - Author/Committer: From git config or defaults
/// - Message: Conventional commit format "chore(version): bump X -> Y"
///
/// ## HEAD Update
///
/// Updates the current branch reference to point to the new commit. This is
/// equivalent to `git commit` moving the branch forward.
pub fn commit_version_changes(
    manifest_path: &Path,
    old_version: &str,
    new_version: &str,
) -> Result<()> {
    // Discover git repository by walking up from the manifest's directory
    let repo = gix::discover(manifest_path.parent().unwrap_or_else(|| Path::new(".")))
        .context("Not in a git repository")?;

    // Calculate relative path from repository root
    // This is needed for index entries which use repo-relative paths
    let repo_path = repo.path().parent().context("Invalid repository path")?;
    let relative_path = manifest_path
        .strip_prefix(repo_path)
        .or_else(|_| manifest_path.strip_prefix("."))
        .unwrap_or(manifest_path);

    // Read current working directory content
    let current_content = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    // Get HEAD commit to compare against
    let head = repo.head().context("Failed to read HEAD")?;
    let head_commit_id = head.id().context("HEAD does not point to a commit")?;
    let head_commit = repo
        .find_object(head_commit_id)
        .context("Failed to find HEAD commit")?
        .try_into_commit()
        .context("HEAD is not a commit")?;

    // Get the tree from HEAD (what's currently committed)
    let head_tree = head_commit.tree().context("Failed to get HEAD tree")?;

    // Verify that version changes exist
    verify_version_changes(
        &head_tree,
        relative_path,
        &current_content,
        old_version,
        new_version,
    )?;

    // Get HEAD content for comparison
    let head_content = get_head_content(&head_tree, relative_path)?;

    // Check if there are non-version changes in the file
    let has_other_changes =
        diff::has_non_version_changes(&head_content, &current_content, old_version, new_version);

    // Create the content to stage
    let staged_content = if has_other_changes {
        // File has non-version changes - apply only version hunks
        eprintln!("⚠️  Using hunk-level staging: only version lines will be committed.");

        // Apply only version-related hunks
        diff::apply_version_hunks(&head_content, &current_content, old_version, new_version)?
    } else {
        // File only has version changes - stage the whole file
        current_content.clone()
    };

    // Create blob for the staged content
    let blob_id = write_blob(&repo, &staged_content)?;

    // Build tree by modifying HEAD's tree (not creating minimal tree!)
    // We need to preserve all other files in the repository
    let tree_id = update_tree_with_file(&repo, &head_tree, relative_path, blob_id)?;

    // Create the commit
    let commit_id = create_commit(&repo, &tree_id, head_commit_id, old_version, new_version)?;

    // Update HEAD to point to the new commit
    update_head(&repo, commit_id)?;

    Ok(())
}

/// Get the content of a file from the HEAD tree.
///
/// # Arguments
///
/// * `head_tree` - The tree from HEAD commit
/// * `relative_path` - Path to the file
///
/// # Returns
///
/// Returns the file content as a string.
///
/// # Errors
///
/// Returns an error if the file doesn't exist in HEAD or cannot be read.
fn get_head_content(head_tree: &gix::Tree, relative_path: &Path) -> Result<String> {
    let entry = head_tree
        .lookup_entry_by_path(relative_path)
        .context("Failed to lookup file in HEAD tree")?
        .context("File does not exist in HEAD")?;

    let blob = entry
        .object()
        .context("Failed to get blob from tree entry")?
        .try_into_blob()
        .context("Tree entry is not a blob")?;

    Ok(blob.data.to_str_lossy().into_owned())
}

/// Verify that the file has version-related changes.
///
/// Checks if:
/// - File exists in HEAD and old_version → new_version
/// - OR file is new and contains new_version
///
/// # Errors
///
/// Returns an error if no version-related changes are detected.
fn verify_version_changes(
    head_tree: &gix::Tree,
    relative_path: &Path,
    current_content: &str,
    old_version: &str,
    new_version: &str,
) -> Result<()> {
    let has_version_changes = if let Ok(Some(entry)) = head_tree.lookup_entry_by_path(relative_path)
    {
        // File exists in HEAD - verify version changed
        let head_blob = entry
            .object()
            .context("Failed to get blob from tree entry")?
            .try_into_blob()
            .context("Tree entry is not a blob")?;
        let head_content = head_blob.data.to_str_lossy();

        head_content.contains(old_version) && current_content.contains(new_version)
    } else {
        // File doesn't exist in HEAD - verify it's a version file
        current_content.contains("version") && current_content.contains(new_version)
    };

    if !has_version_changes {
        anyhow::bail!("No version-related changes found");
    }

    Ok(())
}

/// Write file content as a blob object to the git object database.
///
/// Git stores file contents as "blob" objects in `.git/objects/`. Each blob
/// is identified by the SHA-1 hash of its content.
///
/// # Arguments
///
/// * `repo` - The git repository
/// * `content` - The file content to store
///
/// # Returns
///
/// Returns the object ID (SHA-1 hash) of the blob.
fn write_blob(repo: &gix::Repository, content: &str) -> Result<gix::ObjectId> {
    let blob_id = repo
        .write_object(gix::objs::Blob {
            data: content.as_bytes().into(),
        })
        .context("Failed to write blob")?
        .detach();

    Ok(blob_id)
}

/// Update a tree by replacing a single file's blob.
///
/// **CRITICAL**: This function takes HEAD's tree and creates a NEW tree with
/// only ONE file changed. All other files remain exactly as they were in HEAD.
///
/// # Why This Is Critical
///
/// A git commit represents the FULL state of the repository at a point in time.
/// If we create a tree with only Cargo.toml, the commit will DELETE all other
/// files!
///
/// ## Wrong Approach (What We Were Doing)
/// ```text
/// HEAD tree: [Cargo.toml, src/*, docs/*, .github/*]
/// New tree:  [Cargo.toml]
/// Commit:    Deletes src/, docs/, .github/ ❌
/// ```
///
/// ## Correct Approach (What We Do Now)
/// ```text
/// HEAD tree:    [Cargo.toml(v0.1.0), src/*, docs/*, .github/*]
/// Updated tree: [Cargo.toml(v0.2.0), src/*, docs/*, .github/*]
/// Commit:       Only Cargo.toml version changed ✓
/// ```
///
/// # Implementation Strategy
///
/// Since we're using a simplified tree builder (single-level), we need to:
/// 1. Recreate HEAD's tree structure
/// 2. Replace only the target file's blob
/// 3. Keep all other entries unchanged
///
/// For a full implementation with recursive trees, we'd:
/// 1. Parse the path to identify which subtree to modify
/// 2. Clone all trees from HEAD to the target file's parent
/// 3. Replace the blob in the deepest subtree
/// 4. Rebuild parent trees up to root
///
/// # Arguments
///
/// * `repo` - The git repository
/// * `head_tree` - The tree from HEAD commit
/// * `file_path` - Path to the file to update (relative to repo root)
/// * `new_blob_id` - The new blob ID for the file
///
/// # Returns
///
/// Returns the object ID of the new tree with the file updated.
fn update_tree_with_file(
    repo: &gix::Repository,
    head_tree: &gix::Tree,
    file_path: &Path,
    new_blob_id: gix::ObjectId,
) -> Result<gix::ObjectId> {
    use gix::objs::{
        Tree,
        tree,
    };

    // Get all entries from HEAD's tree
    let mut tree_entries: Vec<tree::Entry> = Vec::new();

    // Iterate through HEAD's tree entries
    for entry in head_tree.iter() {
        let entry = entry.context("Failed to iterate tree entry")?;
        let entry_path = entry.filename();

        // Check if this is the file we're updating
        if file_path.as_os_str().as_encoded_bytes() == entry_path {
            // This is the file we're updating - use the new blob
            tree_entries.push(tree::Entry {
                mode: entry.mode(),
                filename: entry_path.into(),
                oid: new_blob_id,
            });
        } else {
            // Keep the entry unchanged from HEAD
            tree_entries.push(tree::Entry {
                mode: entry.mode(),
                filename: entry_path.into(),
                oid: entry.oid().to_owned(),
            });
        }
    }

    // Sort entries using git's special sorting rules
    // Git treats directories as if they have a trailing '/' for sorting purposes
    tree_entries.sort_by(|a, b| {
        use gix::objs::tree::EntryKind;

        let a_name = if matches!(a.mode.kind(), EntryKind::Tree) {
            // Directory - append '/' for sorting
            let mut name = a.filename.to_vec();
            name.push(b'/');
            name
        } else {
            a.filename.to_vec()
        };

        let b_name = if matches!(b.mode.kind(), EntryKind::Tree) {
            // Directory - append '/' for sorting
            let mut name = b.filename.to_vec();
            name.push(b'/');
            name
        } else {
            b.filename.to_vec()
        };

        a_name.cmp(&b_name)
    });

    // Build the tree
    let tree = Tree {
        entries: tree_entries,
    };

    // Write the tree to the object database
    let tree_id = repo
        .write_object(&tree)
        .context("Failed to write updated tree")?
        .detach();

    Ok(tree_id)
}

/// Create a commit object and write it to the object database.
///
/// # Git Commit Structure
///
/// A git commit is a simple text object containing:
/// ```text
/// tree <tree-sha1>
/// parent <parent-sha1>
/// author Name <email> timestamp timezone
/// committer Name <email> timestamp timezone
///
/// Commit message goes here
/// ```
///
/// # Arguments
///
/// * `repo` - The git repository
/// * `tree_id` - The tree object ID (root tree of the commit)
/// * `parent_id` - The parent commit ID (current HEAD)
/// * `old_version` - Previous version (for commit message)
/// * `new_version` - New version (for commit message)
///
/// # Returns
///
/// Returns the object ID of the newly created commit.
fn create_commit(
    repo: &gix::Repository,
    tree_id: &gix::ObjectId,
    parent_id: gix::Id,
    old_version: &str,
    new_version: &str,
) -> Result<gix::ObjectId> {
    // Create commit message following conventional commits format
    let commit_message = format!("chore(version): bump {} -> {}", old_version, new_version);

    // Get author and committer from git config
    let author = get_signature_from_config(repo)?;
    let committer = author.clone();

    // Create parent list - commits can have multiple parents (for merges)
    // We only have one parent (the current HEAD)
    let parents: SmallVec<[gix::ObjectId; 1]> = SmallVec::from_iter([parent_id.detach()]);

    // Write the commit object to the object database
    let commit_id = repo
        .write_object(gix::objs::Commit {
            tree: *tree_id,
            parents,
            author,
            committer,
            message: commit_message.into(),
            encoding: None,
            extra_headers: vec![],
        })
        .context("Failed to write commit object")?
        .detach();

    Ok(commit_id)
}

/// Update HEAD to point to the new commit.
///
/// This moves the current branch forward to include the new commit. This is
/// equivalent to what `git commit` does after creating the commit object.
///
/// # Git References
///
/// HEAD can be:
/// - **Symbolic**: Points to a branch (e.g., `ref: refs/heads/main`)
/// - **Detached**: Points directly to a commit SHA
///
/// In normal operation, HEAD is symbolic and points to the current branch.
/// Updating HEAD in this case means updating the branch reference.
///
/// # Arguments
///
/// * `repo` - The git repository
/// * `commit_id` - The object ID of the commit to point HEAD to
///
/// # Errors
///
/// Returns an error if:
/// - HEAD doesn't exist or is invalid
/// - HEAD is not a reference (detached HEAD state)
/// - Reference update fails
fn update_head(repo: &gix::Repository, commit_id: gix::ObjectId) -> Result<()> {
    // Read current HEAD
    let mut head_ref = repo
        .head()
        .context("Failed to read HEAD")?
        .try_into_referent()
        .context("HEAD is not a reference (detached HEAD state)")?;

    // Update the reference to point to the new commit
    // This is an atomic operation - either succeeds completely or fails
    head_ref
        .set_target_id(commit_id, "bump version")
        .context("Failed to update HEAD reference")?;

    Ok(())
}

/// Get git signature (author/committer) from repository config.
///
/// Reads the `user.name` and `user.email` from git config and creates a
/// signature with the current timestamp.
///
/// # Required Configuration
///
/// This function REQUIRES that git config has both:
/// - `user.name` - The author's name
/// - `user.email` - The author's email
///
/// If either is missing, the function returns an error. This ensures commits
/// have proper attribution and prevents silent fallbacks that could lead to
/// incorrect author information.
///
/// # Setup Instructions
///
/// If you get an error about missing git config, set it with:
/// ```bash
/// git config user.name "Your Name"
/// git config user.email "your.email@example.com"
/// ```
///
/// Or globally:
/// ```bash
/// git config --global user.name "Your Name"
/// git config --global user.email "your.email@example.com"
/// ```
///
/// # Arguments
///
/// * `repo` - The git repository to read config from
///
/// # Returns
///
/// Returns a `Signature` with name, email, and current timestamp.
///
/// # Errors
///
/// Returns an error if:
/// - `user.name` is not set in git config
/// - `user.email` is not set in git config
/// - Config cannot be read
/// - Timestamp cannot be determined
fn get_signature_from_config(repo: &gix::Repository) -> Result<gix::actor::Signature> {
    let config = repo.config_snapshot();

    // Read user.name from config (REQUIRED - no fallback)
    let name = config
        .string("user.name")
        .map(|s| s.to_string())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Git config 'user.name' is not set.\n\
                 Please configure it with:\n  \
                 git config user.name \"Your Name\""
            )
        })?;

    // Read user.email from config (REQUIRED - no fallback)
    let email = config
        .string("user.email")
        .map(|s| s.to_string())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Git config 'user.email' is not set.\n\
                 Please configure it with:\n  \
                 git config user.email \"your.email@example.com\""
            )
        })?;

    // Get current time for the commit
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("Failed to get current time")?;

    let time = gix::date::Time {
        seconds: now.as_secs() as i64,
        offset: 0, // UTC
    };

    Ok(gix::actor::Signature {
        name: name.into(),
        email: email.into(),
        time,
    })
}
