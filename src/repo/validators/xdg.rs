use super::{Severity, Validator, Violation};
use anyhow::Result;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub struct XdgValidator;

impl Validator for XdgValidator {
    fn name(&self) -> &str {
        "xdg_compliance"
    }

    fn validate(&self, repo_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();

        // Scan markdown, TOML, YAML files
        for entry in WalkDir::new(repo_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if matches!(
                    ext.to_str(),
                    Some("md") | Some("toml") | Some("yaml") | Some("yml")
                ) {
                    if let Ok(file_violations) = self.check_file(path) {
                        violations.extend(file_violations);
                    }
                }
            }
        }

        Ok(violations)
    }
}

impl XdgValidator {
    fn check_file(&self, file_path: &Path) -> Result<Vec<Violation>> {
        let content = fs::read_to_string(file_path)?;
        let mut violations = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            // Skip example lines
            if line.contains("<!-- EXAMPLE") || line.contains("# EXAMPLE") {
                continue;
            }

            // Check for hardcoded ~/.config instead of $XDG_CONFIG_HOME
            if line.contains("~/.config")
                && !line.contains("XDG_CONFIG_HOME")
                && !line.contains("${XDG_CONFIG_HOME}")
            {
                violations.push(Violation {
                    file_path: file_path.to_path_buf(),
                    line_number: Some(line_num + 1),
                    rule_id: "xdg_config".to_string(),
                    severity: Severity::Warning,
                    message: "Hardcoded ~/.config path detected".to_string(),
                    suggestion: Some(
                        "Use $XDG_CONFIG_HOME or ${XDG_CONFIG_HOME} instead".to_string(),
                    ),
                });
            }

            // Check for hardcoded ~/.cache instead of $XDG_CACHE_HOME
            if line.contains("~/.cache")
                && !line.contains("XDG_CACHE_HOME")
                && !line.contains("${XDG_CACHE_HOME}")
            {
                violations.push(Violation {
                    file_path: file_path.to_path_buf(),
                    line_number: Some(line_num + 1),
                    rule_id: "xdg_cache".to_string(),
                    severity: Severity::Warning,
                    message: "Hardcoded ~/.cache path detected".to_string(),
                    suggestion: Some(
                        "Use $XDG_CACHE_HOME or ${XDG_CACHE_HOME} instead".to_string(),
                    ),
                });
            }

            // Check for hardcoded ~/.local/share instead of $XDG_DATA_HOME
            if line.contains("~/.local/share")
                && !line.contains("XDG_DATA_HOME")
                && !line.contains("${XDG_DATA_HOME}")
            {
                violations.push(Violation {
                    file_path: file_path.to_path_buf(),
                    line_number: Some(line_num + 1),
                    rule_id: "xdg_data".to_string(),
                    severity: Severity::Warning,
                    message: "Hardcoded ~/.local/share path detected".to_string(),
                    suggestion: Some("Use $XDG_DATA_HOME or ${XDG_DATA_HOME} instead".to_string()),
                });
            }

            // Check for hardcoded ~/.local/state instead of $XDG_STATE_HOME
            if line.contains("~/.local/state")
                && !line.contains("XDG_STATE_HOME")
                && !line.contains("${XDG_STATE_HOME}")
            {
                violations.push(Violation {
                    file_path: file_path.to_path_buf(),
                    line_number: Some(line_num + 1),
                    rule_id: "xdg_state".to_string(),
                    severity: Severity::Warning,
                    message: "Hardcoded ~/.local/state path detected".to_string(),
                    suggestion: Some(
                        "Use $XDG_STATE_HOME or ${XDG_STATE_HOME} instead".to_string(),
                    ),
                });
            }
        }

        Ok(violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_detects_hardcoded_config_path() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Some text with ~/.config/nabi path").unwrap();

        let validator = XdgValidator;
        let violations = validator.check_file(file.path()).unwrap();

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "xdg_config");
    }

    #[test]
    fn test_ignores_example_lines() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "<!-- EXAMPLE: ~/.config/nabi -->").unwrap();

        let validator = XdgValidator;
        let violations = validator.check_file(file.path()).unwrap();

        assert_eq!(violations.len(), 0);
    }
}
