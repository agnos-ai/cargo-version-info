//! Generate license badge.

use std::io::Write;

use anyhow::Result;

/// Show the license badge.
pub async fn badge_license(
    writer: &mut dyn Write,
    package: &cargo_metadata::Package,
) -> Result<()> {
    let mut logger = cargo_plugin_utils::logger::Logger::new();
    logger.status("Generating", "license badge");

    if let Some(license) = &package.license {
        let license_encoded = license.replace(' ', "%20");
        let badge_url = format!("https://img.shields.io/crates/l/{}", license_encoded);
        let badge_markdown = format!(
            "[![license]({})](https://opensource.org/licenses/{})",
            badge_url, license_encoded
        );
        writeln!(writer, "{}", badge_markdown)?;
    }

    Ok(())
}
