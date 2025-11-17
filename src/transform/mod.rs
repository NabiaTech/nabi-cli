//! Transformation Engine - Executes the Formalized Trinity Pattern
//!
//! Three transformation types:
//! 1. **Behavioral**: Direct TOML consumption at runtime (no transformation)
//! 2. **Structural**: TOML → JSON (schema validated serialization)
//! 3. **Generative**: TOML + Jinja2 template → Generated files
//!
//! All transformations respect XDG directory structure and support `~/` path expansion.

pub mod schema;
pub mod structural;
pub mod generative;
pub mod error;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

pub use error::TransformError;

/// Metadata for configuration transformation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransformMeta {
    pub transformation_type: TransformationType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    pub validators: Vec<String>,
    pub consumers: Vec<String>,
    pub schema_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_validated: Option<String>,
}

/// The three transformation types in the Formalized Trinity Pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransformationType {
    /// Direct read at runtime - no transformation needed
    Behavioral,
    /// TOML → JSON with schema validation
    Structural,
    /// TOML + Jinja2 template → generated files
    Generative,
}

/// Expands home directory paths (`~/...`) to absolute paths
///
/// Respects environment variables like `$HOME`, `$XDG_CONFIG_HOME`, etc.
pub fn expand_path(path: &str) -> Result<PathBuf> {
    if path.starts_with("~/") {
        let home = std::env::var("HOME")
            .context("Failed to get HOME environment variable")?;
        Ok(PathBuf::from(path.replace("~", &home)))
    } else if path.starts_with("$") {
        // Handle $VAR expansion
        let parts: Vec<&str> = path.split('/').collect();
        if let Some(var_name) = parts.first() {
            let var_key = var_name.trim_start_matches('$');
            if let Ok(var_val) = std::env::var(var_key) {
                let rest = parts[1..].join("/");
                Ok(PathBuf::from(var_val).join(rest))
            } else {
                Ok(PathBuf::from(path))
            }
        } else {
            Ok(PathBuf::from(path))
        }
    } else if path.starts_with('/') {
        Ok(PathBuf::from(path))
    } else {
        // Relative path - resolve against current directory
        std::env::current_dir()
            .context("Failed to get current directory")?
            .canonicalize()
            .map(|d| d.join(path))
            .context("Failed to canonicalize path")
    }
}

/// Creates parent directories if they don't exist
pub fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .context(format!("Failed to create directory {}", parent.display()))?;
        }
    }
    Ok(())
}

/// Adds metadata timestamp to JSON output
pub fn add_generated_metadata(json: &mut serde_json::Value, source_file: &str, schema_version: &str) {
    if let Some(obj) = json.as_object_mut() {
        obj.insert(
            "_meta".to_string(),
            json!({
                "generated_at": Utc::now().to_rfc3339(),
                "source_file": source_file,
                "schema_version": schema_version,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_home_path() {
        std::env::set_var("HOME", "/Users/test");
        let expanded = expand_path("~/config/nabi").unwrap();
        assert_eq!(expanded, PathBuf::from("/Users/test/config/nabi"));
    }

    #[test]
    fn test_expand_absolute_path() {
        let expanded = expand_path("/tmp/test").unwrap();
        assert_eq!(expanded, PathBuf::from("/tmp/test"));
    }

    #[test]
    fn test_expand_relative_path() {
        let expanded = expand_path("./test").unwrap();
        assert!(expanded.is_absolute());
    }
}
