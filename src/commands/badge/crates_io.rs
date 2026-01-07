//! Generate crates.io badge.

use std::io::Write;

use anyhow::{
    Context,
    Result,
};

use super::common::guess_if_published;

/// Check if crate is published on crates.io.
///
/// Uses HTTP request when `no_network` is false, otherwise uses heuristics.
async fn is_published_on_crates_io(
    package_name: &str,
    package: &cargo_metadata::Package,
    no_network: bool,
) -> Result<bool> {
    if no_network {
        guess_if_published(package).await
    } else {
        let api_url = format!("https://crates.io/api/v1/crates/{}", package_name);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .context("Failed to create HTTP client")?;

        let response = client
            .get(&api_url)
            .header("User-Agent", "cargo-version-info")
            .send()
            .await
            .context("Failed to check crates.io")?;

        Ok(response.status().is_success())
    }
}

/// Show the crates.io badge if the project is published there.
pub async fn badge_cratesio(
    writer: &mut dyn Write,
    package: &cargo_metadata::Package,
    no_network: bool,
) -> Result<()> {
    let mut logger = cargo_plugin_utils::logger::Logger::new();
    logger.status("Generating", "crates.io badge");

    let package_name = &package.name;

    if is_published_on_crates_io(package_name, package, no_network).await? {
        let badge_url = format!("https://img.shields.io/crates/v/{}", package_name);
        let badge_markdown = format!(
            "[![crates.io]({})](https://crates.io/crates/{})",
            badge_url, package_name
        );
        writeln!(writer, "{}", badge_markdown)?;
    }

    Ok(())
}
