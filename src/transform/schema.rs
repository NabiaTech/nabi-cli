//! JSON Schema validation for TOML configurations
//!
//! Validates TOML structures against JSON schemas before transformation,
//! providing detailed error messages with validation context.

use crate::paths::NabiPaths;
use crate::transform::error::{TransformError, TransformResult};
use jsonschema::JSONSchema;
use serde_json::{from_str as json_from_str, json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Loads a JSON schema from the governance/schemas directory
fn load_schema(schema_name: &str) -> TransformResult<JSONSchema> {
    let config_dir = NabiPaths::config_dir().map_err(|e| TransformError::Internal {
        message: e.to_string(),
    })?;
    let schema_path = config_dir
        .join("governance")
        .join("schemas")
        .join(schema_name);

    if !schema_path.exists() {
        return Err(TransformError::SchemaReadError {
            path: schema_path.clone(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "Schema file not found"),
        });
    }

    let schema_content =
        fs::read_to_string(&schema_path).map_err(|e| TransformError::SchemaReadError {
            path: schema_path.clone(),
            source: e,
        })?;

    let schema_json: Value =
        json_from_str(&schema_content).map_err(|e| TransformError::SchemaParseError {
            path: schema_path.clone(),
            source: e,
        })?;

    JSONSchema::compile(&schema_json).map_err(|e| TransformError::SchemaValidationFailed {
        file: schema_name.to_string(),
        reason: format!("Failed to compile schema: {}", e),
    })
}

/// Validates a TOML value against one or more JSON schemas
///
/// # Arguments
/// * `toml_value` - The TOML data parsed as JSON
/// * `schema_names` - Array of schema file names to validate against
/// * `file_path` - Path to the TOML file (for error reporting)
///
/// # Returns
/// * `Ok(())` if validation passes
/// * `Err` with detailed validation error if any schema fails
pub fn validate_against_schemas(
    toml_value: &Value,
    schema_names: &[String],
    file_path: &Path,
) -> TransformResult<()> {
    if schema_names.is_empty() {
        return Ok(()); // No schemas to validate against
    }

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<unknown>");

    for schema_name in schema_names {
        let schema = load_schema(schema_name)?;

        schema.validate(toml_value).map_err(|e| {
            let errors: Vec<String> = e.into_iter().map(|err| format!("  - {}", err)).collect();

            TransformError::SchemaValidationFailed {
                file: file_name.to_string(),
                reason: format!("Invalid against {}\n{}", schema_name, errors.join("\n")),
            }
        })?;
    }

    Ok(())
}

/// Validates that required metadata fields exist
pub fn validate_metadata(toml_value: &Value, file_path: &Path) -> TransformResult<()> {
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<unknown>");

    // Check for [meta] section
    let meta = toml_value
        .get("meta")
        .ok_or(TransformError::MissingMetadata {
            field: "meta (section)".to_string(),
            file: file_name.to_string(),
        })?;

    // Check for transformation_type
    let _trans_type = meta
        .get("transformation_type")
        .ok_or(TransformError::MissingMetadata {
            field: "transformation_type".to_string(),
            file: file_name.to_string(),
        })?;

    // Check for schema_version
    let _schema_version = meta
        .get("schema_version")
        .ok_or(TransformError::MissingMetadata {
            field: "schema_version".to_string(),
            file: file_name.to_string(),
        })?;

    // Check for validators or consumers
    let has_validators = meta.get("validators").is_some();
    let has_consumers = meta.get("consumers").is_some();

    if !has_validators && !has_consumers {
        // At least one should exist
        eprintln!(
            "⚠ Warning: {} should have 'validators' or 'consumers' in [meta]",
            file_name
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_metadata_missing_section() {
        let json = json!({
            "service": { "id": "test" }
        });

        let result = validate_metadata(&json, Path::new("test.toml"));
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(TransformError::MissingMetadata { .. })
        ));
    }

    #[test]
    fn test_validate_metadata_missing_transformation_type() {
        let json = json!({
            "meta": {
                "schema_version": "1.0.0"
            }
        });

        let result = validate_metadata(&json, Path::new("test.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_metadata_valid() {
        let json = json!({
            "meta": {
                "transformation_type": "structural",
                "schema_version": "1.0.0",
                "validators": ["service.schema.json"],
                "consumers": ["launcher"]
            }
        });

        let result = validate_metadata(&json, Path::new("test.toml"));
        assert!(result.is_ok());
    }
}
