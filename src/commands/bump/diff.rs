//! Diff generation and hunk-level filtering.
//!
//! This module implements the "holy grail" of version bumping: staging ONLY
//! the lines that contain version changes, leaving all other changes (even
//! in the same file) unstaged.
//!
//! # The Problem
//!
//! Consider this scenario - you're working on Cargo.toml:
//!
//! ```diff
//! @@ -1,7 +1,8 @@
//!  [package]
//!  name = "my-crate"
//! -version = "0.1.0"
//! +version = "0.2.0"
//!  edition = "2021"
//!  
//!  [dependencies]
//! -serde = "1.0"
//! +serde = { version = "1.0", features = ["derive"] }
//! ```
//!
//! We want to commit ONLY the version line change, not the serde dependency
//! change. This requires hunk-level staging.
//!
//! # Solution: Unified Diff + Hunk Filtering
//!
//! 1. Generate unified diff between HEAD and working directory
//! 2. Parse diff into hunks
//! 3. Filter hunks to find version-related changes
//! 4. Apply only those hunks to create a partially-staged file
//! 5. Write the partially-staged content as a blob
//!
//! # Unified Diff Format
//!
//! A unified diff looks like:
//! ```text
//! --- a/Cargo.toml
//! +++ b/Cargo.toml
//! @@ -3,5 +3,5 @@
//!  name = "my-crate"
//! -version = "0.1.0"
//! +version = "0.2.0"
//!  edition = "2021"
//! ```
//!
//! Key components:
//! - **Hunk header**: `@@ -3,5 +3,5 @@` means "at line 3, remove 5 lines, add 5
//!   lines"
//! - **Context lines**: Start with space, unchanged
//! - **Removed lines**: Start with `-`
//! - **Added lines**: Start with `+`
//!
//! # Hunk Filtering Logic
//!
//! A hunk is "version-related" if:
//! - It contains lines with "version" keyword
//! - It contains the old or new version string
//! - It's within a reasonable distance of other version changes
//!
//! # Implementation Strategy
//!
//! We use the `similar` crate to:
//! - Generate line-by-line diff
//! - Identify change regions (hunks)
//! - Reconstruct file content with selected changes only

use anyhow::Result;
use similar::{
    ChangeTag,
    TextDiff,
};

/// Apply only version-related hunks to create partially-staged content.
///
/// This is the core function that implements selective hunk staging. It:
/// 1. Generates a diff between HEAD and working directory versions
/// 2. Identifies which lines changed
/// 3. Filters to keep only version-related changes
/// 4. Reconstructs the file with only those changes applied
///
/// # Arguments
///
/// * `head_content` - Content of the file in HEAD commit
/// * `working_content` - Content of the file in working directory
/// * `old_version` - The version string being replaced
/// * `new_version` - The version string being added
///
/// # Returns
///
/// Returns the partially-staged content (HEAD + only version changes).
///
/// # Examples
///
/// ```rust
/// # use cargo_version_info::commands::bump::diff::apply_version_hunks;
/// let head = "[package]\nname = \"test\"\nversion = \"0.1.0\"\ndesc = \"old\"";
/// let working = "[package]\nname = \"test\"\nversion = \"0.2.0\"\ndesc = \"new\"";
///
/// let staged = apply_version_hunks(head, working, "0.1.0", "0.2.0").unwrap();
///
/// // staged contains only the version change, not the desc change
/// assert!(staged.contains("version = \"0.2.0\""));
/// assert!(staged.contains("desc = \"old\"")); // NOT "new"
/// ```
///
/// # Algorithm
///
/// 1. Generate unified diff using `similar::TextDiff`
/// 2. Iterate through all changes (insertions, deletions, unchanged)
/// 3. For each change, check if it's version-related:
///    - Does the line contain "version"?
///    - Does the line contain old_version or new_version?
/// 4. Build output:
///    - Version-related changes: Use working directory version
///    - Non-version changes: Use HEAD version (ignore working changes)
///    - Unchanged lines: Include as-is
///
/// # Edge Cases
///
/// - **Multiple version fields**: All are updated (package.version,
///   dependencies.*.version)
/// - **Version in comments**: May be incorrectly detected (acceptable
///   trade-off)
/// - **Adjacent changes**: Non-version changes adjacent to version changes are
///   kept separate
pub fn apply_version_hunks(
    head_content: &str,
    working_content: &str,
    old_version: &str,
    new_version: &str,
) -> Result<String> {
    // Generate unified diff between HEAD and working directory
    let diff = TextDiff::from_lines(head_content, working_content);

    let mut result = Vec::new();

    // Iterate through all changes
    for change in diff.iter_all_changes() {
        let line = change.value();

        // Determine if this line is version-related
        let is_version_related =
            line.contains("version") || line.contains(old_version) || line.contains(new_version);

        match change.tag() {
            ChangeTag::Equal => {
                // Unchanged line - always include
                result.push(line);
            }
            ChangeTag::Delete => {
                // Line removed in working directory
                if is_version_related {
                    // This is a version line being removed - apply the change
                    // (skip it) Don't add to result
                } else {
                    // Non-version line removed - keep the original (don't apply change)
                    result.push(line);
                }
            }
            ChangeTag::Insert => {
                // Line added in working directory
                if is_version_related {
                    // This is a version line being added - apply the change (include it)
                    result.push(line);
                } else {
                    // Non-version line added - don't apply the change (skip it)
                    // The line stays not present (remains as in HEAD)
                }
            }
        }
    }

    Ok(result.join(""))
}

/// Check if the file has changes beyond version modifications.
///
/// This is used to determine if we need hunk-level filtering or if we can
/// just stage the whole file.
///
/// # Arguments
///
/// * `head_content` - Content from HEAD
/// * `working_content` - Content from working directory
/// * `old_version` - Old version string
/// * `new_version` - New version string
///
/// # Returns
///
/// Returns `true` if there are non-version changes.
pub fn has_non_version_changes(
    head_content: &str,
    working_content: &str,
    old_version: &str,
    new_version: &str,
) -> bool {
    let diff = TextDiff::from_lines(head_content, working_content);

    // Check if any changes are NOT version-related
    for change in diff.iter_all_changes() {
        if matches!(change.tag(), ChangeTag::Delete | ChangeTag::Insert) {
            let line = change.value();
            let is_version_related = line.contains("version")
                || line.contains(old_version)
                || line.contains(new_version);

            if !is_version_related {
                // Found a non-version change
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_version_hunks_only_version_change() {
        let head = "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
        let working = "[package]\nname = \"test\"\nversion = \"0.2.0\"\nedition = \"2021\"\n";

        let staged = apply_version_hunks(head, working, "0.1.0", "0.2.0").unwrap();

        assert!(staged.contains("version = \"0.2.0\""));
        assert!(!staged.contains("0.1.0"));
    }

    #[test]
    fn test_apply_version_hunks_mixed_changes() {
        let head = "[package]\nname = \"test\"\nversion = \"0.1.0\"\ndescription = \"old desc\"\n";
        let working =
            "[package]\nname = \"test\"\nversion = \"0.2.0\"\ndescription = \"new desc\"\n";

        let staged = apply_version_hunks(head, working, "0.1.0", "0.2.0").unwrap();

        // Should have version change
        assert!(staged.contains("version = \"0.2.0\""));
        // Should NOT have description change - keeps old value
        assert!(staged.contains("description = \"old desc\""));
        assert!(!staged.contains("description = \"new desc\""));
    }

    #[test]
    fn test_has_non_version_changes_true() {
        let head = "[package]\nname = \"test\"\nversion = \"0.1.0\"\n";
        let working = "[package]\nname = \"test-renamed\"\nversion = \"0.2.0\"\n";

        assert!(has_non_version_changes(head, working, "0.1.0", "0.2.0"));
    }

    #[test]
    fn test_has_non_version_changes_false() {
        let head = "[package]\nname = \"test\"\nversion = \"0.1.0\"\n";
        let working = "[package]\nname = \"test\"\nversion = \"0.2.0\"\n";

        assert!(!has_non_version_changes(head, working, "0.1.0", "0.2.0"));
    }

    #[test]
    fn test_apply_version_hunks_multiple_version_fields() {
        let head =
            "[package]\nversion = \"1.0.0\"\n[dependencies]\ncrate-a = { version = \"1.0.0\" }\n";
        let working =
            "[package]\nversion = \"2.0.0\"\n[dependencies]\ncrate-a = { version = \"2.0.0\" }\n";

        let staged = apply_version_hunks(head, working, "1.0.0", "2.0.0").unwrap();

        // Should update both version fields
        assert!(staged.contains("version = \"2.0.0\""));
        assert!(!staged.contains("1.0.0"));
    }
}
