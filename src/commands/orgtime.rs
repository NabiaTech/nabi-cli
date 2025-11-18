/// Organize files/directories with timestamp prefixes and topology categories
///
/// Renames files/directories using their latest modification time as prefix
/// followed by inferred topology categories for better chronological ordering.

use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Common topology categories for automatic inference
// TODO make this this dynamic based off a schema toml file in xdg config nabi
const TOPOLOGY_CATEGORIES: &[&str] = &[
    "inventory", "architecture", "observability", "guardian", "activation",
    "link-mapper", "discovery", "coordination", "migration", "setup",
    "configuration", "deployment", "monitoring", "analysis", "strategy",
];

/// Run the orgtime command
pub fn cmd_run(
    path: String,
    category: Option<String>,
    files: bool,
    preserve_times: bool,
    dry_run: bool,
) -> Result<()> {
    let target_path = PathBuf::from(&path);

    if !target_path.exists() {
        anyhow::bail!("Path does not exist: {}", path);
    }

    if files && target_path.is_file() {
        // Single file operation
        process_single_file(&target_path, category, preserve_times, dry_run)
    } else if files && target_path.is_dir() {
        // Process files within directory
        process_directory_files(&target_path, category, preserve_times, dry_run)
    } else if target_path.is_dir() {
        // Rename the directory itself
        process_directory(&target_path, category, preserve_times, dry_run)
    } else {
        anyhow::bail!("Invalid target: {} (expected directory or file)", path);
    }
}

/// Process a single file
fn process_single_file(
    file_path: &Path,
    category: Option<String>,
    preserve_times: bool,
    dry_run: bool,
) -> Result<()> {
    let category = category.unwrap_or_else(|| infer_category_from_filename(file_path));
    let timestamp = get_file_timestamp(file_path)?;
    let new_name = format!("{}_{}_{}", timestamp, category, file_path.file_name().unwrap().to_string_lossy());

    println!("📁 {} → {}", file_path.display(), new_name.green());

    if !dry_run {
        if preserve_times {
            preserve_rename(file_path, &PathBuf::from(&new_name))?;
        } else {
            fs::rename(file_path, &new_name)?;
        }
    }

    Ok(())
}

/// Process all files within a directory
fn process_directory_files(
    dir_path: &Path,
    category: Option<String>,
    preserve_times: bool,
    dry_run: bool,
) -> Result<()> {
    let mut files_processed = 0;

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let file_category = category.clone().unwrap_or_else(|| infer_category_from_filename(&path));
            let timestamp = get_file_timestamp(&path)?;
            let file_name = path.file_name().unwrap().to_string_lossy();
            let new_name = format!("{}_{}_{}", timestamp, file_category, file_name);
            let new_path = dir_path.join(&new_name);

            println!("📄 {} → {}", path.strip_prefix(dir_path)?.display(), new_name.green());

            if !dry_run {
                if preserve_times {
                    preserve_rename(&path, &new_path)?;
                } else {
                    fs::rename(&path, &new_path)?;
                }
            }

            files_processed += 1;
        }
    }

    println!("\n✅ Processed {} files in {}", files_processed, dir_path.display());
    Ok(())
}

/// Process a directory (rename the directory itself)
fn process_directory(
    dir_path: &Path,
    category: Option<String>,
    preserve_times: bool,
    dry_run: bool,
) -> Result<()> {
    let category = category.unwrap_or_else(|| infer_category_from_directory(dir_path));
    let timestamp = get_directory_latest_timestamp(dir_path)?;
    let dir_name = dir_path.file_name().unwrap().to_string_lossy();
    let new_name = format!("{}_{}", timestamp, category);

    let parent = dir_path.parent().unwrap_or(Path::new("."));
    let new_path = parent.join(&new_name);

    println!("📁 {} → {}", dir_path.display(), new_name.green());

    if !dry_run {
        if preserve_times {
            preserve_rename(dir_path, &new_path)?;
        } else {
            fs::rename(dir_path, &new_path)?;
        }
    }

    Ok(())
}

/// Get file timestamp in YYYY-MM-DD_HH-MM-SS format
fn get_file_timestamp(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path)?;
    let mtime = metadata.modified()?;
    let datetime: chrono::DateTime<chrono::Local> = mtime.into();
    Ok(datetime.format("%Y-%m-%d_%H-%M-%S").to_string())
}

/// Get latest timestamp from files in directory
fn get_directory_latest_timestamp(dir_path: &Path) -> Result<String> {
    let mut latest_time = None;

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let metadata = fs::metadata(&path)?;
            let mtime = metadata.modified()?;

            if latest_time.is_none() || mtime > latest_time.unwrap() {
                latest_time = Some(mtime);
            }
        }
    }

    match latest_time {
        Some(time) => {
            let datetime: chrono::DateTime<chrono::Local> = time.into();
            Ok(datetime.format("%Y-%m-%d_%H-%M-%S").to_string())
        }
        None => anyhow::bail!("No files found in directory"),
    }
}

/// Infer category from filename
fn infer_category_from_filename(path: &Path) -> String {
    let filename = path.file_name().unwrap().to_string_lossy().to_lowercase();

    // Check for exact matches in filename
    for category in TOPOLOGY_CATEGORIES {
        if filename.contains(category) {
            return category.to_string();
        }
    }

    // Check file extensions and patterns
    if filename.ends_with(".md") {
        if filename.contains("readme") || filename.contains("index") {
            "documentation".to_string()
        } else if filename.contains("config") || filename.contains("setup") {
            "configuration".to_string()
        } else if filename.contains("analysis") || filename.contains("report") {
            "analysis".to_string()
        } else {
            "documentation".to_string()
        }
    } else if filename.ends_with(".txt") || filename.ends_with(".log") {
        "logs".to_string()
    } else if filename.ends_with(".json") || filename.ends_with(".yaml") || filename.ends_with(".yml") {
        "configuration".to_string()
    } else {
        "misc".to_string()
    }
}

/// Infer category from directory name and contents
fn infer_category_from_directory(dir_path: &Path) -> String {
    let dir_name = dir_path.file_name().unwrap().to_string_lossy().to_lowercase();

    // Check directory name
    for category in TOPOLOGY_CATEGORIES {
        if dir_name.contains(category) {
            return category.to_string();
        }
    }

    // Check file contents for patterns
    if let Ok(entries) = fs::read_dir(dir_path) {
        let mut file_types = HashMap::new();

        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_string();
                        *file_types.entry(ext_str).or_insert(0) += 1;
                    }
                }
            }
        }

        // Infer category from file type distribution
        if file_types.contains_key("md") && file_types.get("md").unwrap() > &2 {
            "documentation".to_string()
        } else if file_types.contains_key("py") {
            "code".to_string()
        } else if file_types.contains_key("log") || file_types.contains_key("txt") {
            "logs".to_string()
        } else {
            "misc".to_string()
        }
    } else {
        "misc".to_string()
    }
}

/// Rename while preserving timestamps using touch -r
fn preserve_rename(from: &Path, to: &Path) -> Result<()> {
    // First, get the original timestamp
    let metadata = fs::metadata(from)?;
    let mtime = metadata.modified()?;

    // Perform the rename
    fs::rename(from, to)?;

    // Restore the original timestamp
    let datetime: chrono::DateTime<chrono::Local> = mtime.into();
    let timestamp_str = datetime.format("%Y%m%d%H%M.%S").to_string();

    // Use touch -t to restore timestamp
    Command::new("touch")
        .arg("-t")
        .arg(&timestamp_str)
        .arg(to)
        .status()
        .context("Failed to restore timestamp")?;

    Ok(())
}
