# cargo-version-info

A Cargo subcommand for unified version management across CI/CD, Rust
code, and shell scripts.

## Overview

`cargo-version-info` provides a single source of truth for version
operations, replacing scattered version logic in GitHub Actions
workflows, bash scripts, Makefiles, and Rust build scripts.

## Installation

```bash
# Via cargo-binstall (recommended)
cargo binstall cargo-version-info

# Via cargo install
cargo install cargo-version-info

# From source
cargo install --path .
```

## Commands

### `cargo version-info next`

Calculate the next patch version from the latest GitHub release.

```bash
# Basic usage (auto-detects repo from git remote)
cargo version-info next

# Specify repository explicitly
cargo version-info next --owner dataroad --repo dr-rs-ekg

# Output as tag format
cargo version-info next --format tag

# Output as JSON
cargo version-info next --format json
```

**Output formats:**

- `version` (default): Just the version number (e.g., `0.0.6`)
- `tag`: Version with v prefix (e.g., `v0.0.6`)
- `json`: JSON object with `latest`, `next`, and `next_tag` fields

### `cargo version-info current`

Get the current version from `Cargo.toml`.

```bash
# Read from default Cargo.toml
cargo version-info current

# Specify custom manifest path
cargo version-info current --manifest ./crates/my-crate/Cargo.toml

# Output as JSON
cargo version-info current --format json
```

**Output formats:**

- `version` (default): Just the version number
- `json`: JSON object with `version` field

### `cargo version-info latest`

Get the latest published GitHub release version.

```bash
# Auto-detect repository
cargo version-info latest

# Specify repository
cargo version-info latest --owner dataroad --repo dr-rs-ekg

# Output as tag format
cargo version-info latest --format tag
```

**Output formats:**

- `version` (default): Just the version number
- `tag`: Version with v prefix
- `json`: JSON object with `version` and `tag` fields

### `cargo version-info dev`

Generate a dev version from the current git SHA.

```bash
# Use current directory
cargo version-info dev

# Specify repository path
cargo version-info dev --repo-path ./some/path

# Output as JSON
cargo version-info dev --format json
```

**Output format:** `0.0.0-dev-<short-sha>` (e.g., `0.0.0-dev-a1b2c3d`)

### `cargo version-info tag`

Generate a tag name from a version string.

```bash
# Generate tag from version
cargo version-info tag 0.1.2

# Output as JSON
cargo version-info tag 0.1.2 --format json
```

**Output:** `v0.1.2`

### `cargo version-info compare`

Compare two versions.

```bash
# Compare versions (outputs true if version1 > version2)
cargo version-info compare 0.1.2 0.1.3

# Output as JSON
cargo version-info compare 0.1.2 0.1.3 --format json

# Output as diff format
cargo version-info compare 0.1.2 0.1.3 --format diff
```

**Output formats:**

- `bool` (default): `true` if version1 > version2, `false` otherwise
- `json`: JSON object with comparison result
- `diff`: Human-readable comparison (e.g., `0.1.2 < 0.1.3`)

## Environment Variables

- `GITHUB_TOKEN`: GitHub personal access token for API access
  (optional, falls back to `gh` CLI)
- `GITHUB_REPOSITORY`: Repository in `owner/repo` format
  (auto-detected from git remote if not set)

## Use Cases

### GitHub Actions

Replace bash scripts in GitHub Actions workflows:

```yaml
- name: Calculate next version
  run: |
    NEXT_VERSION=$(cargo version-info next --format version)
    echo "version=$NEXT_VERSION" >> $GITHUB_OUTPUT
```

### Bash Scripts

Replace version extraction logic:

```bash
# Instead of: VERSION=$(grep '^version' Cargo.toml | ...)
VERSION=$(cargo version-info current --format version)

# Instead of: gh release list ... | jq ...
LATEST=$(cargo version-info latest --format version)
```

### Makefiles

Use in Make targets:

```makefile
VERSION := $(shell cargo version-info current --format version)
NEXT_VERSION := $(shell cargo version-info next --format version)
```

### Rust Build Scripts

Can be called from `build.rs`:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let output = std::process::Command::new("cargo")
    .args(["version-info", "current", "--format", "version"])
    .output()?;
let version = String::from_utf8(output.stdout)?;
# Ok(())
# }
```

## Integration with Existing Workflows

This tool is designed to replace:

1. **GitHub Actions**:
   `.github/actions/calculate-next-version/action.yml` and
   `.github/actions/get-version/action.yml`
2. **Bash scripts**: Version extraction in `.bash/*.sh` files
3. **Rust build scripts**: Version resolution in
   `crates/ekg-util-env/build.rs` (can call this tool)

## License

Same as the workspace (see root LICENSE file).
