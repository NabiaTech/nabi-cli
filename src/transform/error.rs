//! Error types for the transformation engine
//!
//! Provides context-aware error reporting for validation, transformation,
//! and file operations with actionable diagnostics.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransformError {
    #[error("Schema validation failed for {file}: {reason}")]
    SchemaValidationFailed { file: String, reason: String },

    #[error("Failed to read TOML file {path}: {source}")]
    TomlReadError {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("Failed to parse TOML in {path}: {source}")]
    TomlParseError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to read JSON schema {path}: {source}")]
    SchemaReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse JSON schema {path}: {source}")]
    SchemaParseError {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("Missing required metadata field: {field} in {file}")]
    MissingMetadata { field: String, file: String },

    #[error("Invalid transformation type '{type_name}' in {file}. Must be one of: behavioral, structural, generative")]
    InvalidTransformationType { type_name: String, file: String },

    #[error("Generative transformation requires 'template' field in {file}")]
    MissingTemplate { file: String },

    #[error("Generative transformation requires 'output_path' field in {file}")]
    MissingOutputPath { file: String },

    #[error("Failed to read template {path}: {source}")]
    TemplateReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Template rendering failed for {template}: {reason}")]
    TemplateRenderError { template: String, reason: String },

    #[error("Failed to write output file {path}: {source}")]
    WriteError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to create parent directory for {path}: {source}")]
    DirCreationError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Configuration file not found: {path}")]
    ConfigNotFound { path: PathBuf },

    #[error("Directory not found: {path}")]
    DirectoryNotFound { path: PathBuf },

    #[error("Failed to expand path '{original}': {reason}")]
    PathExpansionError { original: String, reason: String },

    #[error("Invalid category '{category}'. Valid categories: {valid:?}")]
    InvalidCategory { category: String, valid: Vec<String> },

    #[error("No TOML files found in {directory}")]
    NoFilesFound { directory: String },

    #[error("Failed to serialize to JSON: {source}")]
    JsonSerializeError {
        #[source]
        source: serde_json::Error,
    },

    #[error("Failed to serialize to TOML: {source}")]
    TomlSerializeError {
        #[source]
        source: toml::ser::Error,
    },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

/// Result type for transformation operations
pub type TransformResult<T> = Result<T, TransformError>;
