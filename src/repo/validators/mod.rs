use anyhow::Result;
use std::path::{Path, PathBuf};

pub mod paths;
pub mod symlinks;
pub mod xdg;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone)]
pub struct Violation {
    pub file_path: PathBuf,
    pub line_number: Option<usize>,
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub suggestion: Option<String>,
}

pub struct ComplianceReport {
    pub repo_path: PathBuf,
    pub violations: Vec<Violation>,
    pub total_files_scanned: usize,
}

pub trait Validator {
    fn name(&self) -> &str;
    fn validate(&self, repo_path: &Path) -> Result<Vec<Violation>>;
}
