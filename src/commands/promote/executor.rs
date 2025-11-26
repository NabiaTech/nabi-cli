// Execution Abstraction Layer
// Resolves promoted artifacts and executes tools from deployed locations

use anyhow::{anyhow, Context, Result};
use colored::*;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use super::record::PromotionRecord;

/// Execute a tool from its promoted location
///
/// This function implements the core execution abstraction:
/// 1. Loads promotion record to find deployed artifacts
/// 2. Resolves the promoted executable path
/// 3. Executes from promoted location (not source)
///
/// # Arguments
/// * `tool_id` - Tool identifier (e.g., "cursorignore")
/// * `args` - Command-line arguments to pass to the tool
///
/// # Returns
/// * `Ok(ExitStatus)` - Tool executed successfully
/// * `Err(_)` - Tool not promoted or execution failed
///
/// # Example
/// ```
/// let status = execute_promoted_tool("cursorignore", &vec!["sync".to_string()])?;
/// if !status.success() {
///     eprintln!("Tool failed with code: {:?}", status.code());
/// }
/// ```
pub fn execute_promoted_tool(tool_id: &str, args: &[String]) -> Result<ExitStatus> {
    // 1. Load promotion record
    let record = load_promotion_record(tool_id).context(format!(
        "Tool not promoted: {}. Run: nabi tool promote {}",
        tool_id, tool_id
    ))?;

    // 2. Resolve promoted executable path
    let exec_path = resolve_promoted_executable(&record).context(format!(
        "No executable artifact found for tool: {}",
        tool_id
    ))?;

    println!(
        "  {} Executing promoted tool: {}",
        "→".cyan(),
        tool_id.bold()
    );
    println!(
        "  {} Version: {}",
        "→".cyan(),
        record.version.yellow()
    );
    println!(
        "  {} Path: {}",
        "→".cyan(),
        exec_path.display().to_string().dimmed()
    );

    // 3. Determine execution strategy based on artifact type
    let status = execute_artifact(&exec_path, args)?;

    Ok(status)
}

/// Check if a tool is promoted
///
/// This is a lightweight check to determine if execution abstraction
/// should be used for a given tool.
///
/// # Arguments
/// * `tool_id` - Tool identifier
///
/// # Returns
/// * `true` - Promotion record exists
/// * `false` - Tool not promoted
pub fn is_tool_promoted(tool_id: &str) -> bool {
    let record_path = get_promotion_record_path(tool_id);
    record_path.exists()
}

/// Get promoted executable path for a tool
///
/// This function extracts the resolved path without executing.
/// Useful for inspection, validation, or wrapper scripts.
///
/// # Arguments
/// * `tool_id` - Tool identifier
///
/// # Returns
/// * `Ok(PathBuf)` - Absolute path to promoted executable
/// * `Err(_)` - Tool not promoted or no executable artifact
pub fn get_promoted_executable(tool_id: &str) -> Result<PathBuf> {
    let record = load_promotion_record(tool_id)?;
    resolve_promoted_executable(&record)
}

/// Get promotion record for inspection
///
/// Allows CLI commands to inspect promotion metadata without execution.
///
/// # Arguments
/// * `tool_id` - Tool identifier
///
/// # Returns
/// * `Ok(PromotionRecord)` - Full promotion metadata
/// * `Err(_)` - Tool not promoted or record corrupted
pub fn get_promotion_record(tool_id: &str) -> Result<PromotionRecord> {
    load_promotion_record(tool_id)
}

/// Resolve executable artifact from promotion record
///
/// Search strategy:
/// 1. Look for "binary" artifact type (preferred for CLI execution)
///    - SKIP if it's a symlink (indicates LIVE mode, not truly promoted)
/// 2. Fall back to "library" artifact type (for single-file scripts)
/// 3. Fall back to "executable" artifact type
///
/// This ensures we use the promoted artifact, not source via symlink.
fn resolve_promoted_executable(record: &PromotionRecord) -> Result<PathBuf> {
    // Priority 1: binary artifact (wrapper script in ~/.local/share/nabi/bin/)
    // BUT skip symlinks - those point back to source (LIVE mode)
    for artifact in &record.artifacts {
        if artifact.artifact_type == "binary" {
            let path = expand_path(&artifact.target_path);
            if path.exists() && path.is_file() && !path.is_symlink() {
                // Verify it's executable or a script
                if is_executable_artifact(&path)? {
                    return Ok(path);
                }
            }
        }
    }

    // Priority 2: library artifact (single-file scripts)
    // This is the STABLE mode copy - use this for execution abstraction
    for artifact in &record.artifacts {
        if artifact.artifact_type == "library" {
            let path = expand_path(&artifact.target_path);
            if path.exists() && path.is_file() {
                if is_executable_artifact(&path)? {
                    return Ok(path);
                }
            }
        }
    }

    // Priority 3: executable artifact
    for artifact in &record.artifacts {
        if artifact.artifact_type == "executable" {
            let path = expand_path(&artifact.target_path);
            if path.exists() && path.is_file() && !path.is_symlink() {
                if is_executable_artifact(&path)? {
                    return Ok(path);
                }
            }
        }
    }

    Err(anyhow!(
        "No valid executable artifact found in promotion record. Artifacts: {}",
        record
            .artifacts
            .iter()
            .map(|a| format!("{} ({})", a.artifact_type, a.target_path))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Check if a path represents an executable artifact
///
/// Criteria:
/// 1. File exists and is a regular file
/// 2. Has executable permissions OR is a known script type (.py, .sh, .rb, etc.)
#[cfg(unix)]
fn is_executable_artifact(path: &PathBuf) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    if !path.is_file() {
        return Ok(false);
    }

    let metadata = fs::metadata(path)?;
    let permissions = metadata.permissions();

    // Check Unix executable bit
    if permissions.mode() & 0o111 != 0 {
        return Ok(true);
    }

    // Check for known script extensions
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy();
        if matches!(ext_str.as_ref(), "py" | "sh" | "rb" | "pl" | "js" | "ts") {
            return Ok(true);
        }
    }

    // Check for shebang
    if has_shebang(path)? {
        return Ok(true);
    }

    Ok(false)
}

/// Check for shebang in file
fn has_shebang(path: &PathBuf) -> Result<bool> {
    let content = fs::read(path)?;
    Ok(content.len() >= 2 && content[0] == b'#' && content[1] == b'!')
}

/// Execute an artifact with appropriate interpreter
///
/// Execution strategy:
/// 1. If file has executable bit → execute directly
/// 2. If Python script (.py) → use python3
/// 3. If shell script (.sh) → use bash
/// 4. If has shebang → execute directly
/// 5. Otherwise → return error
fn execute_artifact(path: &PathBuf, args: &[String]) -> Result<ExitStatus> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(path)?;
        let permissions = metadata.permissions();

        // Strategy 1: Executable bit set
        if permissions.mode() & 0o111 != 0 {
            return Command::new(path)
                .args(args)
                .status()
                .context("Failed to execute promoted tool");
        }
    }

    // Strategy 2: Known script types
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy();
        match ext_str.as_ref() {
            "py" => {
                return Command::new("python3")
                    .arg(path)
                    .args(args)
                    .status()
                    .context("Failed to execute Python script");
            }
            "sh" => {
                return Command::new("bash")
                    .arg(path)
                    .args(args)
                    .status()
                    .context("Failed to execute shell script");
            }
            "rb" => {
                return Command::new("ruby")
                    .arg(path)
                    .args(args)
                    .status()
                    .context("Failed to execute Ruby script");
            }
            "js" => {
                return Command::new("node")
                    .arg(path)
                    .args(args)
                    .status()
                    .context("Failed to execute JavaScript");
            }
            _ => {}
        }
    }

    // Strategy 3: Try direct execution (shebang handling)
    Command::new(path)
        .args(args)
        .status()
        .context("Failed to execute promoted tool")
}

/// Load promotion record from state directory
fn load_promotion_record(tool_id: &str) -> Result<PromotionRecord> {
    let path = get_promotion_record_path(tool_id);

    let content = fs::read_to_string(&path).context(format!(
        "Failed to read promotion record: {}",
        path.display()
    ))?;

    let record: PromotionRecord = serde_json::from_str(&content).context(format!(
        "Failed to parse promotion record (corrupted JSON?): {}",
        path.display()
    ))?;

    Ok(record)
}

/// Get promotion record path for a tool
fn get_promotion_record_path(tool_id: &str) -> PathBuf {
    expand_path(&format!(
        "~/.local/state/nabi/promoted/{}.json",
        tool_id
    ))
}

/// Expand ~ and environment variables in paths
fn expand_path(path: &str) -> PathBuf {
    let expanded = shellexpand::tilde(path);
    PathBuf::from(expanded.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_promotion_record_path_generation() {
        let path = get_promotion_record_path("cursorignore");
        assert!(path.to_string_lossy().ends_with("promoted/cursorignore.json"));
    }

    #[test]
    fn test_path_expansion() {
        let expanded = expand_path("~/.local/share/nabi/lib");
        assert!(!expanded.to_string_lossy().contains('~'));
    }
}
