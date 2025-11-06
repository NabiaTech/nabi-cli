use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct PathRulesConfig {
    pub ignore_patterns: IgnorePatterns,
}

#[derive(Debug, Deserialize)]
pub struct IgnorePatterns {
    pub historical_reports: Vec<String>,
    pub migration_docs: Vec<String>,
    pub setup_guides: Vec<String>,
    pub generated_files: Vec<String>,
}

impl PathRulesConfig {
    pub fn load() -> Result<Self> {
        let config_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join(".config/nabi/validation/path-rules.toml");

        if !config_path.exists() {
            // Return default config if file doesn't exist
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: PathRulesConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Check if a file path should be ignored based on configured patterns
    pub fn should_ignore(&self, file_path: &Path) -> bool {
        let path_str = file_path.to_string_lossy();

        // Collect all patterns into a single vector
        let all_patterns: Vec<&String> = self.ignore_patterns.historical_reports.iter()
            .chain(self.ignore_patterns.migration_docs.iter())
            .chain(self.ignore_patterns.setup_guides.iter())
            .chain(self.ignore_patterns.generated_files.iter())
            .collect();

        // Check if path matches any pattern
        all_patterns.iter().any(|pattern| {
            Self::matches_glob_pattern(&path_str, pattern)
        })
    }

    /// Simple glob pattern matcher supporting * and ** wildcards
    fn matches_glob_pattern(path: &str, pattern: &str) -> bool {
        // Handle ** (matches any number of directory levels)
        if pattern.contains("**") {
            let parts: Vec<&str> = pattern.split("**").collect();

            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1].trim_start_matches('/');

                // Check if path contains the pattern
                if !prefix.is_empty() && !path.contains(prefix) {
                    return false;
                }

                if !suffix.is_empty() {
                    // Convert suffix glob to simple pattern matching
                    let suffix_pattern = suffix.replace('*', "");
                    return path.contains(&suffix_pattern);
                }

                return true;
            }
        }

        // Handle single * (matches within a path segment)
        if pattern.contains('*') {
            let pattern_parts: Vec<&str> = pattern.split('*').collect();

            // For patterns like "*VALIDATION*.md"
            if pattern_parts.len() >= 2 {
                let mut search_from = 0;

                for (i, part) in pattern_parts.iter().enumerate() {
                    if part.is_empty() {
                        continue;
                    }

                    if i == 0 && !pattern.starts_with('*') {
                        // First part must match at start
                        if !path.starts_with(part) {
                            return false;
                        }
                        search_from = part.len();
                    } else if i == pattern_parts.len() - 1 && !pattern.ends_with('*') {
                        // Last part must match at end
                        return path.ends_with(part);
                    } else {
                        // Middle parts must be found in order
                        if let Some(pos) = path[search_from..].find(part) {
                            search_from += pos + part.len();
                        } else {
                            return false;
                        }
                    }
                }

                return true;
            }
        }

        // Exact match as fallback
        path.contains(pattern)
    }
}

impl Default for PathRulesConfig {
    fn default() -> Self {
        Self {
            ignore_patterns: IgnorePatterns {
                historical_reports: vec![],
                migration_docs: vec![],
                setup_guides: vec![],
                generated_files: vec![],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_glob_pattern_wildcard() {
        assert!(PathRulesConfig::matches_glob_pattern(
            "docs/VALIDATION_RESULTS.md",
            "*VALIDATION*.md"
        ));

        assert!(PathRulesConfig::matches_glob_pattern(
            "PATH_VALIDATION_SUMMARY.md",
            "*VALIDATION*.md"
        ));

        assert!(!PathRulesConfig::matches_glob_pattern(
            "docs/README.md",
            "*VALIDATION*.md"
        ));
    }

    #[test]
    fn test_matches_glob_pattern_double_wildcard() {
        assert!(PathRulesConfig::matches_glob_pattern(
            "docs/migration/guide.md",
            "docs/migration/**"
        ));

        assert!(PathRulesConfig::matches_glob_pattern(
            "build/dist/output.js",
            "**/dist/**"
        ));
    }

    #[test]
    fn test_should_ignore() {
        let config = PathRulesConfig {
            ignore_patterns: IgnorePatterns {
                historical_reports: vec!["*VALIDATION*.md".to_string()],
                migration_docs: vec!["*MIGRATION*.md".to_string()],
                setup_guides: vec![],
                generated_files: vec!["**/node_modules/**".to_string()],
            },
        };

        assert!(config.should_ignore(Path::new("docs/VALIDATION_RESULTS.md")));
        assert!(config.should_ignore(Path::new("MIGRATION_GUIDE.md")));
        assert!(config.should_ignore(Path::new("src/node_modules/package/index.js")));
        assert!(!config.should_ignore(Path::new("docs/README.md")));
    }
}
