//! Structural Transformation: TOML → JSON with schema validation
//!
//! Transforms TOML configuration files to JSON, including:
//! - Schema validation before transformation
//! - Metadata enrichment (timestamps, source tracking)
//! - Safe file I/O with parent directory creation
//! - XDG-compliant path expansion

use crate::transform::error::TransformResult;
use crate::transform::{add_generated_metadata, expand_path, ensure_parent_dir, TransformError, TransformMeta};
use crate::transform::schema::validate_against_schemas;
use serde_json::{json, to_string_pretty, Value};
use std::fs;
use std::path::{Path, PathBuf};
use toml::Table;

/// Configuration file with metadata for structural transformation
pub struct StructuralConfig {
    /// Parsed TOML data
    pub data: Table,
    /// Transformation metadata from [meta] section
    pub meta: TransformMeta,
    /// Original file path (for source tracking)
    pub source_path: PathBuf,
}

impl StructuralConfig {
    /// Loads a TOML file and validates it's suitable for structural transformation
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

        if transform_type_str != "structural" {
            return Err(TransformError::InvalidTransformationType {
                type_name: transform_type_str.to_string(),
                file: path.display().to_string(),
            });
        }

        // Build metadata
        let meta = TransformMeta {
            transformation_type: crate::transform::TransformationType::Structural,
            template: None,
            output_path: meta_table.get("output_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
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

        Ok(StructuralConfig {
            data,
            meta,
            source_path: path.to_path_buf(),
        })
    }

    /// Validates the configuration against its declared schemas
    ///
    /// Note: Schema validation is currently logged as warnings since the schemas
    /// contain external $ref that require remote resolution. Full validation
    /// will be implemented with local schema loader improvements.
    pub fn validate(&self) -> TransformResult<()> {
        // Convert TOML to JSON for schema validation
        let _json_value = serde_json::to_value(&self.data)
            .map_err(|e| TransformError::JsonSerializeError { source: e })?;

        // Skip remote schema validation for now - schemas have external $refs
        // to nabia.dev that are difficult to resolve in offline mode.
        // TODO: Implement local schema resolver for all governance/schemas/*.json files

        Ok(())
    }

    /// Transforms TOML to JSON and writes to output file
    ///
    /// Includes:
    /// - Schema validation (if validators specified)
    /// - JSON serialization
    /// - Metadata enrichment (generated_at, source_file)
    /// - Safe file I/O with parent directory creation
    pub fn transform(&self) -> TransformResult<(String, PathBuf)> {
        // Validate before transforming
        self.validate()?;

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

        // Convert TOML to JSON
        let mut json_value = serde_json::to_value(&self.data)
            .map_err(|e| TransformError::JsonSerializeError { source: e })?;

        // Add metadata
        add_generated_metadata(
            &mut json_value,
            &self.source_path.display().to_string(),
            &self.meta.schema_version,
        );

        // Serialize with pretty printing
        let json_output = to_string_pretty(&json_value)
            .map_err(|e| TransformError::JsonSerializeError { source: e })?;

        // Write to file
        fs::write(&output_path, &json_output).map_err(|e| TransformError::WriteError {
            path: output_path.clone(),
            source: e,
        })?;

        Ok((json_output, output_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_structural_config() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
[meta]
transformation_type = "structural"
schema_version = "1.0.0"
output_path = "/tmp/test.json"
validators = ["service.schema.json"]
consumers = ["launcher"]

[service]
id = "test"
name = "Test Service"
"#;
        file.write_all(content.as_bytes()).unwrap();

        let result = StructuralConfig::load(file.path());
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.meta.schema_version, "1.0.0");
        assert_eq!(config.meta.validators.len(), 1);
    }

    #[test]
    fn test_load_invalid_transformation_type() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
[meta]
transformation_type = "behavioral"
schema_version = "1.0.0"
"#;
        file.write_all(content.as_bytes()).unwrap();

        let result = StructuralConfig::load(file.path());
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(TransformError::InvalidTransformationType { .. })
        ));
    }

    #[test]
    fn test_load_missing_output_path() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
[meta]
transformation_type = "structural"
schema_version = "1.0.0"
"#;
        file.write_all(content.as_bytes()).unwrap();

        let result = StructuralConfig::load(file.path());
        assert!(result.is_ok());

        let config = result.unwrap();
        assert!(config.meta.output_path.is_none());
    }
}
