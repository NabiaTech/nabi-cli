//! Configuration transformation commands
//!
//! Implements the CLI interface for the transformation pipeline:
//! - `nabi validate <category>` - Validate TOML files against schemas
//! - `nabi services rebuild [name]` - Transform services to JSON
//! - `nabi services generate <name>` - Generate files from templates
//! - `nabi platforms rebuild [name]` - Transform platforms to generated files

use crate::paths::NabiPaths;
use crate::transform::generative::GenerativeConfig;
use crate::transform::structural::StructuralConfig;
use anyhow::{Context, Result};
use colored::*;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

/// Computes SHA256 hash of content
fn compute_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    format!("sha256:{}", hex::encode(result))
}

#[derive(Debug, Deserialize)]
struct TransformSettings {
    #[serde(default)]
    vector_clock: VectorClockSettings,
    #[serde(default)]
    event_bus: EventBusSettings,
}

impl TransformSettings {
    fn load() -> Result<Self> {
        let config_path = NabiPaths::config_dir()?
            .join("transform")
            .join("config.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path).with_context(|| {
            format!("Failed to read transform config: {}", config_path.display())
        })?;
        let settings: Self = toml::from_str(&content).with_context(|| {
            format!(
                "Failed to parse transform config: {}",
                config_path.display()
            )
        })?;
        Ok(settings)
    }
}

impl Default for TransformSettings {
    fn default() -> Self {
        Self {
            vector_clock: VectorClockSettings::default(),
            event_bus: EventBusSettings::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct VectorClockSettings {
    #[serde(default = "default_vector_clock_enabled")]
    enabled: bool,
    #[serde(default)]
    path: Option<String>,
}

impl VectorClockSettings {
    fn resolved_path(&self) -> Result<PathBuf> {
        if let Some(ref custom) = self.path {
            crate::transform::expand_path(custom)
        } else {
            Ok(NabiPaths::state_dir()?.join("transform-vector-clock.json"))
        }
    }
}

impl Default for VectorClockSettings {
    fn default() -> Self {
        Self {
            enabled: default_vector_clock_enabled(),
            path: None,
        }
    }
}

fn default_vector_clock_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct EventBusSettings {
    #[serde(default = "default_event_bus_enabled")]
    enabled: bool,
    #[serde(default = "default_event_source")]
    source: String,
    #[serde(default = "default_event_severity")]
    severity: String,
}

impl Default for EventBusSettings {
    fn default() -> Self {
        Self {
            enabled: default_event_bus_enabled(),
            source: default_event_source(),
            severity: default_event_severity(),
        }
    }
}

fn default_event_bus_enabled() -> bool {
    true
}

fn default_event_source() -> String {
    "transform-pipeline".to_string()
}

fn default_event_severity() -> String {
    "info".to_string()
}

/// Gets the local node ID for vector clock tracking
fn get_node_id() -> Result<String> {
    let platform = get_current_platform();
    let hostname = hostname::get()
        .context("Failed to get hostname")?
        .to_string_lossy()
        .to_string();

    // Format: platform-hostname (e.g., "darwin-tylers-mbp")
    Ok(format!(
        "{}-{}",
        platform,
        hostname.to_lowercase().replace(' ', "-")
    ))
}

/// Loads the vector clock from state, increments local node counter, and saves
fn increment_vector_clock(settings: &TransformSettings) -> Result<HashMap<String, u64>> {
    if !settings.vector_clock.enabled {
        return Ok(HashMap::new());
    }

    let vector_clock_file = settings.vector_clock.resolved_path()?;

    // Load existing vector clock or create new one
    let mut vector_clock: HashMap<String, u64> = if vector_clock_file.exists() {
        let content =
            fs::read_to_string(&vector_clock_file).context("Failed to read vector clock state")?;
        serde_json::from_str(&content).unwrap_or_else(|_| HashMap::new())
    } else {
        HashMap::new()
    };

    // Increment local node's counter
    let node_id = get_node_id()?;
    let counter = vector_clock.entry(node_id).or_insert(0);
    *counter += 1;

    // Save updated vector clock
    let content = serde_json::to_string_pretty(&vector_clock)?;
    fs::write(&vector_clock_file, content).context("Failed to save vector clock state")?;

    Ok(vector_clock)
}

/// Publishes a ConfigTransformRecord event to the Aether federation bus
fn publish_config_transform_event(
    category: &str,
    name: &str,
    toml_content: &str,
    json_content: &str,
    output_path: &Path,
    settings: &TransformSettings,
) -> Result<()> {
    if !settings.event_bus.enabled {
        return Ok(());
    }

    let toml_hash = compute_sha256(toml_content);
    let json_hash = compute_sha256(json_content);

    // Extract version from TOML if available, otherwise use default
    let version = extract_version_from_toml(toml_content).unwrap_or_else(|| "0.0.0".to_string());

    // Increment vector clock for this transform
    let vector_clock = increment_vector_clock(settings)?;
    let node_id = get_node_id()?;

    // Build metadata for ConfigTransformRecord
    let metadata = json!({
        "category": category,
        "name": name,
        "version": version,
        "previous_version": "0.0.0", // TODO: track previous version from state
        "change_type": "minor", // TODO: compute from diffs
        "toml_hash": toml_hash,
        "json_hash": json_hash,
        "platform": get_current_platform(),
        "syncthing_file_id": output_path.to_string_lossy().to_string(),
        "vector_clock": vector_clock,
        "node_id": node_id,
    });

    // Publish to Aether event bus
    let metadata_json = serde_json::to_string(&metadata)?;
    let message = format!("ConfigTransform: {} → {} ({})", category, name, version);
    let output = Command::new("nabi")
        .arg("aether")
        .arg("events")
        .arg("publish")
        .arg("--source")
        .arg(&settings.event_bus.source)
        .arg("--severity")
        .arg(&settings.event_bus.severity)
        .arg("--message")
        .arg(&message)
        .arg("--metadata")
        .arg(&metadata_json)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to publish ConfigTransformRecord: {}",
            stderr
        ));
    }

    Ok(())
}

/// Extracts version from TOML [meta] section
fn extract_version_from_toml(content: &str) -> Option<String> {
    let table: toml::Table = toml::from_str(content).ok()?;
    table
        .get("meta")
        .and_then(|meta| meta.get("version"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Gets current platform identifier
fn get_current_platform() -> &'static str {
    #[cfg(target_os = "macos")]
    return "darwin";
    #[cfg(target_os = "linux")]
    return "linux";
    #[cfg(target_os = "windows")]
    return "windows";
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return "unknown";
}

/// Validates TOML files in a category directory against their declared schemas
pub fn validate_category(category: &str) -> Result<()> {
    let config_dir = NabiPaths::config_dir()?;
    let category_dir = config_dir.join(category);

    if !category_dir.exists() {
        return Err(anyhow::anyhow!(
            "Category directory not found: {}",
            category_dir.display()
        ));
    }

    println!("Validating {} configurations...\n", category.bold());

    let mut valid_count = 0;
    let mut error_count = 0;

    for entry in WalkDir::new(&category_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "toml"))
    {
        let file_path = entry.path();
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<unknown>");

        match validate_toml_file(file_path) {
            Ok(_) => {
                println!("  {} {}", "✓".green(), file_name);
                valid_count += 1;
            }
            Err(e) => {
                println!("  {} {} - {}", "✗".red(), file_name, e.to_string().red());
                error_count += 1;
            }
        }
    }

    println!("\nValidation Summary:");
    println!(
        "  {} Valid: {}, {} Errors: {}",
        "✓".green(),
        valid_count.to_string().green(),
        if error_count > 0 { "✗" } else { "✓" },
        if error_count > 0 {
            error_count.to_string().red()
        } else {
            error_count.to_string().green()
        }
    );

    if error_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Validates a single TOML file
fn validate_toml_file(path: &Path) -> Result<()> {
    use crate::transform::schema::validate_metadata;

    let content = fs::read_to_string(path)?;
    let _data: toml::Table = toml::from_str(&content)?;

    // Convert TOML to JSON for metadata validation
    let json_value = serde_json::to_value(&_data)?;
    validate_metadata(&json_value, path).map_err(|e| anyhow::anyhow!("{}", e.to_string()))
}

/// Rebuilds structural transformations (TOML → JSON) for a category
pub fn rebuild_structural(category: &str, name: Option<&str>) -> Result<()> {
    let config_dir = NabiPaths::config_dir()?;
    let category_dir = config_dir.join(category);
    let transform_settings = TransformSettings::load()?;

    if !category_dir.exists() {
        return Err(anyhow::anyhow!(
            "Category directory not found: {}",
            category_dir.display()
        ));
    }

    println!("Rebuilding {} (TOML → JSON)...\n", category.bold());

    let mut success_count = 0;
    let mut error_count = 0;

    // Get files to process
    let files: Vec<_> = if let Some(n) = name {
        vec![category_dir.join(format!("{}.toml", n))]
    } else {
        WalkDir::new(&category_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "toml"))
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    for file_path in files {
        if !file_path.exists() {
            eprintln!("  {} File not found: {}", "✗".red(), file_path.display());
            error_count += 1;
            continue;
        }

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<unknown>");

        match StructuralConfig::load(&file_path) {
            Ok(config) => match config.transform() {
                Ok((json_output, output_path)) => {
                    // Extract category and name from file path
                    let category = file_path
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");

                    let name = file_path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");

                    // Read original TOML content
                    let toml_content =
                        fs::read_to_string(&file_path).unwrap_or_else(|_| String::new());

                    println!(
                        "  {} {} → {}",
                        "✓".green(),
                        file_name,
                        output_path.display().to_string().cyan()
                    );

                    // Publish ConfigTransformRecord to Aether federation bus
                    if let Err(e) = publish_config_transform_event(
                        category,
                        name,
                        &toml_content,
                        &json_output,
                        &output_path,
                        &transform_settings,
                    ) {
                        println!(
                            "  {} {} (publish warning) - {}",
                            "⚠".yellow(),
                            file_name,
                            e.to_string().yellow()
                        );
                    }

                    success_count += 1;
                }
                Err(e) => {
                    println!(
                        "  {} {} (transform) - {}",
                        "✗".red(),
                        file_name,
                        e.to_string().red()
                    );
                    error_count += 1;
                }
            },
            Err(e) => {
                println!(
                    "  {} {} (load) - {}",
                    "✗".red(),
                    file_name,
                    e.to_string().red()
                );
                error_count += 1;
            }
        }
    }

    println!("\nTransformation Summary:");
    println!(
        "  {} Success: {}, {} Errors: {}",
        "✓".green(),
        success_count.to_string().green(),
        if error_count > 0 { "✗" } else { "✓" },
        if error_count > 0 {
            error_count.to_string().red()
        } else {
            error_count.to_string().green()
        }
    );

    if error_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Generates files from templates (TOML + template → output)
pub fn generate_from_template(category: &str, name: &str) -> Result<()> {
    let config_dir = NabiPaths::config_dir()?;
    let config_path = config_dir.join(category).join(format!("{}.toml", name));

    if !config_path.exists() {
        return Err(anyhow::anyhow!(
            "Configuration not found: {}",
            config_path.display()
        ));
    }

    println!(
        "Generating {} from template (TOML + Jinja2 → output)...\n",
        name.bold()
    );

    match GenerativeConfig::load(&config_path) {
        Ok(config) => match config.transform() {
            Ok((output, output_path)) => {
                println!(
                    "  {} {} → {}",
                    "✓".green(),
                    name,
                    output_path.display().to_string().cyan()
                );
                println!(
                    "\nGenerated {} bytes of output",
                    output.len().to_string().cyan()
                );
                Ok(())
            }
            Err(e) => {
                eprintln!("  {} Generation failed: {}", "✗".red(), e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("  {} Load failed: {}", "✗".red(), e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_validate_toml_file() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
[meta]
transformation_type = "structural"
schema_version = "1.0.0"
validators = ["service.schema.json"]
consumers = ["launcher"]

[service]
id = "test"
"#;
        file.write_all(content.as_bytes()).unwrap();

        let result = validate_toml_file(file.path());
        assert!(result.is_ok());
    }
}
