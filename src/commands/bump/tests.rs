//! Tests for the bump command.
//!
//! This module contains comprehensive tests for all aspects of the bump
//! command including version calculation, TOML updates, and git integration.

use bstr::ByteSlice;
use tempfile::TempDir;

use super::*;

/// Create a temporary cargo project for testing.
///
/// Creates a minimal valid cargo project with:
/// - Cargo.toml with the specified content
/// - src/ directory
/// - src/lib.rs with minimal content (required by cargo_metadata)
fn create_temp_cargo_project(content: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let manifest_path = dir.path().join("Cargo.toml");
    std::fs::write(&manifest_path, content).unwrap();

    // Create src directory with a minimal lib.rs for cargo metadata to work
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("lib.rs"), "// Test library\n").unwrap();

    dir
}

/// Initialize a git repository in the test directory.
///
/// Uses git commands for test setup (simpler and more reliable than using gix
/// for initialization). The important part is that the bump function itself
/// uses gix, not the test setup.
fn init_test_git_repo(dir: &std::path::Path) {
    std::process::Command::new("git")
        .arg("init")
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "Cargo.toml"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(dir)
        .output()
        .unwrap();
}

#[test]
fn test_bump_patch_version() {
    let dir = create_temp_cargo_project(
        r#"
[package]
name = "test"
version = "0.1.2"
"#,
    );
    let manifest_path = dir.path().join("Cargo.toml");

    init_test_git_repo(dir.path());

    let args = BumpArgs {
        manifest_path: Some(manifest_path.clone()),
        version: None,
        auto: false,
        major: false,
        minor: false,
        patch: true,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: true, // Don't commit in tests
    };

    let result = bump(args);
    assert!(result.is_ok());

    // Verify version was updated
    let content = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(content.contains("version = \"0.1.3\""));
}

#[test]
fn test_bump_minor_version() {
    let dir = create_temp_cargo_project(
        r#"
[package]
name = "test"
version = "0.1.2"
"#,
    );
    let manifest_path = dir.path().join("Cargo.toml");

    let args = BumpArgs {
        manifest_path: Some(manifest_path.clone()),
        version: None,
        auto: false,
        major: false,
        minor: true,
        patch: false,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: true,
    };

    let result = bump(args);
    assert!(result.is_ok());

    let content = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(content.contains("version = \"0.2.0\""));
}

#[test]
fn test_bump_major_version() {
    let dir = create_temp_cargo_project(
        r#"
[package]
name = "test"
version = "0.1.2"
"#,
    );
    let manifest_path = dir.path().join("Cargo.toml");

    let args = BumpArgs {
        manifest_path: Some(manifest_path.clone()),
        version: None,
        auto: false,
        major: true,
        minor: false,
        patch: false,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: true,
    };

    let result = bump(args);
    assert!(result.is_ok());

    let content = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(content.contains("version = \"1.0.0\""));
}

#[test]
fn test_bump_manual_version() {
    let dir = create_temp_cargo_project(
        r#"
[package]
name = "test"
version = "0.1.2"
"#,
    );
    let manifest_path = dir.path().join("Cargo.toml");

    let args = BumpArgs {
        manifest_path: Some(manifest_path.clone()),
        version: Some("2.5.10".to_string()),
        auto: false,
        major: false,
        minor: false,
        patch: false,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: true,
    };

    let result = bump(args);
    assert!(result.is_ok());

    let content = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(content.contains("version = \"2.5.10\""));
}

#[test]
fn test_bump_same_version_error() {
    let dir = create_temp_cargo_project(
        r#"
[package]
name = "test"
version = "0.1.2"
"#,
    );
    let manifest_path = dir.path().join("Cargo.toml");

    let args = BumpArgs {
        manifest_path: Some(manifest_path),
        version: Some("0.1.2".to_string()),
        auto: false,
        major: false,
        minor: false,
        patch: false,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: true,
    };

    let result = bump(args);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("already the target version")
    );
}

/// Create a test git repository using gix (not git commands).
///
/// This creates a proper git repository with:
/// - Initial commit containing Cargo.toml
/// - Proper author/committer configuration
/// - Ready for testing bump operations
fn create_test_git_repo_with_gix(dir: &std::path::Path, initial_content: &str) -> gix::Repository {
    use gix::index::{
        State,
        entry,
    };
    use smallvec::SmallVec;

    // Initialize repository
    let repo = gix::init(dir).expect("Failed to initialize git repository");

    // Create Cargo.toml
    let manifest_path = dir.join("Cargo.toml");
    std::fs::write(&manifest_path, initial_content).expect("Failed to write Cargo.toml");

    // Create src/lib.rs for valid cargo project
    let src_dir = dir.join("src");
    std::fs::create_dir_all(&src_dir).expect("Failed to create src directory");
    std::fs::write(src_dir.join("lib.rs"), "// Test library\n").expect("Failed to write lib.rs");

    // Create initial commit using gix
    // 1. Create empty index
    let mut index_state = State::new(repo.object_hash());

    // 2. Add Cargo.toml to index
    let cargo_toml_blob = repo
        .write_object(gix::objs::Blob {
            data: initial_content.as_bytes().into(),
        })
        .expect("Failed to write Cargo.toml blob")
        .detach();

    let cargo_path: &bstr::BStr = b"Cargo.toml".into();
    index_state.dangerously_push_entry(
        entry::Stat::default(),
        cargo_toml_blob,
        entry::Flags::empty(),
        entry::Mode::FILE,
        cargo_path,
    );

    // 3. Add src/lib.rs to index
    let lib_rs_blob = repo
        .write_object(gix::objs::Blob {
            data: b"// Test library\n".into(),
        })
        .expect("Failed to write lib.rs blob")
        .detach();

    let lib_path: &bstr::BStr = b"src/lib.rs".into();
    index_state.dangerously_push_entry(
        entry::Stat::default(),
        lib_rs_blob,
        entry::Flags::empty(),
        entry::Mode::FILE,
        lib_path,
    );

    index_state.sort_entries();

    // 4. Build tree from index
    use gix::objs::{
        Tree,
        tree,
    };

    // Create src/ subtree
    let src_tree = Tree {
        entries: vec![tree::Entry {
            mode: tree::EntryMode::from(tree::EntryKind::Blob),
            filename: b"lib.rs".into(),
            oid: lib_rs_blob,
        }],
    };
    let src_tree_id = repo
        .write_object(&src_tree)
        .expect("Failed to write src tree")
        .detach();

    // Create root tree
    let root_tree = Tree {
        entries: vec![
            tree::Entry {
                mode: tree::EntryMode::from(tree::EntryKind::Blob),
                filename: b"Cargo.toml".into(),
                oid: cargo_toml_blob,
            },
            tree::Entry {
                mode: tree::EntryMode::from(tree::EntryKind::Tree),
                filename: b"src".into(),
                oid: src_tree_id,
            },
        ],
    };
    let tree_id = repo
        .write_object(&root_tree)
        .expect("Failed to write root tree")
        .detach();

    // 5. Create initial commit
    let author = gix::actor::Signature {
        name: "Test User".into(),
        email: "test@example.com".into(),
        time: gix::date::Time {
            seconds: 1234567890,
            offset: 0,
        },
    };

    let commit = gix::objs::Commit {
        tree: tree_id,
        parents: SmallVec::new(),
        author: author.clone(),
        committer: author,
        message: "Initial commit".into(),
        encoding: None,
        extra_headers: vec![],
    };
    let commit_id = repo
        .write_object(&commit)
        .expect("Failed to write commit")
        .detach();

    // 6. Create and update main branch
    repo.refs
        .transaction()
        .prepare(
            vec![gix::refs::transaction::RefEdit {
                change: gix::refs::transaction::Change::Update {
                    log: gix::refs::transaction::LogChange {
                        mode: gix::refs::transaction::RefLog::AndReference,
                        force_create_reflog: false,
                        message: "initial commit".into(),
                    },
                    expected: gix::refs::transaction::PreviousValue::Any,
                    new: gix::refs::Target::Object(commit_id),
                },
                name: "refs/heads/main".try_into().expect("Invalid ref name"),
                deref: false,
            }],
            gix::lock::acquire::Fail::Immediately,
            gix::lock::acquire::Fail::Immediately,
        )
        .expect("Failed to prepare transaction")
        .commit(Some(gix::actor::SignatureRef {
            name: "Test User".into(),
            email: "test@example.com".into(),
            time: "1234567890 +0000",
        }))
        .expect("Failed to commit transaction");

    // Set user.name and user.email in repo config for bump command
    let config_path = repo.path().join("config");
    let config_content = std::fs::read_to_string(&config_path).unwrap_or_else(|_| String::new());
    let new_config = format!(
        "{}\n[user]\n\tname = Test User\n\temail = test@example.com\n",
        config_content
    );
    std::fs::write(&config_path, new_config).expect("Failed to write config");

    repo
}

#[test]
fn test_hunk_level_staging_only_version_line() {
    // Create repo with initial content
    let dir = tempfile::tempdir().unwrap();
    let initial_content = r#"[package]
name = "test"
version = "0.1.0"
description = "original description"
edition = "2021"
"#;

    let _repo = create_test_git_repo_with_gix(dir.path(), initial_content);

    // Modify Cargo.toml: change version AND description
    let manifest_path = dir.path().join("Cargo.toml");
    let modified_content = r#"[package]
name = "test"
version = "0.1.0"
description = "modified description"
edition = "2021"
"#;
    std::fs::write(&manifest_path, modified_content).expect("Failed to modify Cargo.toml");

    // Run bump command
    let args = BumpArgs {
        manifest_path: Some(manifest_path.clone()),
        version: Some("0.2.0".to_string()),
        auto: false,
        major: false,
        minor: false,
        patch: false,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: false, // DO commit
    };

    let result = bump(args);
    assert!(result.is_ok(), "Bump failed: {:?}", result.err());

    // Verify the commit using gix
    let repo = gix::open(dir.path()).expect("Failed to open repo");
    let head = repo.head().expect("Failed to read HEAD");
    let commit_id = head.id().expect("HEAD not pointing to commit");
    let commit = repo
        .find_object(commit_id)
        .expect("Failed to find commit")
        .try_into_commit()
        .expect("Not a commit");

    // Get the tree from the commit
    let tree = commit.tree().expect("Failed to get tree");

    // Get Cargo.toml from the commit
    let cargo_entry = tree
        .lookup_entry_by_path("Cargo.toml")
        .expect("Failed to lookup Cargo.toml")
        .expect("Cargo.toml not in commit");

    let blob = cargo_entry
        .object()
        .expect("Failed to get blob")
        .try_into_blob()
        .expect("Not a blob");

    let committed_content = blob.data.to_str_lossy();

    // Verify ONLY version line changed
    assert!(
        committed_content.contains("version = \"0.2.0\""),
        "Version should be updated in commit"
    );
    assert!(
        committed_content.contains("description = \"original description\""),
        "Description should NOT be changed in commit (should be original)"
    );
    assert!(
        !committed_content.contains("description = \"modified description\""),
        "Modified description should NOT be in commit"
    );

    // Verify working directory still has the description change
    let working_content = std::fs::read_to_string(&manifest_path).expect("Failed to read file");
    assert!(
        working_content.contains("description = \"modified description\""),
        "Working directory should still have modified description"
    );
}

#[test]
fn test_hunk_level_staging_multiple_changes() {
    // Test with multiple non-version changes
    let dir = tempfile::tempdir().unwrap();
    let initial_content = r#"[package]
name = "test"
version = "1.0.0"
authors = ["Original Author"]
description = "A test crate"
license = "MIT"
"#;

    let _repo = create_test_git_repo_with_gix(dir.path(), initial_content);

    // Modify multiple fields including version
    let manifest_path = dir.path().join("Cargo.toml");
    let modified_content = r#"[package]
name = "test"
version = "1.0.0"
authors = ["New Author"]
description = "An updated test crate"
license = "Apache-2.0"
"#;
    std::fs::write(&manifest_path, modified_content).expect("Failed to modify Cargo.toml");

    // Run bump to change version
    let args = BumpArgs {
        manifest_path: Some(manifest_path.clone()),
        patch: true,
        version: None,
        auto: false,
        major: false,
        minor: false,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: false,
    };

    let result = bump(args);
    assert!(result.is_ok(), "Bump failed: {:?}", result.err());

    // Verify the commit
    let repo = gix::open(dir.path()).expect("Failed to open repo");
    let head = repo.head().expect("Failed to read HEAD");
    let commit_id = head.id().expect("HEAD not pointing to commit");
    let commit = repo
        .find_object(commit_id)
        .expect("Failed to find commit")
        .try_into_commit()
        .expect("Not a commit");

    let tree = commit.tree().expect("Failed to get tree");
    let cargo_entry = tree
        .lookup_entry_by_path("Cargo.toml")
        .expect("Failed to lookup Cargo.toml")
        .expect("Cargo.toml not in commit");

    let blob = cargo_entry
        .object()
        .expect("Failed to get blob")
        .try_into_blob()
        .expect("Not a blob");

    let committed_content = blob.data.to_str_lossy();

    // Verify ONLY version changed
    assert!(
        committed_content.contains("version = \"1.0.1\""),
        "Version should be bumped to 1.0.1"
    );
    assert!(
        committed_content.contains("authors = [\"Original Author\"]"),
        "Authors should be original, not modified"
    );
    assert!(
        committed_content.contains("description = \"A test crate\""),
        "Description should be original, not modified"
    );
    assert!(
        committed_content.contains("license = \"MIT\""),
        "License should be original, not modified"
    );

    // Verify working directory still has all the other changes
    let working_content = std::fs::read_to_string(&manifest_path).expect("Failed to read file");
    assert!(working_content.contains("authors = [\"New Author\"]"));
    assert!(working_content.contains("description = \"An updated test crate\""));
    assert!(working_content.contains("license = \"Apache-2.0\""));
}

#[test]
fn test_commit_has_proper_author() {
    // Verify commits have proper author from git config
    let dir = tempfile::tempdir().unwrap();
    let initial_content = r#"[package]
name = "test"
version = "0.5.0"
"#;

    let _repo = create_test_git_repo_with_gix(dir.path(), initial_content);

    let manifest_path = dir.path().join("Cargo.toml");

    // Run bump
    let args = BumpArgs {
        manifest_path: Some(manifest_path),
        patch: true,
        version: None,
        auto: false,
        major: false,
        minor: false,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: false,
    };

    let result = bump(args);
    assert!(result.is_ok(), "Bump failed: {:?}", result.err());

    // Verify the commit has proper author
    let repo = gix::open(dir.path()).expect("Failed to open repo");
    let head = repo.head().expect("Failed to read HEAD");
    let commit_id = head.id().expect("HEAD not pointing to commit");
    let commit = repo
        .find_object(commit_id)
        .expect("Failed to find commit")
        .try_into_commit()
        .expect("Not a commit");

    // Check author
    let author = commit.author().expect("Failed to get author");
    assert_eq!(
        author.name.to_string(),
        "Test User",
        "Author name should be set"
    );
    assert_eq!(
        author.email.as_bstr(),
        "test@example.com",
        "Author email should be set"
    );
    // Check that time is set (not empty)
    assert!(!author.time.is_empty(), "Author time should not be empty");

    // Check committer
    let committer = commit.committer().expect("Failed to get committer");
    assert_eq!(
        committer.name.to_string(),
        "Test User",
        "Committer name should be set"
    );
    assert_eq!(
        committer.email.to_string(),
        "test@example.com",
        "Committer email should be set"
    );
}

#[test]
fn test_only_version_file_in_commit_not_other_staged_files() {
    // Verify that bump doesn't include other staged files
    let dir = tempfile::tempdir().unwrap();
    let initial_content = r#"[package]
name = "test"
version = "2.0.0"
"#;

    let repo = create_test_git_repo_with_gix(dir.path(), initial_content);

    // Create another file and stage it (but don't commit)
    let readme_path = dir.path().join("README.md");
    std::fs::write(&readme_path, "# Test Project\n").expect("Failed to write README");

    // Stage the README using gix
    let index_path = repo.path().join("index");

    use gix::index::{
        File,
        State,
        entry,
    };

    // Create or load index
    let mut index_state = if index_path.exists() {
        let file = File::at(
            &index_path,
            repo.object_hash(),
            false,
            gix::index::decode::Options::default(),
        )
        .expect("Failed to read index");
        State::from(file)
    } else {
        // Index doesn't exist yet, create empty one
        State::new(repo.object_hash())
    };

    // Add README.md to index
    let readme_blob = repo
        .write_object(gix::objs::Blob {
            data: b"# Test Project\n".into(),
        })
        .expect("Failed to write README blob")
        .detach();

    let readme_path_bstr: &bstr::BStr = b"README.md".into();
    index_state.dangerously_push_entry(
        entry::Stat::default(),
        readme_blob,
        entry::Flags::empty(),
        entry::Mode::FILE,
        readme_path_bstr,
    );
    index_state.sort_entries();

    // Write index back to disk (staging README.md)
    let mut index_file_write =
        std::fs::File::create(&index_path).expect("Failed to create index file");
    index_state
        .write_to(&mut index_file_write, gix::index::write::Options::default())
        .expect("Failed to write index");

    // Now run bump - it should NOT include README.md
    let manifest_path = dir.path().join("Cargo.toml");
    let args = BumpArgs {
        manifest_path: Some(manifest_path),
        major: true,
        version: None,
        auto: false,
        minor: false,
        patch: false,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: false,
    };

    let result = bump(args);
    assert!(result.is_ok(), "Bump failed: {:?}", result.err());

    // Verify the commit does NOT contain README.md
    let repo = gix::open(dir.path()).expect("Failed to open repo");
    let head = repo.head().expect("Failed to read HEAD");
    let commit_id = head.id().expect("HEAD not pointing to commit");
    let commit = repo
        .find_object(commit_id)
        .expect("Failed to find commit")
        .try_into_commit()
        .expect("Not a commit");

    let tree = commit.tree().expect("Failed to get tree");

    // Verify Cargo.toml is in the commit
    assert!(
        tree.lookup_entry_by_path("Cargo.toml")
            .expect("Failed to lookup")
            .is_some(),
        "Cargo.toml should be in commit"
    );

    // Verify README.md is NOT in the commit
    assert!(
        tree.lookup_entry_by_path("README.md")
            .expect("Failed to lookup")
            .is_none(),
        "README.md should NOT be in commit (was staged but not committed by bump)"
    );

    // The key assertion passed: README.md was staged but NOT included in the
    // bump commit. This proves the bump command creates a minimal index with
    // only the version file, regardless of what's in .git/index.
}

#[test]
fn test_preserves_all_files_from_head() {
    // CRITICAL REGRESSION TEST:
    // Verify that bump doesn't delete other files by creating a minimal tree.
    // This is the bug that caused all files to be deleted in commit 7192f12.

    let dir = tempfile::tempdir().unwrap();
    let initial_content = r#"[package]
name = "test"
version = "1.0.0"
"#;

    let _repo = create_test_git_repo_with_gix(dir.path(), initial_content);

    // The initial commit has:
    // - Cargo.toml
    // - src/lib.rs (created by create_test_git_repo_with_gix)
    // Both should be in the bump commit!

    // Run bump
    let manifest_path = dir.path().join("Cargo.toml");
    let args = BumpArgs {
        manifest_path: Some(manifest_path),
        patch: true,
        version: None,
        auto: false,
        major: false,
        minor: false,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: false,
    };

    let result = bump(args);
    assert!(result.is_ok(), "Bump failed: {:?}", result.err());

    // Verify the commit using gix
    let repo = gix::open(dir.path()).expect("Failed to open repo");
    let head = repo.head().expect("Failed to read HEAD");
    let commit_id = head.id().expect("HEAD not pointing to commit");
    let commit = repo
        .find_object(commit_id)
        .expect("Failed to find commit")
        .try_into_commit()
        .expect("Not a commit");

    let tree = commit.tree().expect("Failed to get tree");

    // CRITICAL: Verify Cargo.toml is in the commit
    assert!(
        tree.lookup_entry_by_path("Cargo.toml")
            .expect("Failed to lookup")
            .is_some(),
        "Cargo.toml should be in commit"
    );

    // CRITICAL: Verify src/lib.rs is STILL in the commit (not deleted!)
    let src_entry = tree
        .lookup_entry_by_path("src/lib.rs")
        .expect("Failed to lookup src/lib.rs");

    assert!(
        src_entry.is_some(),
        "src/lib.rs should still be in commit - bump should preserve all files from HEAD!"
    );

    // Verify src/lib.rs content is unchanged
    if let Some(entry) = src_entry {
        let blob = entry
            .object()
            .expect("Failed to get blob")
            .try_into_blob()
            .expect("Not a blob");

        let content = blob.data.to_str_lossy();
        assert_eq!(
            content, "// Test library\n",
            "src/lib.rs content should be unchanged"
        );
    }

    // Verify Cargo.toml version was updated
    let cargo_entry = tree
        .lookup_entry_by_path("Cargo.toml")
        .expect("Failed to lookup")
        .expect("Cargo.toml not in tree");

    let cargo_blob = cargo_entry
        .object()
        .expect("Failed to get blob")
        .try_into_blob()
        .expect("Not a blob");

    let cargo_content = cargo_blob.data.to_str_lossy();
    assert!(
        cargo_content.contains("version = \"1.0.1\""),
        "Cargo.toml version should be bumped"
    );
}

#[test]
fn test_preserves_multiple_files_and_directories() {
    // Extended regression test: verify bump preserves complex directory structures
    let dir = tempfile::tempdir().unwrap();
    let initial_content = r#"[package]
name = "multi-file-test"
version = "0.5.0"
"#;

    let _repo = create_test_git_repo_with_gix(dir.path(), initial_content);

    // Add more files to the initial commit
    // Create additional files: README.md, .gitignore, docs/guide.md
    std::fs::write(dir.path().join("README.md"), "# Project\n").expect("Failed to write README");
    std::fs::write(dir.path().join(".gitignore"), "target/\n").expect("Failed to write .gitignore");

    let docs_dir = dir.path().join("docs");
    std::fs::create_dir_all(&docs_dir).expect("Failed to create docs dir");
    std::fs::write(docs_dir.join("guide.md"), "# Guide\n").expect("Failed to write guide");

    // Build a tree with all files
    // For simplicity, we'll use git commands to add these files
    // (the test is about bump, not about our tree building)
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .expect("Failed to git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "Add more files"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to git commit");

    // Now run bump
    let manifest_path = dir.path().join("Cargo.toml");
    let args = BumpArgs {
        manifest_path: Some(manifest_path),
        minor: true,
        version: None,
        auto: false,
        major: false,
        patch: false,
        owner: None,
        repo: None,
        github_token: None,
        no_commit: false,
    };

    let result = bump(args);
    assert!(result.is_ok(), "Bump failed: {:?}", result.err());

    // Verify the bump commit
    let repo = gix::open(dir.path()).expect("Failed to open repo");
    let head = repo.head().expect("Failed to read HEAD");
    let commit_id = head.id().expect("HEAD not pointing to commit");
    let commit = repo
        .find_object(commit_id)
        .expect("Failed to find commit")
        .try_into_commit()
        .expect("Not a commit");

    let tree = commit.tree().expect("Failed to get tree");

    // Verify ALL files are still present
    assert!(
        tree.lookup_entry_by_path("Cargo.toml")
            .expect("Failed to lookup")
            .is_some(),
        "Cargo.toml should be in commit"
    );
    assert!(
        tree.lookup_entry_by_path("README.md")
            .expect("Failed to lookup")
            .is_some(),
        "README.md should still be in commit (not deleted!)"
    );
    assert!(
        tree.lookup_entry_by_path(".gitignore")
            .expect("Failed to lookup")
            .is_some(),
        ".gitignore should still be in commit (not deleted!)"
    );
    assert!(
        tree.lookup_entry_by_path("src/lib.rs")
            .expect("Failed to lookup")
            .is_some(),
        "src/lib.rs should still be in commit (not deleted!)"
    );
    assert!(
        tree.lookup_entry_by_path("docs/guide.md")
            .expect("Failed to lookup")
            .is_some(),
        "docs/guide.md should still be in commit (not deleted!)"
    );

    // Verify Cargo.toml version was updated
    let cargo_entry = tree
        .lookup_entry_by_path("Cargo.toml")
        .expect("Failed to lookup")
        .expect("Cargo.toml not in tree");

    let cargo_blob = cargo_entry
        .object()
        .expect("Failed to get blob")
        .try_into_blob()
        .expect("Not a blob");

    let cargo_content = cargo_blob.data.to_str_lossy();
    assert!(
        cargo_content.contains("version = \"0.6.0\""),
        "Cargo.toml version should be bumped (minor: 0.5.0 -> 0.6.0)"
    );
}
