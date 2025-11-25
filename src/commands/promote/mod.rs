// Tool Promotion Module
// Handles promotion of tools from source to deployed locations
// Supports three modes: LIVE (symlink), STABLE (versioned copy), INSTALL (wrapper)

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use colored::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use toml;

mod executor;
mod record;
mod validation;

pub use executor::{
    execute_promoted_tool, get_promoted_executable, get_promotion_record, is_tool_promoted,
};
pub use record::{write_promotion_record, PromotionRecord};
pub use validation::validate_promotion_config;

/// Promotion modes supported by the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PromotionMode {
    /// Live symlink - immediate updates, no versioning
    Live,
    /// Stable versioned copy - immutable, rollback-safe
    Stable,
    /// Install wrapper script - system command integration
    Install,
}

/// Promotion configuration from tool TOML
#[derive(Debug, Deserialize)]
pub struct PromotionConfig {
    pub mode: PromotionMode,
    pub version: String,
    pub description: Option<String>,
    pub artifacts: toml::map::Map<String, toml::Value>,
    pub source: Option<SourceInfo>,
    pub metadata: Option<toml::map::Map<String, toml::Value>>,
    pub version_aliases: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SourceInfo {
    pub repository: String,
    pub branch: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArtifactConfig {
    pub source: String,
    pub target: String,
    pub artifact_type: Option<String>,
}

/// Main promotion entry point
pub fn promote_tool(
    tool_id: &str,
    version_override: Option<&str>,
    mode_override: Option<PromotionMode>,
) -> Result<PromotionRecord> {
    println!("{} Promoting tool: {}", "▶".cyan(), tool_id.bold());

    // 1. Load tool configuration
    let config_path = expand_path(&format!("~/.config/nabi/tools/{}.toml", tool_id));
    if !config_path.exists() {
        return Err(anyhow!("Tool config not found: {}", config_path.display()));
    }

    let tool_config: toml::Value = {
        let content = fs::read_to_string(&config_path)
            .context("Failed to read tool config")?;
        toml::from_str(&content)
            .context("Failed to parse tool config")?
    };

    // 2. Extract promotion config
    let promotion_config: PromotionConfig = tool_config
        .get("promotion")
        .ok_or_else(|| anyhow!("No [promotion] section in tool config"))?
        .clone()
        .try_into()
        .context("Failed to parse promotion config")?;

    // 3. Determine effective version and mode
    let version = version_override.unwrap_or(&promotion_config.version);
    let mode = mode_override.unwrap_or(promotion_config.mode.clone());

    println!("  {} Version: {}", "→".green(), version.yellow());
    println!("  {} Mode: {:?}", "→".green(), mode);

    // 4. Validate promotion config
    validate_promotion_config(&promotion_config)?;

    // 5. Execute promotion based on mode
    let promotion_record = match mode {
        PromotionMode::Live => promote_live(tool_id, &promotion_config)?,
        PromotionMode::Stable => promote_stable(tool_id, &promotion_config, version)?,
        PromotionMode::Install => promote_install(tool_id, &promotion_config)?,
    };

    // 6. Write promotion record
    let record_id = write_promotion_record(tool_id, &promotion_record)?;
    println!("  {} Record: {}", "→".green(), record_id.bright_blue());

    // 7. Emit federation event (TODO)
    // emit_federation_event("tool.promoted", &promotion_record)?;

    println!("{} Promotion complete!", "✓".green().bold());
    Ok(promotion_record)
}

/// Promote in LIVE mode (symlinks)
fn promote_live(tool_id: &str, config: &PromotionConfig) -> Result<PromotionRecord> {
    println!("  {} Creating symlinks...", "→".cyan());

    let mut artifacts = Vec::new();

    for (artifact_name, artifact_value) in &config.artifacts {
        let artifact_config: ArtifactConfig = artifact_value.clone().try_into()
            .context(format!("Invalid artifact config: {}", artifact_name))?;

        let source = expand_path(&artifact_config.source);
        let target = expand_path(&artifact_config.target);

        // Verify source exists
        if !source.exists() {
            return Err(anyhow!("Source not found: {}", source.display()));
        }

        // Create parent directory
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create target directory")?;
        }

        // Remove existing symlink/file
        if target.exists() || target.is_symlink() {
            fs::remove_file(&target).ok();
        }

        // Create symlink
        symlink(&source, &target)
            .context(format!("Failed to create symlink: {} -> {}",
                           source.display(), target.display()))?;

        let checksum = compute_checksum(&source)?;

        artifacts.push(record::ArtifactRecord {
            artifact_type: artifact_config.artifact_type
                .unwrap_or_else(|| artifact_name.clone()),
            source_path: artifact_config.source.clone(),
            target_path: artifact_config.target.clone(),
            checksum,
            size_bytes: None,
            permissions: None,
        });

        println!("    {} {}: {} → {}",
                 "✓".green(),
                 artifact_name.bold(),
                 source.display().to_string().dimmed(),
                 target.display());
    }

    // Get git info
    let source_info = get_git_info(&config.source)?;

    Ok(PromotionRecord {
        record_id: String::new(), // Will be set by write_promotion_record
        tool_id: tool_id.to_string(),
        version: "live".to_string(),
        promoted_at: Utc::now().to_rfc3339(),
        mode: PromotionMode::Live,
        artifacts,
        source: source_info,
        version_aliases: None,
        metadata: None,
        rollback: None,
        validation: None,
        status: record::PromotionStatus::Active,
        superseded_by: None,
    })
}

/// Promote in STABLE mode (versioned copies)
fn promote_stable(tool_id: &str, config: &PromotionConfig, version: &str) -> Result<PromotionRecord> {
    println!("  {} Copying versioned artifacts...", "→".cyan());

    let mut artifacts = Vec::new();

    for (artifact_name, artifact_value) in &config.artifacts {
        let artifact_config: ArtifactConfig = artifact_value.clone().try_into()
            .context(format!("Invalid artifact config: {}", artifact_name))?;

        let source = expand_path(&artifact_config.source);

        // Inject version into target path
        let target_template = artifact_config.target.replace("{version}", version);
        let target = expand_path(&target_template);

        // Verify source exists
        if !source.exists() {
            return Err(anyhow!("Source not found: {}", source.display()));
        }

        // Create parent directory
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create target directory")?;
        }

        // Copy artifact
        if source.is_dir() {
            copy_dir_recursive(&source, &target)?;
        } else {
            fs::copy(&source, &target)
                .context(format!("Failed to copy: {} -> {}",
                               source.display(), target.display()))?;
        }

        let checksum = compute_checksum(&target)?;
        let size = if target.is_dir() {
            calculate_dir_size(&target)?
        } else {
            fs::metadata(&target)?.len()
        };

        artifacts.push(record::ArtifactRecord {
            artifact_type: artifact_config.artifact_type
                .unwrap_or_else(|| artifact_name.clone()),
            source_path: artifact_config.source.clone(),
            target_path: target_template,
            checksum,
            size_bytes: Some(size),
            permissions: None,
        });

        println!("    {} {}: {} bytes",
                 "✓".green(),
                 artifact_name.bold(),
                 size.to_string().yellow());
    }

    // Create version aliases
    let version_aliases = config.version_aliases.clone()
        .unwrap_or_else(|| vec!["latest".to_string()]);

    for alias in &version_aliases {
        create_version_alias(tool_id, version, alias)?;
        println!("    {} Alias: {} -> {}",
                 "✓".green(),
                 alias.bright_cyan(),
                 version.yellow());
    }

    // Get git info
    let source_info = get_git_info(&config.source)?;

    Ok(PromotionRecord {
        record_id: String::new(),
        tool_id: tool_id.to_string(),
        version: version.to_string(),
        promoted_at: Utc::now().to_rfc3339(),
        mode: PromotionMode::Stable,
        artifacts,
        source: source_info,
        version_aliases: Some(version_aliases),
        metadata: None,
        rollback: None,
        validation: None,
        status: record::PromotionStatus::Active,
        superseded_by: None,
    })
}

/// Promote in INSTALL mode (wrapper scripts)
fn promote_install(tool_id: &str, config: &PromotionConfig) -> Result<PromotionRecord> {
    // TODO: Implement wrapper script generation
    Err(anyhow!("INSTALL mode not yet implemented"))
}

/// Expand ~ and environment variables in paths
fn expand_path(path: &str) -> PathBuf {
    let expanded = shellexpand::tilde(path);
    PathBuf::from(expanded.as_ref())
}

/// Compute SHA-256 checksum of file or directory
fn compute_checksum(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();

    if path.is_dir() {
        // Hash directory contents
        for entry in walkdir::WalkDir::new(path)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let content = fs::read(entry.path())?;
            hasher.update(&content);
        }
    } else {
        // Hash single file
        let content = fs::read(path)?;
        hasher.update(&content);
    }

    let result = hasher.finalize();
    Ok(format!("sha256:{:x}", result))
}

/// Calculate total size of directory
fn calculate_dir_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

/// Copy directory recursively
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Create version alias symlink
fn create_version_alias(tool_id: &str, version: &str, alias: &str) -> Result<()> {
    let lib_dir = expand_path("~/.local/share/nabi/lib");
    let versioned_dir = format!("{}@{}", tool_id, version);
    let alias_link = lib_dir.join(format!("{}@{}", tool_id, alias));

    // Remove existing alias
    if alias_link.exists() || alias_link.is_symlink() {
        fs::remove_file(&alias_link).ok();
    }

    // Create symlink
    symlink(&versioned_dir, &alias_link)?;

    Ok(())
}

/// Get git source information
fn get_git_info(source_config: &Option<SourceInfo>) -> Result<record::SourceRecord> {
    if let Some(source) = source_config {
        let repo_path = expand_path(&source.repository);

        // Try to get git commit (if in git repo)
        let commit = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .arg("rev-parse")
            .arg("--short")
            .arg("HEAD")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.len() >= 7)
            .unwrap_or_else(|| "0000000".to_string()); // Use valid placeholder if no git

        Ok(record::SourceRecord {
            repository: source.repository.clone(),
            commit,
            branch: source.branch.clone(),
            tag: None,
        })
    } else {
        Ok(record::SourceRecord {
            repository: "unknown".to_string(),
            commit: "0000000".to_string(), // Valid placeholder
            branch: None,
            tag: None,
        })
    }
}
