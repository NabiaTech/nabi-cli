use super::{Severity, Validator, Violation};
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

pub struct SymlinkValidator;

impl SymlinkValidator {
    pub fn new() -> Self {
        Self
    }

    /// Check if symlink target is hardcoded with /Users/ or /home/
    fn has_hardcoded_path(target: &str) -> bool {
        target.contains("/Users/") || target.contains("/home/")
    }

    /// Convert hardcoded path to portable format
    fn suggest_portable_path(target: &str) -> String {
        // Try to extract the user home path portion and suggest ~ replacement
        if let Some(pos) = target.find("/Users/") {
            let remainder = &target[pos + 7..]; // Skip "/Users/"
            if let Some(slash_pos) = remainder.find('/') {
                let after_user = &remainder[slash_pos..];
                return format!("~{}", after_user);
            }
        } else if let Some(pos) = target.find("/home/") {
            let remainder = &target[pos + 6..]; // Skip "/home/"
            if let Some(slash_pos) = remainder.find('/') {
                let after_user = &remainder[slash_pos..];
                return format!("~{}", after_user);
            }
        }
        "~/[path]".to_string()
    }
}

impl Validator for SymlinkValidator {
    fn name(&self) -> &str {
        "symlink_validity"
    }

    fn validate(&self, repo_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();

        for entry in WalkDir::new(repo_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_symlink())
        {
            let symlink_path = entry.path();

            // Skip .git directory
            if symlink_path.to_string_lossy().contains("/.git/") {
                continue;
            }

            // Try to read the symlink target
            match std::fs::read_link(symlink_path) {
                Ok(target) => {
                    let target_str = target.to_string_lossy().to_string();

                    // Check 1: Is the symlink target hardcoded?
                    if Self::has_hardcoded_path(&target_str) {
                        violations.push(Violation {
                            file_path: symlink_path.to_path_buf(),
                            line_number: None,
                            rule_id: "hardcoded_symlink_target".to_string(),
                            severity: Severity::Warning,
                            message: format!("Symlink target uses hardcoded path: {}", target_str),
                            suggestion: Some(format!(
                                "Use portable path: {} (e.g., as symlink source)",
                                Self::suggest_portable_path(&target_str)
                            )),
                        });
                    }

                    // Check 2: Does the symlink target actually exist?
                    if !target.exists() && !target.is_symlink() {
                        violations.push(Violation {
                            file_path: symlink_path.to_path_buf(),
                            line_number: None,
                            rule_id: "broken_symlink".to_string(),
                            severity: Severity::Error,
                            message: format!(
                                "Broken symlink: target does not exist ({})",
                                target_str
                            ),
                            suggestion: Some(
                                "Either fix the symlink target or remove this symlink".to_string(),
                            ),
                        });
                    }
                }
                Err(e) => {
                    violations.push(Violation {
                        file_path: symlink_path.to_path_buf(),
                        line_number: None,
                        rule_id: "unreadable_symlink".to_string(),
                        severity: Severity::Warning,
                        message: format!("Could not read symlink: {}", e),
                        suggestion: Some("Check symlink permissions".to_string()),
                    });
                }
            }
        }

        Ok(violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs as unix_fs;
    use tempfile::TempDir;

    #[test]
    fn test_detects_broken_symlink() {
        let tmpdir = TempDir::new().unwrap();
        let symlink_path = tmpdir.path().join("broken_link");

        // Create a symlink to a non-existent target
        unix_fs::symlink("/nonexistent/path", &symlink_path).unwrap();

        let validator = SymlinkValidator::new();
        let violations = validator.validate(tmpdir.path()).unwrap();

        assert!(!violations.is_empty());
        assert_eq!(violations[0].rule_id, "broken_symlink");
    }

    #[test]
    fn test_detects_hardcoded_path_in_symlink() {
        let tmpdir = TempDir::new().unwrap();

        // Create a real target file
        let target = tmpdir.path().join("real_target");
        std::fs::write(&target, "content").unwrap();

        // Create a symlink with hardcoded path
        let symlink_path = tmpdir.path().join("hardcoded_link");
        unix_fs::symlink("/Users/tryk/some/path", &symlink_path).unwrap();

        let validator = SymlinkValidator::new();
        let violations = validator.validate(tmpdir.path()).unwrap();

        assert!(!violations.is_empty());
        assert_eq!(violations[0].rule_id, "hardcoded_symlink_target");
    }

    #[test]
    fn test_ignores_valid_symlinks() {
        let tmpdir = TempDir::new().unwrap();

        // Create a real target
        let target = tmpdir.path().join("real_target");
        std::fs::write(&target, "content").unwrap();

        // Create a valid symlink
        let symlink_path = tmpdir.path().join("valid_link");
        unix_fs::symlink(&target, &symlink_path).unwrap();

        let validator = SymlinkValidator::new();
        let violations = validator.validate(tmpdir.path()).unwrap();

        // Should have no violations for valid symlinks
        assert!(violations.is_empty());
    }

    #[test]
    fn test_suggest_portable_path_macos() {
        let result = SymlinkValidator::suggest_portable_path("/Users/tryk/nabia/core");
        assert_eq!(result, "~/nabia/core");
    }

    #[test]
    fn test_suggest_portable_path_linux() {
        let result = SymlinkValidator::suggest_portable_path("/home/tryk/nabia/core");
        assert_eq!(result, "~/nabia/core");
    }

    #[test]
    fn test_has_hardcoded_path_users() {
        assert!(SymlinkValidator::has_hardcoded_path("/Users/tryk/path"));
        assert!(!SymlinkValidator::has_hardcoded_path("~/path"));
        assert!(!SymlinkValidator::has_hardcoded_path("$HOME/path"));
    }

    #[test]
    fn test_has_hardcoded_path_home() {
        assert!(SymlinkValidator::has_hardcoded_path("/home/tryk/path"));
        assert!(!SymlinkValidator::has_hardcoded_path("~/path"));
    }
}
