/// Safe XDG state-to-data directory migration
///
/// Migrates directories from XDG_STATE_HOME to XDG_DATA_HOME with:
/// - Path traversal protection (whitelist-based validation)
/// - Conflict detection before overwriting
/// - Dry-run mode for safety
/// - Atomic file operations
/// - Comprehensive error handling

use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use crate::paths::NabiPaths;

/// Migration configuration for a directory
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    /// Source directory name in state (e.g., "memory-backups")
    pub state_dir: String,
    /// Destination directory name in data (e.g., "backups/memory-backups")
    pub data_dir: String,
}

/// Migration result statistics
#[derive(Debug, Default)]
pub struct MigrationStats {
    pub files_moved: usize,
    pub files_failed: usize,
    pub conflicts_detected: usize,
    pub total_size_bytes: u64,
}

/// Migration options
#[derive(Debug, Clone)]
pub struct MigrationOptions {
    /// Dry run mode (preview only, no changes)
    pub dry_run: bool,
    /// Force overwrite on conflicts
    pub force: bool,
    /// Verbose output
    pub verbose: bool,
}

impl Default for MigrationOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            force: false,
            verbose: false,
        }
    }
}

/// Validate directory name to prevent path traversal
fn validate_dir_name(name: &str) -> Result<()> {
    // Reject empty, relative paths, or path traversal attempts
    if name.is_empty() {
        anyhow::bail!("Directory name cannot be empty");
    }

    if name.contains('/') || name.contains('\\') {
        anyhow::bail!("Directory name cannot contain path separators: {}", name);
    }

    if name == "." || name == ".." {
        anyhow::bail!("Directory name cannot be '.' or '..'");
    }

    // Reject any path traversal patterns
    if name.contains("..") {
        anyhow::bail!("Directory name contains path traversal attempt: {}", name);
    }

    Ok(())
}

/// Get canonical path and validate it's within base directory
fn validate_path_within_base(path: &Path, base: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize()
        .context(format!("Failed to canonicalize path: {}", path.display()))?;

    let base_canonical = base.canonicalize()
        .context(format!("Failed to canonicalize base: {}", base.display()))?;

    if !canonical.starts_with(&base_canonical) {
        anyhow::bail!(
            "Path {} is not within base directory {}",
            canonical.display(),
            base_canonical.display()
        );
    }

    Ok(canonical)
}

/// Detect conflicts between source and destination
fn detect_conflicts(source_dir: &Path, dest_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut conflicts = Vec::new();

    if !dest_dir.exists() {
        return Ok(conflicts);
    }

    for entry in WalkDir::new(source_dir).min_depth(1) {
        let entry = entry.context("Failed to read source directory entry")?;

        if entry.file_type().is_file() {
            let relative_path = entry.path().strip_prefix(source_dir)
                .context("Failed to compute relative path")?;

            let dest_file = dest_dir.join(relative_path);
            if dest_file.exists() {
                conflicts.push(dest_file);
            }
        }
    }

    Ok(conflicts)
}

/// Calculate directory size in bytes
fn calculate_size(dir: &Path) -> Result<u64> {
    let mut total = 0u64;

    for entry in WalkDir::new(dir).min_depth(1) {
        let entry = entry.context("Failed to read directory entry")?;

        if entry.file_type().is_file() {
            let metadata = entry.metadata()
                .context(format!("Failed to read metadata for: {}", entry.path().display()))?;
            total += metadata.len();
        }
    }

    Ok(total)
}

/// Format bytes as human-readable size
fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Get destination directory name for a state directory
fn get_dest_dir(state_dir: &str) -> Option<String> {
    match state_dir {
        "memory-backups" => Some("backups/memory-backups".to_string()),
        "claude-md-backups" => Some("backups/claude-md-backups".to_string()),
        "backups" => Some("backups/backups".to_string()),
        _ => None,
    }
}

/// Migrate a single directory from state to data
pub fn migrate_directory(
    config: &MigrationConfig,
    options: &MigrationOptions,
) -> Result<MigrationStats> {
    // Validate directory names
    validate_dir_name(&config.state_dir)
        .context("Invalid source directory name")?;

    // Validate destination path components
    for component in config.data_dir.split('/') {
        validate_dir_name(component)
            .context(format!("Invalid destination path component: {}", component))?;
    }

    // Resolve XDG paths
    let state_base = NabiPaths::state_dir()
        .context("Failed to resolve XDG_STATE_HOME")?;
    let data_base = NabiPaths::data_dir()
        .context("Failed to resolve XDG_DATA_HOME")?;

    let source_dir = state_base.join(&config.state_dir);
    let dest_dir = data_base.join(&config.data_dir);

    // Validate paths are within their bases
    if source_dir.exists() {
        validate_path_within_base(&source_dir, &state_base)
            .context("Source path validation failed")?;
    }

    if dest_dir.exists() {
        validate_path_within_base(&dest_dir, &data_base)
            .context("Destination path validation failed")?;
    }

    println!(
        "{} Migrating: {} → {}",
        "→".blue(),
        config.state_dir.bright_white(),
        config.data_dir.bright_white()
    );
    println!("  Source: {}", source_dir.display());
    println!("  Destination: {}", dest_dir.display());

    // Check if source exists
    if !source_dir.exists() {
        println!("  {} Source directory does not exist (skipping)", "⚠".yellow());
        return Ok(MigrationStats::default());
    }

    if !source_dir.is_dir() {
        anyhow::bail!("Source path is not a directory: {}", source_dir.display());
    }

    // Check if source is empty
    let entries: Vec<_> = fs::read_dir(&source_dir)
        .context(format!("Failed to read source directory: {}", source_dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to read directory entries")?;

    if entries.is_empty() {
        println!("  {} Source directory is empty (nothing to migrate)", "ℹ".blue());
        return Ok(MigrationStats::default());
    }

    // Count files and calculate size
    let file_count = WalkDir::new(&source_dir)
        .min_depth(1)
        .into_iter()
        .filter(|e| e.as_ref().map(|e| e.file_type().is_file()).unwrap_or(false))
        .count();

    let total_size = calculate_size(&source_dir)
        .context("Failed to calculate source directory size")?;

    println!("  Found {} files to migrate (Total size: {})",
        file_count.to_string().bright_white(),
        format_size(total_size).bright_white()
    );

    // Detect conflicts
    let conflicts = detect_conflicts(&source_dir, &dest_dir)
        .context("Failed to detect conflicts")?;

    if !conflicts.is_empty() {
        println!("  {} Found {} conflicting files",
            "⚠".yellow(),
            conflicts.len().to_string().bright_yellow()
        );

        if options.verbose {
            for conflict in conflicts.iter().take(10) {
                println!("    Conflict: {}", conflict.display());
            }
            if conflicts.len() > 10 {
                println!("    ... and {} more", conflicts.len() - 10);
            }
        }

        if !options.force {
            anyhow::bail!(
                "Migration aborted due to {} conflicts. Use --force to overwrite.",
                conflicts.len()
            );
        } else {
            println!("  {} --force enabled, will overwrite conflicts", "⚠".yellow());
        }
    }

    if options.dry_run {
        println!("  {} DRY RUN MODE - No changes will be made", "ℹ".blue());
        println!("  Would execute:");
        println!("    mkdir -p {}", dest_dir.display());
        println!("    mv {}/* {}", source_dir.display(), dest_dir.display());
        println!("    rmdir {}", source_dir.display());
        return Ok(MigrationStats {
            files_moved: file_count,
            files_failed: 0,
            conflicts_detected: conflicts.len(),
            total_size_bytes: total_size,
        });
    }

    // Create destination directory
    fs::create_dir_all(&dest_dir)
        .context(format!("Failed to create destination directory: {}", dest_dir.display()))?;

    println!("  {} Created destination directory", "✓".green());

    // Move files
    println!("  {} Moving files...", "→".blue());
    let mut stats = MigrationStats {
        files_moved: 0,
        files_failed: 0,
        conflicts_detected: conflicts.len(),
        total_size_bytes: total_size,
    };

    for entry in WalkDir::new(&source_dir).min_depth(1) {
        let entry = entry.context("Failed to read source directory entry")?;

        if entry.file_type().is_file() {
            let source_file = entry.path();
            let relative_path = source_file.strip_prefix(&source_dir)
                .context("Failed to compute relative path")?;
            let dest_file = dest_dir.join(relative_path);

            // Create parent directory if needed
            if let Some(parent) = dest_file.parent() {
                fs::create_dir_all(parent)
                    .context(format!("Failed to create parent directory: {}", parent.display()))?;
            }

            // Handle force overwrite
            if options.force && dest_file.exists() {
                fs::remove_file(&dest_file)
                    .context(format!("Failed to remove conflicting file: {}", dest_file.display()))?;
            }

            // Move file
            if let Err(e) = fs::rename(source_file, &dest_file) {
                eprintln!("  {} Failed to move {}: {}",
                    "✗".red(),
                    relative_path.display(),
                    e
                );
                stats.files_failed += 1;
            } else {
                stats.files_moved += 1;
                if options.verbose && stats.files_moved % 10 == 0 {
                    print!("\r  Moved {}/{} files...", stats.files_moved, file_count);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
            }
        }
    }

    if options.verbose {
        println!(); // New line after progress
    }

    if stats.files_failed > 0 {
        eprintln!("  {} Migration incomplete: {} files failed to move",
            "✗".red(),
            stats.files_failed
        );
        anyhow::bail!("Migration failed for {} files", stats.files_failed);
    }

    println!("  {} Moved {} files successfully",
        "✓".green(),
        stats.files_moved.to_string().bright_white()
    );

    // Remove source directory if empty
    let remaining: Vec<_> = fs::read_dir(&source_dir)
        .context("Failed to check source directory")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to read directory entries")?;

    if remaining.is_empty() {
        fs::remove_dir(&source_dir)
            .context(format!("Failed to remove empty source directory: {}", source_dir.display()))?;
        println!("  {} Removed empty source directory", "✓".green());
    } else {
        println!("  {} Source directory not empty, leaving in place", "⚠".yellow());
        if options.verbose {
            println!("  Remaining items:");
            for entry in remaining.iter().take(5) {
                println!("    {}", entry.path().display());
            }
        }
    }

    // Final statistics
    let final_size = if dest_dir.exists() {
        calculate_size(&dest_dir).unwrap_or(0)
    } else {
        0
    };

    let final_count = if dest_dir.exists() {
        WalkDir::new(&dest_dir)
            .min_depth(1)
            .into_iter()
            .filter(|e| e.as_ref().map(|e| e.file_type().is_file()).unwrap_or(false))
            .count()
    } else {
        0
    };

    println!("  {} Migration complete!", "✓".green());
    println!("  Summary:");
    println!("    Files migrated: {}", stats.files_moved);
    println!("    Final location: {}", dest_dir.display());
    println!("    Final size: {} ({} files)", format_size(final_size), final_count);

    Ok(stats)
}

/// Migrate multiple directories
pub fn migrate_directories(
    configs: &[MigrationConfig],
    options: &MigrationOptions,
) -> Result<MigrationStats> {
    println!("{} State to Data Migration", "→".blue());
    println!("Migrating {} directory(ies) from XDG_STATE_HOME to XDG_DATA_HOME\n",
        configs.len().to_string().bright_white()
    );

    let mut total_stats = MigrationStats::default();
    let mut successful = 0;
    let mut failed = 0;

    for (idx, config) in configs.iter().enumerate() {
        println!("[{}/{}]", idx + 1, configs.len());

        match migrate_directory(config, options) {
            Ok(stats) => {
                total_stats.files_moved += stats.files_moved;
                total_stats.files_failed += stats.files_failed;
                total_stats.conflicts_detected += stats.conflicts_detected;
                total_stats.total_size_bytes += stats.total_size_bytes;
                successful += 1;
            }
            Err(e) => {
                eprintln!("  {} Migration failed for {}: {}",
                    "✗".red(),
                    config.state_dir,
                    e
                );
                failed += 1;
            }
        }

        if idx < configs.len() - 1 {
            println!(); // Blank line between migrations
        }
    }

    println!("\n{} Migration Summary", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".blue());
    println!("  Total directories: {}", configs.len());
    println!("  Successful: {}", successful.to_string().bright_green());
    println!("  Failed: {}", failed.to_string().bright_red());
    println!("  Total files moved: {}", total_stats.files_moved.to_string().bright_white());
    println!("  Total size: {}", format_size(total_stats.total_size_bytes).bright_white());

    if failed > 0 {
        anyhow::bail!("{} migration(s) failed", failed);
    }

    Ok(total_stats)
}

/// Handle migrate command
pub fn handle_migrate_run(
    dirs: &[String],
    dry_run: bool,
    force: bool,
    verbose: bool,
) -> Result<()> {

    // Default directories to migrate if none specified
    const DEFAULT_DIRS: &[&str] = &["memory-backups", "claude-md-backups", "backups"];

    let dirs_to_migrate: Vec<String> = if dirs.is_empty() {
        DEFAULT_DIRS.iter().map(|s| s.to_string()).collect()
    } else {
        dirs.to_vec()
    };

    // Build migration configs
    let mut configs = Vec::new();
    for state_dir in dirs_to_migrate {
        if let Some(data_dir) = get_dest_dir(&state_dir) {
            configs.push(MigrationConfig {
                state_dir,
                data_dir,
            });
        } else {
            eprintln!("  {} Unknown directory: {} (skipping)", "⚠".yellow(), state_dir);
        }
    }

    if configs.is_empty() {
        anyhow::bail!("No valid directories to migrate");
    }

    let options = MigrationOptions {
        dry_run,
        force,
        verbose,
    };

    migrate_directories(&configs, &options)?;
    Ok(())
}

/// Handle verify command
pub fn handle_migrate_verify(dirs: &[String], _verbose: bool) -> Result<()> {
    use colored::*;
    use crate::paths::NabiPaths;
    use walkdir::WalkDir;

    println!("{} Verifying migrations...\n", "→".blue());

    // Default directories to verify if none specified
    const DEFAULT_DIRS: &[&str] = &["memory-backups", "claude-md-backups", "backups"];

    let dirs_to_check: Vec<String> = if dirs.is_empty() {
        DEFAULT_DIRS.iter().map(|s| s.to_string()).collect()
    } else {
        dirs.to_vec()
    };

    let state_base = NabiPaths::state_dir()
        .context("Failed to resolve XDG_STATE_HOME")?;
    let data_base = NabiPaths::data_dir()
        .context("Failed to resolve XDG_DATA_HOME")?;

    let mut all_verified = true;

    for state_dir in dirs_to_check {
        if let Some(data_dir) = get_dest_dir(&state_dir) {
            let source_path = state_base.join(&state_dir);
            let dest_path = data_base.join(&data_dir);

            println!("Checking: {} → {}", state_dir.bright_white(), data_dir.bright_white());

            let source_exists = source_path.exists() && source_path.is_dir();
            let dest_exists = dest_path.exists() && dest_path.is_dir();

            if source_exists {
                let file_count = WalkDir::new(&source_path)
                    .min_depth(1)
                    .into_iter()
                    .filter(|e| e.as_ref().map(|e| e.file_type().is_file()).unwrap_or(false))
                    .count();
                println!("  {} Source still exists: {} ({} files)",
                    "⚠".yellow(),
                    source_path.display(),
                    file_count
                );
                all_verified = false;
            } else {
                println!("  {} Source removed: {}", "✓".green(), source_path.display());
            }

            if dest_exists {
                let file_count = WalkDir::new(&dest_path)
                    .min_depth(1)
                    .into_iter()
                    .filter(|e| e.as_ref().map(|e| e.file_type().is_file()).unwrap_or(false))
                    .count();
                println!("  {} Destination exists: {} ({} files)",
                    "✓".green(),
                    dest_path.display(),
                    file_count
                );
            } else {
                println!("  {} Destination not found: {}", "✗".red(), dest_path.display());
                all_verified = false;
            }

            if source_exists {
                println!("  {} Migration may be incomplete - source still exists", "⚠".yellow());
            }

            println!();
        } else {
            eprintln!("  {} Unknown directory: {} (skipping)", "⚠".yellow(), state_dir);
        }
    }

    if all_verified {
        println!("{} All migrations verified successfully", "✓".green());
        Ok(())
    } else {
        println!("{} Some migrations may be incomplete", "⚠".yellow());
        anyhow::bail!("Verification failed");
    }
}
