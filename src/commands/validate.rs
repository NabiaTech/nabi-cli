//! Validation commands with maturity-based routing
//!
//! Demonstrates language-agnostic routing based on implementation maturity.
//! Routes to Python prototype or Rust production based on TOML configuration.

use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::Path;

use crate::maturity::{route_command, ImplementationRoute, MaturityStage};
use crate::paths::NabiPaths;

/// Validate transform configuration with maturity-based routing
///
/// Routes to appropriate implementation based on TOML meta.implementation_stage:
/// - prototype: Routes to Python/TS validator
/// - production: Routes to Rust validator (when production_ready=true)
/// - transitioning: Runs both, compares results
pub fn validate_transform(
    config_path: &str,
    override_stage: Option<&str>,
    show_info: bool,
) -> Result<()> {
    let config_file = expand_path(config_path)?;

    if !config_file.exists() {
        anyhow::bail!("Config file not found: {}", config_file.display());
    }

    // Load TOML config
    let toml_content = fs::read_to_string(&config_file)
        .with_context(|| format!("Failed to read TOML config: {}", config_file.display()))?;

    let toml_value: toml::Value = toml::from_str(&toml_content)
        .with_context(|| format!("Failed to parse TOML: {}", config_file.display()))?;

    // Extract meta section
    let meta = toml_value
        .get("meta")
        .context("Missing [meta] section in TOML config")?;

    // Load implementation route from TOML
    let route = ImplementationRoute::from_toml_meta(meta)
        .context("Failed to load implementation route from TOML meta section")?;

    // Get config name for display
    let config_name = config_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Show info mode
    if show_info {
        route.display_info(config_name);
        return Ok(());
    }

    // Parse override stage if provided
    let override_stage_enum = override_stage
        .map(|s| MaturityStage::from_str(s))
        .transpose()
        .context("Invalid --stage override")?;

    // Display routing decision
    println!(
        "{} {}",
        "🔍".cyan(),
        format!("Validating transform config: {}", config_name).bold()
    );

    // Route and execute validation
    let args = &[config_path];
    let result = route_command(&route, args, override_stage_enum)?;

    // Handle result
    if result.success {
        println!("{}", "✅ Validation complete".green().bold());
        if !result.stdout.is_empty() {
            println!("{}", result.stdout);
        }
        Ok(())
    } else {
        eprintln!("{}", "❌ Validation failed".red().bold());
        if !result.stderr.is_empty() {
            eprintln!("{}", result.stderr);
        }
        std::process::exit(result.exit_code);
    }
}

/// List all transformable configs with their maturity stages
pub fn list_transform_configs() -> Result<()> {
    let config_dir = NabiPaths::config_dir()?;

    println!("{}", "📋 Transform Configurations".bold());
    println!();

    // Scan common config directories for TOML files with transform meta
    let search_dirs = vec![
        config_dir.join("services"),
        config_dir.join("hooks"),
        config_dir.join("tools"),
        config_dir.join("validation"),
    ];

    let mut found_configs = false;

    for dir in search_dirs {
        if !dir.exists() {
            continue;
        }

        scan_directory_for_transforms(&dir, &mut found_configs)?;
    }

    if !found_configs {
        println!("{}", "No transformable configs found.".dimmed());
        println!(
            "{}",
            "Transformable configs have [meta] section with implementation_stage field.".dimmed()
        );
    }

    Ok(())
}

fn scan_directory_for_transforms(dir: &Path, found_any: &mut bool) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            // Try to load as transformable config
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                    if let Some(meta) = value.get("meta") {
                        if let Ok(route) = ImplementationRoute::from_toml_meta(meta) {
                            let config_name = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown");

                            println!(
                                "{} {} {}",
                                route.stage.status_icon(),
                                config_name.cyan().bold(),
                                format!("({:?})", route.stage).color(route.stage.color())
                            );
                            println!("  Path: {}", path.display().to_string().dimmed());
                            println!("  Language: {}", format!("{:?}", route.language).dimmed());

                            if let Some(prod) = &route.production_route {
                                println!(
                                    "  Production: {} {}",
                                    if prod.ready { "✅" } else { "⏳" },
                                    format!("{:?}", prod.language).dimmed()
                                );
                            }

                            println!();
                            *found_any = true;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Expand ~ and environment variables in path
fn expand_path(path: &str) -> Result<std::path::PathBuf> {
    if path.starts_with('~') {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(&path[2..]))
    } else {
        Ok(std::path::PathBuf::from(
            shellexpand::env(path)?.to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_path() {
        let path = expand_path("~/.config/test").unwrap();
        assert!(path.to_string_lossy().contains(".config/test"));
        assert!(!path.to_string_lossy().contains('~'));
    }
}
