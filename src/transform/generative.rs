//! Generative Transformation: TOML + Jinja2 Template → Generated Code
//!
//! Transforms TOML configuration files using Jinja2-like templates to generate:
//! - Environment files (.env)
//! - Shell scripts
//! - Configuration files
//! - Docker Compose files
//! - Any text-based output
//!
//! Uses Handlebars (Rust-native Jinja2 alternative) for template rendering.

use crate::transform::error::TransformResult;
use crate::transform::{expand_path, ensure_parent_dir, TransformError, TransformMeta};
use handlebars::{Context, Handlebars, TemplateError};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use toml::Table;

/// Configuration file with metadata for generative transformation
pub struct GenerativeConfig {
    /// Parsed TOML data
    pub data: Table,
    /// Transformation metadata from [meta] section
    pub meta: TransformMeta,
    /// Original file path (for source tracking)
    pub source_path: PathBuf,
}

impl GenerativeConfig {
    /// Loads a TOML file and validates it's suitable for generative transformation
    pub fn load(path: &Path) -> TransformResult<Self> {
        // Read TOML file
        let content = fs::read_to_string(path).map_err(|e| TransformError::TomlParseError {
            path: path.to_path_buf(),
            source: e,
        })?;

        // Parse TOML
        let data: Table = toml::from_str(&content).map_err(|e| TransformError::TomlReadError {
            path: path.to_path_buf(),
            source: e,
        })?;

        // Extract metadata
        let meta_table = data.get("meta").ok_or(TransformError::MissingMetadata {
            field: "meta".to_string(),
            file: path.display().to_string(),
        })?;

        // Parse transformation type
        let transform_type_str = meta_table
            .get("transformation_type")
            .and_then(|v| v.as_str())
            .ok_or(TransformError::MissingMetadata {
                field: "transformation_type".to_string(),
                file: path.display().to_string(),
            })?;

        if transform_type_str != "generative" {
            return Err(TransformError::InvalidTransformationType {
                type_name: transform_type_str.to_string(),
                file: path.display().to_string(),
            });
        }

        // Validate required fields for generative
        let template = meta_table
            .get("template")
            .and_then(|v| v.as_str())
            .ok_or(TransformError::MissingTemplate {
                file: path.display().to_string(),
            })?
            .to_string();

        let output_path = meta_table
            .get("output_path")
            .and_then(|v| v.as_str())
            .ok_or(TransformError::MissingOutputPath {
                file: path.display().to_string(),
            })?
            .to_string();

        // Build metadata
        let meta = TransformMeta {
            transformation_type: crate::transform::TransformationType::Generative,
            template: Some(template),
            output_path: Some(output_path),
            validators: meta_table
                .get("validators")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default(),
            consumers: meta_table
                .get("consumers")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default(),
            schema_version: meta_table
                .get("schema_version")
                .and_then(|v| v.as_str())
                .unwrap_or("1.0.0")
                .to_string(),
            last_validated: None,
        };

        Ok(GenerativeConfig {
            data,
            meta,
            source_path: path.to_path_buf(),
        })
    }

    /// Loads the Jinja2 template and prepares it for rendering
    fn load_template(&self) -> TransformResult<String> {
        let template_path = self
            .meta
            .template
            .as_ref()
            .ok_or(TransformError::MissingTemplate {
                file: self.source_path.display().to_string(),
            })?;

        let expanded_path = expand_path(template_path).map_err(|e| {
            TransformError::PathExpansionError {
                original: template_path.clone(),
                reason: e.to_string(),
            }
        })?;

        if !expanded_path.exists() {
            return Err(TransformError::TemplateReadError {
                path: expanded_path,
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Template file not found",
                ),
            });
        }

        fs::read_to_string(&expanded_path).map_err(|e| TransformError::TemplateReadError {
            path: expanded_path,
            source: e,
        })
    }

    /// Transforms TOML using template and writes to output file
    ///
    /// Includes:
    /// - Template loading
    /// - Handlebars rendering with TOML data as context
    /// - Safe file I/O with parent directory creation
    /// - Support for ~/  path expansion
    pub fn transform(&self) -> TransformResult<(String, PathBuf)> {
        // Load template
        let template_content = self.load_template()?;

        // Get output path
        let output_path_str = self
            .meta
            .output_path
            .as_ref()
            .ok_or(TransformError::MissingOutputPath {
                file: self.source_path.display().to_string(),
            })?;

        let output_path = expand_path(output_path_str).map_err(|e| {
            TransformError::PathExpansionError {
                original: output_path_str.clone(),
                reason: e.to_string(),
            }
        })?;

        // Create parent directory
        ensure_parent_dir(&output_path)
            .map_err(|e| TransformError::Internal { message: e.to_string() })?;

        // Convert TOML data to JSON for template rendering
        let data_json = serde_json::to_value(&self.data)
            .map_err(|e| TransformError::JsonSerializeError { source: e })?;

        // Render template
        let mut handlebars = Handlebars::new();

        // Register custom helper for path expansion if needed
        handlebars
            .register_template_string("generative", &template_content)
            .map_err(|e: TemplateError| TransformError::TemplateRenderError {
                template: self
                    .meta
                    .template
                    .as_ref()
                    .cloned()
                    .unwrap_or_default(),
                reason: e.to_string(),
            })?;

        let rendered = handlebars
            .render("generative", &data_json)
            .map_err(|e| TransformError::TemplateRenderError {
                template: self
                    .meta
                    .template
                    .as_ref()
                    .cloned()
                    .unwrap_or_default(),
                reason: e.to_string(),
            })?;

        // Write to file
        fs::write(&output_path, &rendered).map_err(|e| TransformError::WriteError {
            path: output_path.clone(),
            source: e,
        })?;

        Ok((rendered, output_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_generative_config() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
[meta]
transformation_type = "generative"
schema_version = "1.0.0"
template = "/tmp/test.j2"
output_path = "/tmp/test.env"
consumers = ["service"]

[service]
host = "0.0.0.0"
port = 9000
"#;
        file.write_all(content.as_bytes()).unwrap();

        let result = GenerativeConfig::load(file.path());
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.meta.schema_version, "1.0.0");
        assert!(config.meta.template.is_some());
        assert!(config.meta.output_path.is_some());
    }

    #[test]
    fn test_load_missing_template() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
[meta]
transformation_type = "generative"
schema_version = "1.0.0"
output_path = "/tmp/test.env"
"#;
        file.write_all(content.as_bytes()).unwrap();

        let result = GenerativeConfig::load(file.path());
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(TransformError::MissingTemplate { .. })
        ));
    }

    #[test]
    fn test_load_missing_output_path() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
[meta]
transformation_type = "generative"
schema_version = "1.0.0"
template = "/tmp/test.j2"
"#;
        file.write_all(content.as_bytes()).unwrap();

        let result = GenerativeConfig::load(file.path());
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(TransformError::MissingOutputPath { .. })
        ));
    }

    #[test]
    fn test_load_invalid_transformation_type() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
[meta]
transformation_type = "structural"
schema_version = "1.0.0"
template = "/tmp/test.j2"
output_path = "/tmp/test.env"
"#;
        file.write_all(content.as_bytes()).unwrap();

        let result = GenerativeConfig::load(file.path());
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(TransformError::InvalidTransformationType { .. })
        ));
    }
}
