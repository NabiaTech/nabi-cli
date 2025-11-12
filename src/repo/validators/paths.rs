use super::{Severity, Validator, Violation};
use anyhow::Result;
use regex::Regex;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub struct PathValidator {
    hardcoded_pattern: Regex,
}

impl PathValidator {
    pub fn new() -> Self {
        Self {
            // Match /Users/username or /home/username patterns
            hardcoded_pattern: Regex::new(r"/Users/[a-zA-Z0-9_-]+|/home/[a-zA-Z0-9_-]+").unwrap(),
        }
    }
}

impl Validator for PathValidator {
    fn name(&self) -> &str {
        "hardcoded_paths"
    }

    fn validate(&self, repo_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();

        for entry in WalkDir::new(repo_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();

            // Skip .git directory
            if path.to_string_lossy().contains("/.git/") {
                continue;
            }

            if let Some(ext) = path.extension() {
                if matches!(
                    ext.to_str(),
                    Some("md")
                        | Some("toml")
                        | Some("yaml")
                        | Some("yml")
                        | Some("sh")
                        | Some("bash")
                        | Some("zsh")
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

impl PathValidator {
    fn check_file(&self, file_path: &Path) -> Result<Vec<Violation>> {
        let content = fs::read_to_string(file_path)?;
        let mut violations = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            // Skip example lines
            if line.contains("<!-- EXAMPLE") || line.contains("# EXAMPLE") {
                continue;
            }

            // Skip commented lines (markdown, shell, yaml)
            let trimmed = line.trim();
            if trimmed.starts_with("<!--") || trimmed.starts_with('#') {
                continue;
            }

            if let Some(matched) = self.hardcoded_pattern.find(line) {
                violations.push(Violation {
                    file_path: file_path.to_path_buf(),
                    line_number: Some(line_num + 1),
                    rule_id: "no_hardcoded_users".to_string(),
                    severity: Severity::Error,
                    message: format!("Hardcoded user path detected: {}", matched.as_str()),
                    suggestion: Some("Replace with ~/ or $HOME for portability".to_string()),
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
    fn test_detects_hardcoded_user_path() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "path: /Users/tryk/nabia/core").unwrap();

        let validator = PathValidator::new();
        let violations = validator.check_file(file.path()).unwrap();

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "no_hardcoded_users");
        assert!(violations[0].message.contains("/Users/tryk"));
    }

    #[test]
    fn test_detects_linux_home_path() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "export PATH=/home/user/bin:$PATH").unwrap();

        let validator = PathValidator::new();
        let violations = validator.check_file(file.path()).unwrap();

        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("/home/user"));
    }

    #[test]
    fn test_ignores_example_lines() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "<!-- EXAMPLE: /Users/tryk/example -->").unwrap();

        let validator = PathValidator::new();
        let violations = validator.check_file(file.path()).unwrap();

        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_ignores_commented_lines() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "# /Users/tryk/commented-path").unwrap();

        let validator = PathValidator::new();
        let violations = validator.check_file(file.path()).unwrap();

        assert_eq!(violations.len(), 0);
    }
}
