// Promotion Validation Module
// Validates promotion configs against schema and requirements

use anyhow::{anyhow, Result};
use super::PromotionConfig;

/// Validate promotion configuration
pub fn validate_promotion_config(config: &PromotionConfig) -> Result<()> {
    // Check version format
    if !is_valid_version(&config.version) {
        return Err(anyhow!("Invalid version format: {}", config.version));
    }

    // Check artifacts exist
    if config.artifacts.is_empty() {
        return Err(anyhow!("No artifacts defined in [promotion.artifacts]"));
    }

    // Validate each artifact
    for (name, artifact) in &config.artifacts {
        validate_artifact(name, artifact)?;
    }

    Ok(())
}

/// Validate version string (semantic versioning)
fn is_valid_version(version: &str) -> bool {
    // Allow "live" as special case for LIVE mode
    if version == "live" {
        return true;
    }

    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 3 {
        return false;
    }

    // Check major.minor.patch are numbers
    parts[0].parse::<u32>().is_ok()
        && parts[1].parse::<u32>().is_ok()
        && parts[2].split('-').next().unwrap().parse::<u32>().is_ok()
}

/// Validate artifact configuration
fn validate_artifact(name: &str, artifact: &toml::Value) -> Result<()> {
    let table = artifact.as_table()
        .ok_or_else(|| anyhow!("Artifact {} must be a table", name))?;

    // Check required fields
    if !table.contains_key("source") {
        return Err(anyhow!("Artifact {} missing 'source' field", name));
    }
    if !table.contains_key("target") {
        return Err(anyhow!("Artifact {} missing 'target' field", name));
    }

    // Validate paths use XDG-compliant prefixes
    let source = table.get("source").and_then(|v| v.as_str()).unwrap();
    let target = table.get("target").and_then(|v| v.as_str()).unwrap();

    if !source.starts_with("~/") && !source.starts_with("$HOME/") {
        return Err(anyhow!("Artifact {} source must use ~/ or $HOME/ prefix: {}", name, source));
    }
    if !target.starts_with("~/") && !target.starts_with("$HOME/") {
        return Err(anyhow!("Artifact {} target must use ~/ or $HOME/ prefix: {}", name, target));
    }

    Ok(())
}
