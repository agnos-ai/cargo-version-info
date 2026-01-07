//! Generate docs.rs badge.

use std::io::Write;

use anyhow::{
    Context,
    Result,
};

use super::common::guess_if_published;

/// Check if crate is published on docs.rs.
///
/// Uses HTTP request when `no_network` is false, otherwise uses heuristics.
async fn is_published_on_docs_rs(
    package_name: &str,
    package: &cargo_metadata::Package,
    no_network: bool,
) -> Result<bool> {
    if no_network {
        guess_if_published(package).await
    } else {
        let docs_url = format!("https://docs.rs/{}", package_name);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .context("Failed to create HTTP client")?;

        let response = client
            .head(&docs_url)
            .send()
            .await
            .context("Failed to check docs.rs")?;

        Ok(response.status().is_success())
    }
}

/// Show the docs.rs badge if the project is published there.
pub async fn badge_rustdocs(
    writer: &mut dyn Write,
    package: &cargo_metadata::Package,
    no_network: bool,
) -> Result<()> {
    let mut logger = cargo_plugin_utils::logger::Logger::new();
    logger.status("Generating", "docs.rs badge");

    let package_name = &package.name;

    if is_published_on_docs_rs(package_name, package, no_network).await? {
        let badge_url = format!("https://img.shields.io/docsrs/{}", package_name);
        let badge_markdown = format!(
            "[![docs.rs]({})](https://docs.rs/{})",
            badge_url, package_name
        );
        writeln!(writer, "{}", badge_markdown)?;
    }

    Ok(())
}
