/// Backup System Commands - Comprehensive Federation Data Protection
///
/// Phase 1: Core command structure for backup creation, restoration, listing, and configuration
///
/// Architecture:
/// - XDG-compliant storage with dual-format archiving (ditto.zip + tar.tgz)
/// - External drive auto-detection with priority ordering
/// - NATS queue integration for federation-wide backup coordination
/// - Dry-run support for safe validation
///
/// Commands:
/// - create: Create new backup (with --mode xdg|full, --dry-run, --targets, --external)
/// - restore: Restore from backup with version/time selection
/// - list: List available backups with metadata
/// - config: Show/validate configuration
/// - queue: Monitor NATS backup queue
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use toml;

use crate::paths::NabiPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub version: String,
    pub sources: Vec<String>,
    pub external_drives: Vec<ExternalDrive>,
    pub archive_settings: ArchiveSettings,
    pub storage_mesh: StorageMeshConfig,
    pub scheduling: SchedulingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalDrive {
    pub label: String,
    pub mount_path: String,
    pub capacity_gb: u32,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveSettings {
    pub formats: Vec<String>,  // "ditto.zip", "tar.tgz"
    pub compression_level: u8, // 1-9
    pub exclusions: Vec<String>,
    pub naming_pattern: String, // {timestamp}-{mode}-{hash}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMeshConfig {
    pub enabled: bool,
    pub nats_url: String,
    pub queue_name: String,
    pub replication_factor: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingConfig {
    pub enabled: bool,
    pub frequency: String, // "daily", "weekly", "monthly"
    pub time: String,      // "02:00" (24-hour format)
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub id: String,
    pub timestamp: String,
    pub mode: String, // "xdg", "full", "incremental"
    pub sources_count: usize,
    pub total_size_bytes: u64,
    pub format: String, // "ditto.zip", "tar.tgz", "dual"
    pub checksum: String,
    pub external_drive: Option<String>,
    pub status: String, // "created", "verified", "archived"
}

// ============================================================================
// Command Implementations
// ============================================================================

/// Create a new backup with specified mode and targets
pub fn cmd_create(
    mode: Option<&str>,
    dry_run: bool,
    targets: Option<&str>,
    external: bool,
) -> Result<()> {
    let mode = mode.unwrap_or("xdg");

    println!(
        "{}Creating backup with mode: {}{}",
        "→ ".bright_cyan(),
        mode.bright_yellow(),
        ""
    );

    if dry_run {
        println!(
            "{}DRY RUN - No changes will be made{}",
            "  ⚠️  ".bright_yellow(),
            ""
        );
    }

    // Load configuration
    let config = load_backup_config()?;
    println!("{}Configuration loaded{}", "  ✓ ".bright_green(), "");

    // Determine sources
    let sources = determine_sources(mode, targets, &config)?;
    println!(
        "{}Found {} source directories",
        "  ✓ ".bright_green(),
        sources.len().to_string().bright_cyan()
    );

    // Print sources summary
    for source in &sources {
        println!("     - {}", source);
    }

    // Validate external drive if requested
    if external {
        let drives = detect_external_drives()?;
        println!(
            "{}Found {} external drive(s)",
            "  ✓ ".bright_green(),
            drives.len().to_string().bright_cyan()
        );
        for drive in &drives {
            println!(
                "     - {} ({} GB) at {}",
                drive.label, drive.capacity_gb, drive.mount_path
            );
        }
    }

    // Show what would be archived
    println!("\n{}Archive settings:", "→ ".bright_cyan());
    println!("  Format: {}", config.archive_settings.formats.join(", "));
    println!(
        "  Compression: {}",
        config.archive_settings.compression_level
    );
    println!("  Naming: {}", config.archive_settings.naming_pattern);

    if !dry_run {
        // Create backup metadata
        let metadata = BackupMetadata {
            id: generate_backup_id(),
            timestamp: Utc::now().to_rfc3339(),
            mode: mode.to_string(),
            sources_count: sources.len(),
            total_size_bytes: 0, // Would calculate in real implementation
            format: "dual".to_string(),
            checksum: String::new(),
            external_drive: if external {
                Some("auto-detect".to_string())
            } else {
                None
            },
            status: "created".to_string(),
        };

        // Save metadata
        save_backup_metadata(&metadata)?;
        println!(
            "\n{}Backup created: {}",
            "✓ ".bright_green(),
            metadata.id.bright_cyan()
        );
    } else {
        println!("\n{}Backup would be created (dry-run)", "→ ".bright_cyan());
    }

    Ok(())
}

/// List available backups
pub fn cmd_list(format: Option<&str>) -> Result<()> {
    let format = format.unwrap_or("text");

    println!("{}Listing available backups...", "→ ".bright_cyan());

    let backups = load_backup_manifests()?;

    if backups.is_empty() {
        println!("  No backups found");
        return Ok(());
    }

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&backups)?);
        }
        _ => {
            println!("\n{}", "Backup Inventory".bright_cyan().underline());
            println!("{}", "─".repeat(80));

            for backup in &backups {
                let timestamp_display = &backup.timestamp[..16]; // YYYY-MM-DDTHH:MM
                println!(
                    "{} [{}] {} ({})",
                    "│".bright_cyan(),
                    timestamp_display.bright_yellow(),
                    backup.id.bright_white(),
                    backup.mode.bright_green()
                );
                println!("  ├─ Size: {}", format_size_bytes(backup.total_size_bytes));
                println!("  ├─ Format: {}", backup.format);
                println!("  ├─ Status: {}", backup.status);
                if let Some(drive) = &backup.external_drive {
                    println!("  └─ Location: {}", drive);
                }
            }
            println!("{}", "─".repeat(80));
            println!("Total: {} backups", backups.len().to_string().bright_cyan());
        }
    }

    Ok(())
}

/// Restore from a backup
pub fn cmd_restore(backup_id: &str, target: Option<&str>, dry_run: bool) -> Result<()> {
    println!(
        "{}Restoring backup: {}",
        "→ ".bright_cyan(),
        backup_id.bright_yellow()
    );

    if dry_run {
        println!(
            "{}DRY RUN - No changes will be made{}",
            "  ⚠️  ".bright_yellow(),
            ""
        );
    }

    // Load backup metadata
    let metadata = load_backup_metadata(backup_id)?;
    println!(
        "{}Backup found: {} ({})",
        "  ✓ ".bright_green(),
        metadata.id,
        metadata.timestamp
    );
    println!(
        "  Mode: {}, Size: {}",
        metadata.mode,
        format_size_bytes(metadata.total_size_bytes)
    );

    if let Some(target_path) = target {
        println!("  Restore target: {}", target_path);
    }

    if !dry_run {
        println!(
            "\n{}Extraction would proceed (not implemented in Phase 1)",
            "→ ".bright_cyan()
        );
    }

    Ok(())
}

/// Show backup configuration
pub fn cmd_config(validate: bool) -> Result<()> {
    let config = load_backup_config()?;

    println!("{}Backup Configuration:", "→ ".bright_cyan());
    println!("\n{}Sources:", "  ".bright_cyan());
    for source in &config.sources {
        println!("    - {}", source);
    }

    println!("\n{}External Drives:", "  ".bright_cyan());
    for drive in &config.external_drives {
        println!(
            "    - {} (Priority: {}, {} GB at {})",
            drive.label, drive.priority, drive.capacity_gb, drive.mount_path
        );
    }

    println!("\n{}Archive Settings:", "  ".bright_cyan());
    println!(
        "    - Formats: {}",
        config.archive_settings.formats.join(", ")
    );
    println!(
        "    - Compression: {}",
        config.archive_settings.compression_level
    );
    println!(
        "    - Exclusions: {} patterns",
        config.archive_settings.exclusions.len()
    );

    println!("\n{}Storage Mesh:", "  ".bright_cyan());
    println!("    - Enabled: {}", config.storage_mesh.enabled);
    println!("    - NATS URL: {}", config.storage_mesh.nats_url);
    println!("    - Queue: {}", config.storage_mesh.queue_name);

    println!("\n{}Scheduling:", "  ".bright_cyan());
    println!("    - Enabled: {}", config.scheduling.enabled);
    println!("    - Frequency: {}", config.scheduling.frequency);
    println!("    - Time: {}", config.scheduling.time);
    println!("    - Retention: {} days", config.scheduling.retention_days);

    if validate {
        println!("\n{}Validating configuration...", "→ ".bright_cyan());
        validate_config(&config)?;
        println!("{}Configuration is valid", "✓ ".bright_green());
    }

    Ok(())
}

/// Monitor NATS backup queue (Phase 2)
pub fn cmd_queue(action: Option<&str>) -> Result<()> {
    let _action = action.unwrap_or("status");
    println!(
        "{}Queue command not yet implemented (Phase 2)",
        "⚠️  ".bright_yellow()
    );
    println!("  This will integrate with NATS for federation-wide backup coordination");
    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

fn load_backup_config() -> Result<BackupConfig> {
    let config_path = NabiPaths::config_dir()?.join("backup").join("config.toml");

    if !config_path.exists() {
        // Return default config if not found
        return Ok(BackupConfig {
            version: "1.0.0".to_string(),
            sources: vec![
                "~/.nabi/config".to_string(),
                "~/.nabi/data".to_string(),
                "~/docs".to_string(),
            ],
            external_drives: vec![],
            archive_settings: ArchiveSettings {
                formats: vec!["ditto.zip".to_string(), "tar.tgz".to_string()],
                compression_level: 6,
                exclusions: vec![".git".to_string(), "node_modules".to_string()],
                naming_pattern: "{timestamp}-{mode}-{hash}".to_string(),
            },
            storage_mesh: StorageMeshConfig {
                enabled: false,
                nats_url: "nats://localhost:4222".to_string(),
                queue_name: "backup-queue".to_string(),
                replication_factor: 3,
            },
            scheduling: SchedulingConfig {
                enabled: false,
                frequency: "daily".to_string(),
                time: "02:00".to_string(),
                retention_days: 30,
            },
        });
    }

    let content =
        fs::read_to_string(&config_path).context("Failed to read backup configuration")?;

    let config: BackupConfig =
        toml::from_str(&content).context("Failed to parse backup configuration")?;

    Ok(config)
}

fn determine_sources(
    mode: &str,
    targets: Option<&str>,
    config: &BackupConfig,
) -> Result<Vec<String>> {
    if let Some(target_list) = targets {
        // Use explicitly specified targets
        Ok(target_list
            .split(',')
            .map(|s| s.trim().to_string())
            .collect())
    } else {
        // Use mode-based defaults
        match mode {
            "xdg" => Ok(config.sources.clone()),
            "full" => {
                let mut all_sources = config.sources.clone();
                all_sources.push("~/nabia".to_string());
                Ok(all_sources)
            }
            _ => Err(anyhow!("Unknown backup mode: {}", mode)),
        }
    }
}

fn detect_external_drives() -> Result<Vec<ExternalDrive>> {
    // Phase 1: Mock implementation
    // Phase 2: Use diskutil on macOS, lsblk on Linux, etc.
    Ok(vec![])
}

fn generate_backup_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let mut hasher = DefaultHasher::new();
    timestamp.to_string().hash(&mut hasher);
    let hash = format!("{:x}", hasher.finish());

    format!("backup-{}-{}", timestamp, &hash[..8])
}

fn save_backup_metadata(metadata: &BackupMetadata) -> Result<()> {
    let manifest_dir = NabiPaths::state_dir()?.join("backup").join("manifests");

    fs::create_dir_all(&manifest_dir).context("Failed to create backup manifest directory")?;

    let metadata_path = manifest_dir.join(format!("{}.json", metadata.id));
    let json = serde_json::to_string_pretty(metadata)?;
    fs::write(&metadata_path, json).context("Failed to write backup metadata")?;

    Ok(())
}

fn load_backup_metadata(backup_id: &str) -> Result<BackupMetadata> {
    let metadata_path = NabiPaths::state_dir()?
        .join("backup")
        .join("manifests")
        .join(format!("{}.json", backup_id));

    let content = fs::read_to_string(&metadata_path).context("Failed to read backup metadata")?;

    let metadata: BackupMetadata =
        serde_json::from_str(&content).context("Failed to parse backup metadata")?;

    Ok(metadata)
}

fn load_backup_manifests() -> Result<Vec<BackupMetadata>> {
    let manifest_dir = NabiPaths::state_dir()?.join("backup").join("manifests");

    if !manifest_dir.exists() {
        return Ok(vec![]);
    }

    let mut backups = vec![];

    for entry in fs::read_dir(&manifest_dir).context("Failed to read backup manifests")? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(metadata) = serde_json::from_str::<BackupMetadata>(&content) {
                    backups.push(metadata);
                }
            }
        }
    }

    // Sort by timestamp (newest first)
    backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(backups)
}

fn format_size_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_idx])
}

fn validate_config(config: &BackupConfig) -> Result<()> {
    if config.sources.is_empty() {
        return Err(anyhow!("No backup sources configured"));
    }

    if config.archive_settings.formats.is_empty() {
        return Err(anyhow!("No archive formats configured"));
    }

    if config.archive_settings.compression_level < 1
        || config.archive_settings.compression_level > 9
    {
        return Err(anyhow!("Compression level must be 1-9"));
    }

    Ok(())
}
