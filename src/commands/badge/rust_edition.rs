//! Generate Rust edition badge.

use std::io::Write;

use anyhow::Result;

/// Show the Rust edition badge.
pub async fn badge_rust_edition(
    writer: &mut dyn Write,
    package: &cargo_metadata::Package,
) -> Result<()> {
    let mut logger = cargo_plugin_utils::logger::Logger::new();
    logger.status("Generating", "Rust edition badge");

    let edition_str = package.edition.as_str();
    let badge_url = format!(
        "https://img.shields.io/badge/rust%20edition-{}-orange",
        edition_str
    );
    let badge_markdown = format!("[![Rust Edition]({})](Cargo.toml)", badge_url);
    writeln!(writer, "{}", badge_markdown)?;

    Ok(())
}
