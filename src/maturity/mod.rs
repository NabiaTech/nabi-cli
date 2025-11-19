//! Maturity-based routing infrastructure for nabi-cli
//!
//! Implements language-agnostic command routing based on implementation maturity stage.
//! Prototypes validate concepts (Python/TypeScript), production formalizes (Rust).
//!
//! **Architecture Philosophy**:
//! - TOML configs declare maturity stage and implementation paths
//! - nabi-cli routes to appropriate language runtime based on stage
//! - Prototypes are valid production systems (when proven reliable)
//! - The problem is drift, not prototypes themselves
//!
//! **Maturity Progression**:
//! 1. **Prototype**: Python/TS validates concept, proves pattern works
//! 2. **Production**: Rust formalizes for type safety and performance
//! 3. **Transitioning**: Run both, compare outputs, build confidence
//!
//! **Example TOML Configuration**:
//! ```toml
//! [meta]
//! implementation_stage = "prototype"  # prototype | production | transitioning
//! implementation_language = "python"
//! implementation_path = "~/.local/share/nabi/bin/nabi-transform-validate"
//!
//! # Future production route
//! production_language = "rust"
//! production_path = "~/.nabi/src/core/nabi-cli/src/transform/schema.rs"
//! production_ready = false
//! ```

use anyhow::{Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Implementation maturity stage
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaturityStage {
    /// Prototype implementation - validates concept, proves pattern works
    /// Valid for production when proven reliable (e.g., doc-fsm)
    Prototype,

    /// Production implementation - formalized, type-safe, performance-optimized
    /// Rust implementation with comprehensive error handling
    Production,

    /// Transitioning phase - run both implementations, compare outputs
    /// Builds confidence before deprecating prototype
    Transitioning,
}

impl MaturityStage {
    /// Parse maturity stage from string
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "prototype" => Ok(Self::Prototype),
            "production" => Ok(Self::Production),
            "transitioning" => Ok(Self::Transitioning),
            _ => anyhow::bail!(
                "Invalid maturity stage: {}. Expected: prototype, production, transitioning",
                s
            ),
        }
    }

    /// Get human-readable status indicator
    pub fn status_icon(&self) -> &'static str {
        match self {
            Self::Prototype => "🧪",
            Self::Production => "🏭",
            Self::Transitioning => "🔄",
        }
    }

    /// Get color for terminal output
    pub fn color(&self) -> Color {
        match self {
            Self::Prototype => Color::Yellow,
            Self::Production => Color::Green,
            Self::Transitioning => Color::Cyan,
        }
    }
}

/// Implementation language runtime
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Python,
    Typescript,
    Rust,
    Shell,
}

impl Language {
    /// Parse language from string
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "python" => Ok(Self::Python),
            "typescript" | "ts" => Ok(Self::Typescript),
            "rust" | "rs" => Ok(Self::Rust),
            "shell" | "bash" | "sh" => Ok(Self::Shell),
            _ => anyhow::bail!(
                "Unsupported language: {}. Expected: python, typescript, rust, shell",
                s
            ),
        }
    }

    /// Get executable name for this language runtime
    pub fn executor(&self) -> &'static str {
        match self {
            Self::Python => "python3",
            Self::Typescript => "tsx",
            Self::Rust => "cargo",
            Self::Shell => "bash",
        }
    }
}

/// Implementation route configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationRoute {
    /// Current maturity stage
    pub stage: MaturityStage,

    /// Language for current implementation
    pub language: Language,

    /// Path to executable or script
    pub path: PathBuf,

    /// When this implementation was validated/deployed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validated_at: Option<String>,

    /// Production route (if different from current)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_route: Option<Box<ProductionRoute>>,
}

/// Production implementation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionRoute {
    /// Language for production implementation
    pub language: Language,

    /// Path to production executable/module
    pub path: PathBuf,

    /// Whether production implementation is ready for use
    pub ready: bool,

    /// Optional formalization deadline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
}

impl ImplementationRoute {
    /// Load implementation route from TOML meta section
    pub fn from_toml_meta(meta: &toml::Value) -> Result<Self> {
        let stage = meta
            .get("implementation_stage")
            .and_then(|v| v.as_str())
            .map(|s| MaturityStage::from_str(s))
            .transpose()?
            .context("Missing meta.implementation_stage in TOML")?;

        let language = meta
            .get("implementation_language")
            .and_then(|v| v.as_str())
            .map(|s| Language::from_str(s))
            .transpose()?
            .context("Missing meta.implementation_language in TOML")?;

        let path = meta
            .get("implementation_path")
            .and_then(|v| v.as_str())
            .map(|s| expand_path(s))
            .transpose()?
            .context("Missing meta.implementation_path in TOML")?;

        let validated_at = meta
            .get("validated_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Load production route if defined
        let production_route = if meta.get("production_language").is_some() {
            let prod_language = meta
                .get("production_language")
                .and_then(|v| v.as_str())
                .map(|s| Language::from_str(s))
                .transpose()?
                .context("Invalid production_language")?;

            let prod_path = meta
                .get("production_path")
                .and_then(|v| v.as_str())
                .map(|s| expand_path(s))
                .transpose()?
                .context("Missing production_path when production_language defined")?;

            let ready = meta
                .get("production_ready")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let deadline = meta
                .get("formalization_deadline")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            Some(Box::new(ProductionRoute {
                language: prod_language,
                path: prod_path,
                ready,
                deadline,
            }))
        } else {
            None
        };

        Ok(Self {
            stage,
            language,
            path,
            validated_at,
            production_route,
        })
    }

    /// Display implementation route information
    pub fn display_info(&self, config_name: &str) {
        println!(
            "{} {} {}",
            self.stage.status_icon(),
            "Implementation Route".bold(),
            format!("({})", config_name).dimmed()
        );
        println!(
            "  Stage: {}",
            format!("{:?}", self.stage).color(self.stage.color())
        );
        println!("  Language: {}", format!("{:?}", self.language).cyan());
        println!("  Path: {}", self.path.display().to_string().dimmed());

        if let Some(validated) = &self.validated_at {
            println!("  Validated: {}", validated.green());
        }

        if let Some(prod) = &self.production_route {
            println!();
            println!("{} {}", "📋", "Production Route (Future)".bold());
            println!("  Language: {}", format!("{:?}", prod.language).cyan());
            println!("  Path: {}", prod.path.display().to_string().dimmed());
            println!(
                "  Ready: {}",
                if prod.ready {
                    "✅ Yes".green()
                } else {
                    "⏳ Not yet".yellow()
                }
            );

            if let Some(deadline) = &prod.deadline {
                println!("  Deadline: {}", deadline.yellow());
            }
        }
    }
}

/// Command execution result
#[derive(Debug)]
pub struct ExecutionResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Route and execute command based on maturity stage
pub fn route_command(
    route: &ImplementationRoute,
    args: &[&str],
    override_stage: Option<MaturityStage>,
) -> Result<ExecutionResult> {
    let effective_stage = override_stage.as_ref().unwrap_or(&route.stage);

    match effective_stage {
        MaturityStage::Prototype => {
            println!(
                "{} {}",
                "→".dimmed(),
                format!("Routing to prototype ({:?})", route.language).dimmed()
            );
            execute_subprocess(&route.language, &route.path, args)
        }

        MaturityStage::Production => {
            let prod_route = route
                .production_route
                .as_ref()
                .context("No production route defined in TOML")?;

            if !prod_route.ready {
                anyhow::bail!(
                    "Production implementation not ready. Set production_ready=true when complete."
                );
            }

            println!(
                "{} {}",
                "→".dimmed(),
                format!("Routing to production ({:?})", prod_route.language).green()
            );
            execute_subprocess(&prod_route.language, &prod_route.path, args)
        }

        MaturityStage::Transitioning => {
            println!(
                "{}",
                "🔄 Transitioning mode: Running both implementations"
                    .cyan()
                    .bold()
            );

            // Execute prototype
            println!("{}", "  Running prototype...".dimmed());
            let prototype_result = execute_subprocess(&route.language, &route.path, args)?;

            // Execute production
            let prod_route = route
                .production_route
                .as_ref()
                .context("No production route defined for transitioning stage")?;

            println!("{}", "  Running production...".dimmed());
            let production_result =
                execute_subprocess(&prod_route.language, &prod_route.path, args)?;

            // Compare results
            compare_results(&prototype_result, &production_result)?;

            // Return production result (authoritative in transitioning)
            Ok(production_result)
        }
    }
}

/// Execute subprocess for given language runtime
fn execute_subprocess(language: &Language, path: &Path, args: &[&str]) -> Result<ExecutionResult> {
    // Verify path exists
    if !path.exists() {
        anyhow::bail!(
            "Implementation path not found: {}\nCheck TOML meta.implementation_path or meta.production_path",
            path.display()
        );
    }

    let mut cmd = Command::new(language.executor());

    // Language-specific execution patterns
    match language {
        Language::Python => {
            cmd.arg(path);
            cmd.args(args);
        }
        Language::Typescript => {
            cmd.arg(path);
            cmd.args(args);
        }
        Language::Rust => {
            // For Rust, we expect compiled binary at path
            cmd = Command::new(path);
            cmd.args(args);
        }
        Language::Shell => {
            cmd.arg(path);
            cmd.args(args);
        }
    }

    let output = cmd.output().with_context(|| {
        format!(
            "Failed to execute {} at {}",
            language.executor(),
            path.display()
        )
    })?;

    Ok(ExecutionResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

/// Compare prototype and production results
fn compare_results(prototype: &ExecutionResult, production: &ExecutionResult) -> Result<()> {
    println!();
    println!("{}", "📊 Comparison Results".bold());

    let exit_match = prototype.exit_code == production.exit_code;
    let stdout_match = prototype.stdout == production.stdout;

    println!(
        "  Exit codes: {} (prototype: {}, production: {})",
        if exit_match {
            "✅ Match".green()
        } else {
            "❌ Differ".red()
        },
        prototype.exit_code,
        production.exit_code
    );

    println!(
        "  Output: {}",
        if stdout_match {
            "✅ Match".green()
        } else {
            "⚠️  Differ".yellow()
        }
    );

    if !exit_match {
        anyhow::bail!("Exit code mismatch between prototype and production");
    }

    if !stdout_match {
        println!();
        println!(
            "{}",
            "⚠️  Output differs but exit codes match. Review differences:".yellow()
        );
        println!("{}", "  Prototype output:".dimmed());
        for line in prototype.stdout.lines().take(10) {
            println!("    {}", line.dimmed());
        }
        println!("{}", "  Production output:".dimmed());
        for line in production.stdout.lines().take(10) {
            println!("    {}", line.dimmed());
        }
    }

    Ok(())
}

/// Expand ~ and environment variables in path
fn expand_path(path: &str) -> Result<PathBuf> {
    if path.starts_with('~') {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(&path[2..]))
    } else {
        Ok(PathBuf::from(shellexpand::env(path)?.to_string()))
    }
}

#[cfg(test)]
mod tests;
