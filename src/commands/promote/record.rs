// Promotion Record Module
// Manages promotion_record.json files in ~/.local/state/nabi/promoted/

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::PromotionMode;

/// Promotion record matching promotion-record.schema.json
#[derive(Debug, Serialize, Deserialize)]
pub struct PromotionRecord {
    pub record_id: String,
    pub tool_id: String,
    pub version: String,
    pub promoted_at: String,
    pub mode: PromotionMode,
    pub artifacts: Vec<ArtifactRecord>,
    pub source: SourceRecord,
    pub version_aliases: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
    pub rollback: Option<RollbackInfo>,
    pub validation: Option<ValidationInfo>,
    pub status: PromotionStatus,
    pub superseded_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub artifact_type: String,
    pub source_path: String,
    pub target_path: String,
    pub checksum: String,
    pub size_bytes: Option<u64>,
    pub permissions: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceRecord {
    pub repository: String,
    pub commit: String,
    pub branch: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RollbackInfo {
    pub previous_version: Option<String>,
    pub previous_record_id: Option<String>,
    pub rollback_safe: bool,
    pub archived_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationInfo {
    pub schema_valid: bool,
    pub tests_passed: Option<bool>,
    pub dependencies_met: Option<bool>,
    pub xdg_compliant: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromotionStatus {
    Active,
    Superseded,
    RolledBack,
    Archived,
}

/// Generate promotion record ID
pub fn generate_record_id() -> String {
    let now = Utc::now();
    let timestamp = now.format("%Y%m%d-%H%M%S");
    let random_hex: String = (0..8)
        .map(|_| format!("{:x}", rand::random::<u8>() % 16))
        .collect();
    format!("promo-{}-{}", timestamp, random_hex)
}

/// Write promotion record to state directory
pub fn write_promotion_record(tool_id: &str, record: &PromotionRecord) -> Result<String> {
    let record_id = generate_record_id();

    // Clone and update record with generated ID
    let mut final_record = serde_json::to_value(record)?;
    if let Some(obj) = final_record.as_object_mut() {
        obj.insert("record_id".to_string(), serde_json::Value::String(record_id.clone()));
    }

    // Determine output path
    let state_dir = expand_path("~/.local/state/nabi/promoted");
    fs::create_dir_all(&state_dir)
        .context("Failed to create promoted state directory")?;

    let output_path = state_dir.join(format!("{}.json", tool_id));

    // Write JSON
    let json = serde_json::to_string_pretty(&final_record)
        .context("Failed to serialize promotion record")?;
    fs::write(&output_path, json)
        .context(format!("Failed to write promotion record to {}", output_path.display()))?;

    Ok(record_id)
}

/// Expand ~ in paths
fn expand_path(path: &str) -> PathBuf {
    let expanded = shellexpand::tilde(path);
    PathBuf::from(expanded.as_ref())
}
