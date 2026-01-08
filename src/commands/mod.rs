//! Command implementations.

mod badge;
mod build_version;
pub mod bump;
mod changed;
pub mod changelog;
mod compare;
mod current;
mod dev;
mod dioxus;
mod latest;
mod next;
mod post_bump_hook;
mod pr_log;
mod pre_bump_hook;
mod release_page;
mod rust_toolchain;
mod tag;
mod update_readme;

// Re-export all command argument structs
pub use badge::{
    BadgeArgs,
    badge,
};
pub use build_version::{
    BuildVersionArgs,
    build_version,
    build_version_default,
    build_version_for_repo,
    compute_version_string,
};
pub use bump::{
    BumpArgs,
    bump,
};
pub use changed::{
    ChangedArgs,
    changed,
};
pub use changelog::{
    ChangelogArgs,
    changelog,
};
pub use compare::{
    CompareArgs,
    compare,
};
pub use current::{
    CurrentArgs,
    current,
};
pub use dev::{
    DevArgs,
    dev,
};
pub use dioxus::{
    DioxusArgs,
    dioxus,
};
pub use latest::{
    LatestArgs,
    latest,
};
pub use next::NextArgs;
// Re-export all command functions
pub use next::next;
pub use post_bump_hook::{
    PostBumpHookArgs,
    post_bump_hook,
};
pub use pr_log::{
    PrLogArgs,
    pr_log,
};
pub use pre_bump_hook::{
    PreBumpHookArgs,
    pre_bump_hook,
};
pub use release_page::{
    ReleasePageArgs,
    release_page,
};
pub use rust_toolchain::{
    RustToolchainArgs,
    rust_toolchain,
};
pub use tag::{
    TagArgs,
    tag,
};
pub use update_readme::{
    UpdateReadmeArgs,
    update_readme,
};
