//! Generate framework badge.

use std::io::Write;

use anyhow::Result;

/// Show the framework badge.
pub async fn badge_framework(
    writer: &mut dyn Write,
    package: &cargo_metadata::Package,
) -> Result<()> {
    let mut logger = cargo_plugin_utils::logger::Logger::new();
    logger.status("Generating", "framework badge");

    // Check dependencies for framework
    let has_axum = package.dependencies.iter().any(|dep| dep.name == "axum");

    if has_axum {
        let badge_url = "https://img.shields.io/badge/web%20framework-Axum-blueviolet";
        let badge_markdown = format!(
            "[![Framework]({})](docs/adr/0008-web-framework-axum.typ)",
            badge_url
        );
        writeln!(writer, "{}", badge_markdown)?;
    }
    // Future: add other frameworks (actix-web, warp, etc.)

    Ok(())
}
