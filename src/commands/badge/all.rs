//! Generate all badges.

use std::io::Write;

use anyhow::Result;

use super::{
    adrs,
    coverage,
    crates_io,
    docs_rs,
    framework,
    license,
    number_of_tests,
    platform,
    runtime,
    rust_edition,
};

/// Generate all badges
pub async fn badge_all(
    writer: &mut dyn Write,
    package: &cargo_metadata::Package,
    no_network: bool,
) -> Result<()> {
    docs_rs::badge_rustdocs(writer, package, no_network).await?;
    crates_io::badge_cratesio(writer, package, no_network).await?;
    license::badge_license(writer, package).await?;
    rust_edition::badge_rust_edition(writer, package).await?;
    runtime::badge_runtime(writer, package).await?;
    framework::badge_framework(writer, package).await?;
    platform::badge_platform(writer, package).await?;
    adrs::badge_adrs(writer, package).await?;
    coverage::badge_coverage(writer, package).await?;
    number_of_tests::badge_number_of_tests(writer, package).await?;

    Ok(())
}
