use super::{Validator, Violation, Severity};
use std::path::{Path, PathBuf};
use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct AgentRulesConfig {
    pub rules: HashMap<String, Rule>,
    #[serde(default)]
    pub ignore_patterns: IgnorePatternsConfig,
}

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub rule_id: String,
    pub name: String,
    pub severity: String,
    pub description: String,
    pub patterns: Vec<String>,
    pub regex: bool,
    pub target_files: Vec<String>,
    #[serde(default)]
    pub migration_guidance: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct IgnorePatternsConfig {
    #[serde(default)]
    pub documentation: Vec<String>,
    #[serde(default)]
    pub comments: Vec<String>,
    #[serde(default)]
    pub clean_files: Vec<String>,
}

pub struct AgentValidator {
    config: Option<AgentRulesConfig>,
}

impl AgentValidator {
    pub fn new() -> Self {
        let config = Self::load_config().ok();
        Self { config }
    }

    fn load_config() -> Result<AgentRulesConfig> {
        let config_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join(".config/nabi/validation/agent-rules.toml");

        if !config_path.exists() {
            return Err(anyhow::anyhow!("agent-rules.toml not found at {:?}", config_path));
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: AgentRulesConfig = toml::from_str(&content)?;
        Ok(config)
    }

    fn should_ignore_file(&self, file_path: &Path, config: &AgentRulesConfig) -> bool {
        let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();
        
        // Check clean_files list
        for pattern in &config.ignore_patterns.clean_files {
            if Self::glob_match(&file_name, pattern) {
                return true;
            }
        }
        false
    }

    fn glob_match(file_name: &str, pattern: &str) -> bool {
        // Simple glob matching for * wildcard
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            let mut pos = 0;
            
            for (i, part) in parts.iter().enumerate() {
                if part.is_empty() {
                    continue;
                }
                
                if i == 0 && !pattern.starts_with('*') {
                    if !file_name.starts_with(part) {
                        return false;
                    }
                    pos = part.len();
                } else if i == parts.len() - 1 && !pattern.ends_with('*') {
                    if !file_name[pos..].ends_with(part) {
                        return false;
                    }
                } else if let Some(found_pos) = file_name[pos..].find(part) {
                    pos += found_pos + part.len();
                } else {
                    return false;
                }
            }
            true
        } else {
            file_name == pattern
        }
    }

    fn should_skip_line(&self, line: &str) -> bool {
        let trimmed = line.trim();
        
        // Skip comment lines
        trimmed.starts_with('#') 
            || trimmed.starts_with("//")
            || trimmed.starts_with("<!--")
            || trimmed.starts_with("```")
            || trimmed.starts_with("---")
    }

    fn severity_to_enum(&self, severity_str: &str) -> Severity {
        match severity_str {
            "HIGH" => Severity::Critical,
            "MEDIUM" => Severity::Warning,
            "LOW-MEDIUM" => Severity::Warning,
            _ => Severity::Info,
        }
    }

    fn get_command_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(&home).join(".claude/commands"));
            dirs.push(PathBuf::from(&home).join(".xdg/claude-config/commands"));
        }
        
        dirs
    }

    fn check_file(&self, file_path: &Path, config: &AgentRulesConfig) -> Result<Vec<Violation>> {
        let content = fs::read_to_string(file_path)?;
        let mut violations = Vec::new();

        for (_rule_key, rule) in &config.rules {
            // Check if file matches target_files patterns
            let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();
            let matches_target = rule.target_files.iter().any(|pattern| {
                Self::glob_match(&file_name, pattern)
            });

            if !matches_target {
                continue;
            }

            // Apply patterns
            for pattern_str in &rule.patterns {
                let violations_for_pattern = if rule.regex {
                    self.check_regex_pattern(file_path, &content, pattern_str, rule)?
                } else {
                    self.check_literal_pattern(file_path, &content, pattern_str, rule)?
                };
                violations.extend(violations_for_pattern);
            }
        }

        Ok(violations)
    }

    fn check_literal_pattern(
        &self,
        file_path: &Path,
        content: &str,
        pattern: &str,
        rule: &Rule,
    ) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if self.should_skip_line(line) {
                continue;
            }

            if line.contains(pattern) {
                violations.push(Violation {
                    file_path: file_path.to_path_buf(),
                    line_number: Some(line_num + 1),
                    rule_id: rule.rule_id.clone(),
                    severity: self.severity_to_enum(&rule.severity),
                    message: format!("{}: {}", rule.name, rule.description),
                    suggestion: if !rule.migration_guidance.is_empty() {
                        Some(rule.migration_guidance.lines().next().unwrap_or("").to_string())
                    } else {
                        None
                    },
                });
            }
        }

        Ok(violations)
    }

    fn check_regex_pattern(
        &self,
        file_path: &Path,
        content: &str,
        pattern_str: &str,
        rule: &Rule,
    ) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();

        match Regex::new(pattern_str) {
            Ok(regex) => {
                for (line_num, line) in content.lines().enumerate() {
                    if self.should_skip_line(line) {
                        continue;
                    }

                    if regex.is_match(line) {
                        violations.push(Violation {
                            file_path: file_path.to_path_buf(),
                            line_number: Some(line_num + 1),
                            rule_id: rule.rule_id.clone(),
                            severity: self.severity_to_enum(&rule.severity),
                            message: format!("{}: {}", rule.name, rule.description),
                            suggestion: if !rule.migration_guidance.is_empty() {
                                Some(rule.migration_guidance.lines().next().unwrap_or("").to_string())
                            } else {
                                None
                            },
                        });
                    }
                }
            }
            Err(e) => {
                eprintln!("Invalid regex pattern '{}': {}", pattern_str, e);
            }
        }

        Ok(violations)
    }
}

impl Validator for AgentValidator {
    fn name(&self) -> &str {
        "agent_commands"
    }

    fn validate(&self, _repo_path: &Path) -> Result<Vec<Violation>> {
        let config = match &self.config {
            Some(c) => c,
            None => return Ok(Vec::new()), // No config = no violations
        };

        let mut violations = Vec::new();
        let command_dirs = Self::get_command_dirs();

        for dir in command_dirs {
            if !dir.exists() {
                continue;
            }

            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if self.should_ignore_file(&path, config) {
                        continue;
                    }

                    if let Some(ext) = path.extension() {
                        if matches!(ext.to_str(), Some("md") | Some("yaml") | Some("yml") | Some("sh")) {
                            if let Ok(mut file_violations) = self.check_file(&path, config) {
                                violations.append(&mut file_violations);
                            }
                        }
                    }
                }
            }
        }

        // Sort violations by file path, then line number
        violations.sort_by(|a, b| {
            a.file_path.cmp(&b.file_path)
                .then_with(|| a.line_number.cmp(&b.line_number))
        });

        Ok(violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_exact() {
        assert!(AgentValidator::glob_match("atomic.md", "atomic.md"));
        assert!(!AgentValidator::glob_match("atomic.md", "bootstrap.md"));
    }

    #[test]
    fn test_glob_match_wildcard() {
        assert!(AgentValidator::glob_match("atomic.md", "*.md"));
        assert!(AgentValidator::glob_match("command.yaml", "*.yaml"));
        assert!(!AgentValidator::glob_match("command.yaml", "*.md"));
    }

    #[test]
    fn test_glob_match_prefix_wildcard() {
        assert!(AgentValidator::glob_match("nabi-cli-docs-search.md", "nabi-*.md"));
        assert!(AgentValidator::glob_match("fumadoc.md", "*doc.md"));
    }

    #[test]
    fn test_should_skip_line() {
        let validator = AgentValidator::new();
        assert!(validator.should_skip_line("# This is a comment"));
        assert!(validator.should_skip_line("// Code comment"));
        assert!(validator.should_skip_line("<!-- HTML comment -->"));
        assert!(validator.should_skip_line("```"));
        assert!(!validator.should_skip_line("Some actual content"));
    }

    #[test]
    fn test_severity_mapping() {
        let validator = AgentValidator::new();
        assert_eq!(validator.severity_to_enum("HIGH"), Severity::Critical);
        assert_eq!(validator.severity_to_enum("MEDIUM"), Severity::Warning);
        assert_eq!(validator.severity_to_enum("LOW-MEDIUM"), Severity::Warning);
    }
}
