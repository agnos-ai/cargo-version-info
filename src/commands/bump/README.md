# Bump Command Architecture

This directory contains the implementation of the
`cargo version-info bump` command, refactored into a well-organized
module structure with extensive documentation.

## Module Structure

```text
bump/
├── mod.rs              # Main entry point, orchestration
├── args.rs             # CLI argument definitions
├── version_update.rs   # TOML manipulation (toml_edit)
├── commit.rs           # Commit orchestration
├── diff.rs             # Diff generation & hunk filtering
├── index.rs            # Git index (staging) operations
├── tree.rs             # Git tree building
├── tests.rs            # Comprehensive test suite
└── README.md           # This file
```

## Data Flow

```text
CLI Args → Calculate Version → Update TOML → Commit Changes
                                                    ↓
                       ┌────────────────────────────┘
                       ↓
            Verify Changes → Detect Other Changes → Apply Hunks → Stage File → Build Tree → Create Commit → Update HEAD
                                     ↓                   ↓           ↓            ↓              ↓
                                 diff.rs             diff.rs     index.rs     tree.rs      commit.rs
```

## Key Concepts

### Git Index (Staging Area)

The index is git's "staging area" - a binary file (`.git/index`) that
tracks which files should be included in the next commit.

**Path Storage**: The index uses a clever optimization where all paths
are stored in a single byte array, and entries reference paths via
`Range<usize>`.

See: `index.rs` for detailed documentation.

### Git Trees

Trees are git's way of representing directory hierarchies. They're
stored as objects in `.git/objects/` and contain entries for files and
subdirectories.

**Current Limitation**: We use a simplified single-level tree builder.
Full recursive tree building is a TODO.

See: `tree.rs` for detailed documentation and the tree building
algorithm.

### Selective Staging (Hunk-Level)

The goal is to stage ONLY version changes, not other uncommitted work.
This is the core value proposition of the bump command.

**✅ Implemented**: True hunk-level staging using the `similar` crate:

- Generates unified diff between HEAD and working directory
- Identifies version-related hunks (lines containing version strings)
- Creates partially-staged content with only version changes
- Leaves non-version changes uncommitted

See: `diff.rs` for the hunk filtering algorithm and `commit.rs` for
orchestration.

## Git Operations with gix

All git operations use `gix` (gitoxide) instead of shelling out to the
git binary. This provides:

- **Type Safety**: Compile-time guarantees
- **Performance**: No process spawning
- **Consistency**: Unified API
- **Testability**: Easier to test

### gix API Complexity

The gix index and tree APIs are low-level and require careful
handling:

- Path storage must be managed manually
- Entries must be kept sorted
- Tree building isn't automated (we do it manually)

This complexity is why the code is heavily documented.

## Testing Strategy

### Unit Tests (`tests.rs`)

- Version calculation (patch, minor, major, manual)
- TOML updates (package, workspace, formatting)
- Error cases (missing sections, same version)

### Doctests

Each public function has compilable example code in its documentation.

### Test Setup

Test setup uses git commands for simplicity (initializing repos,
creating commits). The important part is that the **production code**
(the `bump` function) uses `gix`.

## Documentation Philosophy

The code is heavily documented because:

1. **Git internals are complex**: Index format, tree structure, object
   storage
2. **gix API is low-level**: Requires understanding git internals
3. **Future maintainers**: Should understand WHY not just WHAT
4. **Educational value**: Good reference for git internals

Each module contains:

- Overview of the problem domain
- Explanation of key concepts
- Implementation rationale
- Future improvement suggestions
- Comprehensive examples

## Implementation Status

### ✅ Hunk-Level Staging (Implemented!)

The bump command now implements true line-level selective staging:

**How it works**:

1. Generate unified diff between HEAD and working directory
2. Identify which lines contain version-related changes
3. Create partially-staged content with only those lines changed
4. Write the partial content as a blob
5. Build tree and commit with only version changes

**Example scenario**:

```toml
# In working directory, Cargo.toml has:
# - Line 3: version changed from 0.1.0 → 0.2.0 (staged ✓)
# - Line 15: description changed (NOT staged ✗)

# Result: Commit contains only the version line change!
```

See `diff.rs` for the implementation using the `similar` crate.

### Recursive Tree Building

Currently builds a single-level tree (all files flattened).

For full support:

1. Parse paths to identify directory structure
2. Build subtrees recursively from leaves to root
3. Optimize by reusing unchanged subtrees
4. Handle special cases (submodules, gitlinks)

See `tree.rs` for detailed discussion.

## Related Documentation

- [Git Index Format](https://git-scm.com/docs/index-format)
- [Git Tree Objects](https://git-scm.com/book/en/v2/Git-Internals-Git-Objects#_tree_objects)
- [gix Documentation](https://docs.rs/gix)
- [gitoxide Project](https://github.com/Byron/gitoxide)
