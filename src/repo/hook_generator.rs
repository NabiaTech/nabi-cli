// Hook Generator: Schema-Driven Hook Deployment
// Transforms Handlebars templates + TOML configuration → Executable hook scripts
//
// This module implements Phase 2 of the codegraph hook system:
// - Reads hook template configuration from ~/.config/nabi/codegraph.toml
// - Loads .hbs templates from ~/nabia/core/nabi-cli/scripts/hooks/
// - Renders with variables (min_disk_mb, sync_enabled, freshness_hours, etc.)
// - Deploys generated scripts to ~/.local/share/nabi/bin/ with chmod +x

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use handlebars::Handlebars;
use serde_json::{json, Value as JsonValue};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Hook deployment configuration from TOML schema
#[derive(Debug, Clone)]
pub struct HookDeployment {
    pub template_dir: PathBuf,
    pub output_dir: PathBuf,
    pub hooks: Vec<HookTemplate>,
}

/// Individual hook template definition
#[derive(Debug, Clone)]
pub struct HookTemplate {
    pub name: String,
    pub template_file: String,
    pub output_file: String,
    pub variables: JsonValue,
}

impl HookDeployment {
    /// Load hook deployment configuration from codegraph.toml
    pub fn from_toml(toml_path: &Path) -> Result<Self> {
        let toml_content = fs::read_to_string(toml_path)
            .context("Failed to read codegraph.toml")?;

        let config: toml::Value = toml::from_str(&toml_content)
            .context("Failed to parse codegraph.toml")?;

        // Resolve template directory
        let template_dir = {
            let dir_str = config
                .get("codegraph")
                .and_then(|c| c.get("hooks"))
                .and_then(|h| h.get("template_dir"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("template_dir not found in [codegraph.hooks] section"))?;

            expand_tilde(dir_str)
        };

        if !template_dir.exists() {
            return Err(anyhow!("Template directory not found: {}", template_dir.display()));
        }

        // Resolve output directory
        let output_dir = get_hooks_output_dir()?;

        // Parse hook configurations
        let mut hooks = Vec::new();

        // Pre-index hook
        if let Some(pre_index_config) = config
            .get("codegraph")
            .and_then(|c| c.get("hooks"))
            .and_then(|h| h.get("pre_index_config"))
        {
            let template = pre_index_config
                .get("template")
                .and_then(|v| v.as_str())
                .unwrap_or("codegraph-pre-index.hbs");

            let variables_json = toml_to_json(pre_index_config)?;
            hooks.push(HookTemplate {
                name: "pre_index".to_string(),
                template_file: template.to_string(),
                output_file: "codegraph-pre-index.sh".to_string(),
                variables: variables_json,
            });
        }

        // Post-index hook
        if let Some(post_index_config) = config
            .get("codegraph")
            .and_then(|c| c.get("hooks"))
            .and_then(|h| h.get("post_index_config"))
        {
            let template = post_index_config
                .get("template")
                .and_then(|v| v.as_str())
                .unwrap_or("codegraph-post-index.hbs");

            let variables_json = toml_to_json(post_index_config)?;
            hooks.push(HookTemplate {
                name: "post_index".to_string(),
                template_file: template.to_string(),
                output_file: "codegraph-post-index.sh".to_string(),
                variables: variables_json,
            });
        }

        // Pre-query hook
        if let Some(pre_query_config) = config
            .get("codegraph")
            .and_then(|c| c.get("hooks"))
            .and_then(|h| h.get("pre_query_config"))
        {
            let template = pre_query_config
                .get("template")
                .and_then(|v| v.as_str())
                .unwrap_or("codegraph-pre-query.hbs");

            let variables_json = toml_to_json(pre_query_config)?;
            hooks.push(HookTemplate {
                name: "pre_query".to_string(),
                template_file: template.to_string(),
                output_file: "codegraph-pre-query.sh".to_string(),
                variables: variables_json,
            });
        }

        if hooks.is_empty() {
            return Err(anyhow!("No hook configurations found in codegraph.toml"));
        }

        Ok(HookDeployment {
            template_dir,
            output_dir,
            hooks,
        })
    }

    /// Deploy all hooks: render templates and write to output directory
    pub fn deploy(&self, verbose: bool) -> Result<DeploymentStats> {
        let mut hb = Handlebars::new();
        let mut stats = DeploymentStats::default();

        // Ensure output directory exists
        fs::create_dir_all(&self.output_dir)
            .context("Failed to create hooks output directory")?;

        // Render each hook
        for hook in &self.hooks {
            let template_path = self.template_dir.join(&hook.template_file);

            if !template_path.exists() {
                eprintln!(
                    "⚠ Template not found: {} (skipping {})",
                    template_path.display(),
                    hook.name
                );
                stats.skipped += 1;
                continue;
            }

            // Register template
            hb.register_template_file(&hook.name, &template_path)
                .context(format!("Failed to register template: {}", hook.template_file))?;

            // Prepare variables with default values
            let mut variables = match hook.variables.as_object() {
                Some(table) => {
                    let mut map = serde_json::Map::new();
                    for (key, value) in table {
                        if key != "template" {
                            map.insert(key.clone(), value.clone());
                        }
                    }
                    map
                }
                None => serde_json::Map::new(),
            };

            // Add generated_at timestamp
            variables.insert(
                "generated_at".to_string(),
                json!(Utc::now().to_rfc3339()),
            );

            let data = serde_json::Value::Object(variables);

            // Render template
            let rendered = hb
                .render(&hook.name, &data)
                .context(format!("Failed to render hook template: {}", hook.name))?;

            // Write to output directory
            let output_path = self.output_dir.join(&hook.output_file);
            fs::write(&output_path, rendered)
                .context(format!("Failed to write hook: {}", output_path.display()))?;

            // Make executable (chmod +x)
            let permissions = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&output_path, permissions)
                .context(format!("Failed to set permissions: {}", output_path.display()))?;

            if verbose {
                println!("✓ Deployed {}: {}", hook.name, output_path.display());
            }

            stats.deployed += 1;
        }

        Ok(stats)
    }
}

/// Deployment statistics for reporting
#[derive(Debug, Default)]
pub struct DeploymentStats {
    pub deployed: usize,
    pub skipped: usize,
}

impl DeploymentStats {
    pub fn total(&self) -> usize {
        self.deployed + self.skipped
    }
}

/// Convert TOML value to JSON value for templating
fn toml_to_json(toml_value: &toml::Value) -> Result<JsonValue> {
    match toml_value {
        toml::Value::String(s) => Ok(JsonValue::String(s.clone())),
        toml::Value::Integer(i) => Ok(JsonValue::Number(
            serde_json::Number::from(*i)
        )),
        toml::Value::Float(f) => {
            if let Some(n) = serde_json::Number::from_f64(*f) {
                Ok(JsonValue::Number(n))
            } else {
                Err(anyhow!("Invalid float value: {}", f))
            }
        }
        toml::Value::Boolean(b) => Ok(JsonValue::Bool(*b)),
        toml::Value::Array(arr) => {
            let json_arr: Result<Vec<JsonValue>> = arr
                .iter()
                .map(toml_to_json)
                .collect();
            Ok(JsonValue::Array(json_arr?))
        }
        toml::Value::Table(tbl) => {
            let mut json_obj = serde_json::Map::new();
            for (k, v) in tbl.iter() {
                json_obj.insert(k.clone(), toml_to_json(v)?);
            }
            Ok(JsonValue::Object(json_obj))
        }
        _ => Err(anyhow!("Unsupported TOML value type")),
    }
}

/// Resolve XDG_DATA_HOME and return hooks directory
fn get_hooks_output_dir() -> Result<PathBuf> {
    let data_dir = if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg_data)
    } else {
        let home = std::env::var("HOME").context("HOME environment variable not set")?;
        PathBuf::from(home).join(".local").join("share")
    };

    Ok(data_dir.join("nabi").join("bin"))
}

/// Expand tilde and environment variables in path
fn expand_tilde(path: &str) -> PathBuf {
    let mut expanded = path.to_string();

    // Expand ${HOME}
    if let Ok(home) = std::env::var("HOME") {
        expanded = expanded.replace("${HOME}", &home);
    }

    // Expand ~/
    if let Some(home) = dirs::home_dir() {
        if expanded.starts_with("~/") {
            let rest = &expanded[2..];
            return home.join(rest);
        } else if expanded == "~" {
            return home;
        }
    }

    PathBuf::from(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/test");
        assert!(expanded.to_string_lossy().contains("test"));

        let absolute = expand_tilde("/absolute/path");
        assert_eq!(absolute, PathBuf::from("/absolute/path"));
    }
}
