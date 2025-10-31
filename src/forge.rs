use anyhow::{Context, Result};
use chrono::Utc;
use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

use crate::paths::NabiPaths;

/// Feature flag configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeConfig {
    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default)]
    pub features: HashMap<String, FeatureFlag>,

    #[serde(default = "Utc::now")]
    pub last_modified: chrono::DateTime<Utc>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// Individual feature flag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlag {
    pub enabled: bool,
    pub description: Option<String>,
    pub env_var: Option<String>,
    pub modified_at: chrono::DateTime<Utc>,
    pub modified_by: Option<String>,
}

impl Default for ForgeConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            features: HashMap::new(),
            last_modified: Utc::now(),
        }
    }
}

impl ForgeConfig {
    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = NabiPaths::config_dir()?
            .join("config");

        // Ensure directory exists
        fs::create_dir_all(&config_dir)
            .context("Failed to create config directory")?;

        Ok(config_dir.join("forge.yaml"))
    }

    /// Load config from disk
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            // Create default config if it doesn't exist
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let contents = fs::read_to_string(&path)
            .context("Failed to read forge config")?;

        let config: Self = serde_yaml::from_str(&contents)
            .context("Failed to parse forge config")?;

        Ok(config)
    }

    /// Save config to disk atomically
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        // Create temp file in same directory for atomic rename
        let temp_file = NamedTempFile::new_in(path.parent().unwrap())
            .context("Failed to create temp file")?;

        // Write YAML content
        let yaml = serde_yaml::to_string(&self)
            .context("Failed to serialize config")?;

        temp_file.as_file()
            .write_all(yaml.as_bytes())
            .context("Failed to write to temp file")?;

        // Atomic rename
        temp_file.persist(&path)
            .context("Failed to persist config file")?;

        Ok(())
    }

    /// Enable a feature
    pub fn enable_feature(&mut self, name: &str, description: Option<String>) -> Result<()> {
        let env_var = format!("NABI_FORGE_{}", name.to_uppercase().replace('-', "_"));

        self.features.insert(
            name.to_string(),
            FeatureFlag {
                enabled: true,
                description,
                env_var: Some(env_var.clone()),
                modified_at: Utc::now(),
                modified_by: std::env::var("USER").ok(),
            },
        );

        self.last_modified = Utc::now();
        self.save()?;

        // Export environment variable
        std::env::set_var(&env_var, "1");

        Ok(())
    }

    /// Disable a feature
    pub fn disable_feature(&mut self, name: &str) -> Result<()> {
        if let Some(feature) = self.features.get_mut(name) {
            feature.enabled = false;
            feature.modified_at = Utc::now();
            feature.modified_by = std::env::var("USER").ok();

            // Unset environment variable
            if let Some(ref env_var) = feature.env_var {
                std::env::remove_var(env_var);
            }
        } else {
            // Add as disabled feature
            let env_var = format!("NABI_FORGE_{}", name.to_uppercase().replace('-', "_"));
            self.features.insert(
                name.to_string(),
                FeatureFlag {
                    enabled: false,
                    description: None,
                    env_var: Some(env_var),
                    modified_at: Utc::now(),
                    modified_by: std::env::var("USER").ok(),
                },
            );
        }

        self.last_modified = Utc::now();
        self.save()?;

        Ok(())
    }

    /// Export all enabled features as environment variables
    pub fn export_env_vars(&self) {
        for (name, feature) in &self.features {
            if feature.enabled {
                let env_var = feature.env_var.clone()
                    .unwrap_or_else(|| format!("NABI_FORGE_{}", name.to_uppercase().replace('-', "_")));
                std::env::set_var(env_var, "1");
            }
        }
    }

    /// Generate shell export commands for enabled features
    pub fn generate_exports(&self) -> String {
        let mut exports = Vec::new();

        for (name, feature) in &self.features {
            if feature.enabled {
                let env_var = feature.env_var.clone()
                    .unwrap_or_else(|| format!("NABI_FORGE_{}", name.to_uppercase().replace('-', "_")));
                exports.push(format!("export {}=1", env_var));
            }
        }

        exports.join("\n")
    }
}

/// Handle forge enable command
pub fn handle_enable(feature: String) -> Result<()> {
    let mut config = ForgeConfig::load()?;

    println!("{}", format!("🔥 Enabling feature: {}", feature).cyan().bold());

    config.enable_feature(&feature, None)?;

    println!("{}", format!("✓ Feature '{}' enabled", feature).green().bold());
    println!("{}", format!("  Environment variable: NABI_FORGE_{}",
        feature.to_uppercase().replace('-', "_")).dimmed());

    // Show export command for user
    println!("\n{}", "To export in current shell:".yellow());
    println!("  export NABI_FORGE_{}=1", feature.to_uppercase().replace('-', "_"));

    Ok(())
}

/// Handle forge disable command
pub fn handle_disable(feature: String) -> Result<()> {
    let mut config = ForgeConfig::load()?;

    println!("{}", format!("🔥 Disabling feature: {}", feature).cyan().bold());

    config.disable_feature(&feature)?;

    println!("{}", format!("✓ Feature '{}' disabled", feature).green().bold());

    Ok(())
}

/// Handle forge status command
pub fn handle_status() -> Result<()> {
    let config = ForgeConfig::load()?;

    println!("{}", "🔥 Forge Feature Flags Status".cyan().bold());
    println!("{}", "─".repeat(50).dimmed());

    if config.features.is_empty() {
        println!("{}", "No features configured".yellow());
        return Ok(());
    }

    // Group by enabled/disabled
    let mut enabled = Vec::new();
    let mut disabled = Vec::new();

    for (name, feature) in &config.features {
        if feature.enabled {
            enabled.push((name, feature));
        } else {
            disabled.push((name, feature));
        }
    }

    // Show enabled features
    if !enabled.is_empty() {
        println!("\n{}", "Enabled Features:".green().bold());
        for (name, feature) in enabled {
            println!("  {} {}", "✓".green(), name.green());
            if let Some(ref desc) = feature.description {
                println!("    {}", desc.dimmed());
            }
            if let Some(ref env) = feature.env_var {
                println!("    → {}", env.dimmed());
            }
        }
    }

    // Show disabled features
    if !disabled.is_empty() {
        println!("\n{}", "Disabled Features:".red().bold());
        for (name, feature) in disabled {
            println!("  {} {}", "✗".red(), name.red());
            if let Some(ref desc) = feature.description {
                println!("    {}", desc.dimmed());
            }
        }
    }

    println!("\n{}", format!("Last modified: {}",
        config.last_modified.format("%Y-%m-%d %H:%M:%S UTC")).dimmed());

    Ok(())
}

/// Handle forge list command
pub fn handle_list() -> Result<()> {
    println!("{}", "🔥 Available Forge Features".cyan().bold());
    println!("{}", "─".repeat(50).dimmed());

    // Define available features with descriptions
    let features = vec![
        ("test-mode", "Enable test mode for development"),
        ("debug-logging", "Enable verbose debug logging"),
        ("federation-sync", "Enable automatic federation synchronization"),
        ("hook-system", "Enable Claude Code hook system"),
        ("vigil-monitoring", "Enable Vigil context injection"),
        ("experimental", "Enable experimental features"),
        ("performance-metrics", "Enable performance metric collection"),
        ("parallel-execution", "Enable parallel agent execution"),
        ("smart-routing", "Enable intelligent command routing"),
        ("context-injection", "Enable automatic context injection"),
    ];

    for (name, description) in features {
        println!("\n  {} {}", "•".cyan(), name.bold());
        println!("    {}", description.dimmed());
        println!("    → NABI_FORGE_{}", name.to_uppercase().replace('-', "_").dimmed());
    }

    println!("\n{}", "Usage:".yellow());
    println!("  nabi forge enable <feature>   # Enable a feature");
    println!("  nabi forge disable <feature>  # Disable a feature");
    println!("  nabi forge status            # Show current status");

    Ok(())
}