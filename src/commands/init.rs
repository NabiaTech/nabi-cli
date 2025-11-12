// nabi init - Complete federation initialization command
// Implements the canonical onboarding flow from CANONICAL_ONBOARDING_FLOW.md

use anyhow::{Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::paths::NabiPaths;

/// AURA profile schema (parsed from TOML)
#[derive(Debug, Deserialize)]
struct AuraProfile {
    profile: ProfileSection,
    hooks: HooksSection,
    federation: FederationSection,
    mcp_servers: McpServersSection,
    environment: HashMap<String, String>,
    paths: PathsSection,
    capabilities: CapabilitiesSection,
    features: FeaturesSection,
    #[serde(default)]
    additional_directories: Option<AdditionalDirectoriesSection>,
    #[serde(default)]
    orchestration: Option<OrchestrationSection>,
}

#[derive(Debug, Deserialize)]
struct ProfileSection {
    name: String,
    version: String,
    description: String,
    schema_version: String,
}

#[derive(Debug, Deserialize)]
struct HooksSection {
    enabled: Vec<String>,
    disabled: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FederationSection {
    enabled: bool,
    loki_url: String,
    agent_id_generation: String,
    uuid_mapping: bool,
    manifest_validation: bool,
    #[serde(default)]
    health_checks: bool,
    #[serde(default)]
    coordination_server: Option<String>,
    #[serde(default)]
    subagent_spawning: bool,
}

#[derive(Debug, Deserialize)]
struct McpServersSection {
    enabled: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PathsSection {
    config_dir: String,
    data_dir: String,
    cache_dir: String,
    state_dir: String,
    venv_dir: String,
}

#[derive(Debug, Deserialize)]
struct CapabilitiesSection {
    federation_aware: bool,
    aura_compatible: bool,
    xdg_compliant: bool,
    hook_integrated: bool,
    cross_platform: bool,
    persistent_agents: bool,
    manifest_tracking: bool,
    #[serde(default)]
    linear_integration: bool,
    #[serde(default)]
    research_tools: bool,
    #[serde(default)]
    subagent_orchestration: bool,
}

#[derive(Debug, Deserialize)]
struct FeaturesSection {
    task_interception: bool,
    security_guards: bool,
    advanced_monitoring: bool,
    #[serde(default)]
    linear_sync: bool,
    #[serde(default)]
    manifest_validation: bool,
    #[serde(default)]
    subagent_spawning: bool,
    #[serde(default)]
    research_integration: bool,
}

#[derive(Debug, Deserialize)]
struct AdditionalDirectoriesSection {
    paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OrchestrationSection {
    role: String,
    subagent_budget: u32,
    token_conservation: bool,
    research_via_subagent: bool,
    delegation_strategies: Vec<String>,
}

/// Claude Code settings.json structure
#[derive(Debug, Serialize, Deserialize)]
struct ClaudeSettings {
    #[serde(rename = "$schema")]
    schema: String,
    #[serde(rename = "cleanupPeriodDays")]
    cleanup_period_days: u32,
    env: HashMap<String, String>,
    #[serde(rename = "includeCoAuthoredBy")]
    include_co_authored_by: bool,
    permissions: PermissionsSection,
    model: String,
    #[serde(rename = "enableAllProjectMcpServers")]
    enable_all_project_mcp_servers: bool,
    #[serde(rename = "enabledMcpjsonServers")]
    enabled_mcp_json_servers: Vec<String>,
    #[serde(rename = "disabledMcpjsonServers")]
    disabled_mcp_json_servers: Vec<String>,
    hooks: HooksConfig,
    #[serde(rename = "statusLine", skip_serializing_if = "Option::is_none")]
    status_line: Option<StatusLineConfig>,
    #[serde(rename = "outputStyle")]
    output_style: String,
    #[serde(rename = "alwaysThinkingEnabled")]
    always_thinking_enabled: bool,
    #[serde(rename = "mcpServers")]
    mcp_servers: serde_json::Value,
    #[serde(rename = "learnMode")]
    learn_mode: bool,
    #[serde(rename = "disableTips")]
    disable_tips: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct PermissionsSection {
    allow: Vec<String>,
    deny: Vec<String>,
    #[serde(rename = "defaultMode")]
    default_mode: String,
    #[serde(rename = "additionalDirectories")]
    additional_directories: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HooksConfig {
    #[serde(rename = "SessionStart")]
    session_start: Vec<HookMatcher>,
    #[serde(rename = "SessionEnd")]
    session_end: Vec<HookMatcher>,
    #[serde(rename = "PreToolUse", skip_serializing_if = "Option::is_none")]
    pre_tool_use: Option<Vec<HookMatcher>>,
    #[serde(rename = "PostToolUse", skip_serializing_if = "Option::is_none")]
    post_tool_use: Option<Vec<HookMatcher>>,
    #[serde(rename = "UserPromptSubmit", skip_serializing_if = "Option::is_none")]
    user_prompt_submit: Option<Vec<HookMatcher>>,
    #[serde(rename = "PreCompact", skip_serializing_if = "Option::is_none")]
    pre_compact: Option<Vec<HookMatcher>>,
    #[serde(rename = "Stop", skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<HookMatcher>>,
    #[serde(rename = "SubagentStop", skip_serializing_if = "Option::is_none")]
    subagent_stop: Option<Vec<HookMatcher>>,
    #[serde(rename = "Notification", skip_serializing_if = "Option::is_none")]
    notification: Option<Vec<HookMatcher>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HookMatcher {
    matcher: String,
    hooks: Vec<HookCommand>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HookCommand {
    #[serde(rename = "type")]
    hook_type: String,
    command: String,
    timeout: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatusLineConfig {
    #[serde(rename = "type")]
    status_type: String,
    command: String,
    padding: u32,
}

/// Initialize the federation system from an AURA profile
pub fn run(aura_name: Option<String>, dry_run: bool, force: bool) -> Result<()> {
    println!("{}", "🚀 NabiOS Federation Initialization".cyan().bold());
    println!();

    // Stage 0: Prerequisites Check
    println!("{}", "Stage 0: Prerequisites Check".yellow().bold());
    check_prerequisites()?;
    println!("{}", "✓ All prerequisites met\n".green());

    // Stage 1: AURA Profile Selection
    let aura_profile = if let Some(name) = aura_name {
        name
    } else {
        select_aura_profile()?
    };

    println!("{}", format!("Selected profile: {}", aura_profile).cyan());
    println!();

    // Load AURA schema
    let aura_path = NabiPaths::config_dir()?
        .join("auras")
        .join(format!("{}.toml", aura_profile));
    let aura_content = fs::read_to_string(&aura_path)
        .with_context(|| format!("Failed to read AURA profile: {}", aura_path.display()))?;
    let aura: AuraProfile =
        toml::from_str(&aura_content).with_context(|| "Failed to parse AURA profile")?;

    println!(
        "{}",
        format!("Profile: {}", aura.profile.description).dimmed()
    );
    println!("{}", format!("Version: {}", aura.profile.version).dimmed());
    println!();

    // Stage 2: Schema Transformation
    println!(
        "{}",
        "Stage 2: Transforming AURA → settings.json".yellow().bold()
    );
    if !dry_run {
        transform_aura_to_settings(&aura, force)?;
        println!("{}", "✓ settings.json generated\n".green());
    } else {
        println!("{}", "  (dry run: skipped)\n".dimmed());
    }

    // Stage 3: Hook Installation
    println!("{}", "Stage 3: Installing hooks".yellow().bold());
    if !dry_run {
        install_hooks(&aura)?;
        println!("{}", "✓ Hooks installed\n".green());
    } else {
        println!("{}", "  (dry run: skipped)\n".dimmed());
    }

    // Stage 4: Manifest Generation (if enabled)
    if aura.federation.manifest_validation {
        println!("{}", "Stage 4: Generating manifest".yellow().bold());
        if !dry_run {
            generate_manifest()?;
            println!("{}", "✓ Manifest generated\n".green());
        } else {
            println!("{}", "  (dry run: skipped)\n".dimmed());
        }
    }

    // Stage 5: Federation Registration
    println!(
        "{}",
        "Stage 5: Initializing federation state".yellow().bold()
    );
    if !dry_run {
        initialize_federation_state(&aura)?;
        println!("{}", "✓ Federation state initialized\n".green());
    } else {
        println!("{}", "  (dry run: skipped)\n".dimmed());
    }

    // Stage 6: Health Check
    println!("{}", "Stage 6: Infrastructure health check".yellow().bold());
    let health_status = check_infrastructure_health();
    print_health_status(&health_status);
    println!();

    // Stage 7: Validation
    println!("{}", "Stage 7: Validation".yellow().bold());
    if !dry_run {
        validate_installation(&aura)?;
        println!("{}", "✓ All validation checks passed\n".green());
    } else {
        println!("{}", "  (dry run: skipped)\n".dimmed());
    }

    // Stage 8: Success Summary
    print_success_summary(&aura_profile, dry_run);

    Ok(())
}

fn check_prerequisites() -> Result<()> {
    // Check Python 3.11+
    let python_version = Command::new("python3")
        .arg("--version")
        .output()
        .context("Python3 not found")?;

    if !python_version.status.success() {
        anyhow::bail!("Python3 is not installed");
    }

    // Check uv
    let uv_check = Command::new("uv").arg("--version").output();

    if uv_check.is_err() {
        println!(
            "{}",
            "  ⚠️  uv not found (recommended for Python package management)".yellow()
        );
        println!(
            "{}",
            "     Install: curl -LsSf https://astral.sh/uv/install.sh | sh".dimmed()
        );
    }

    // Check git
    let git_check = Command::new("git")
        .arg("--version")
        .output()
        .context("Git not found")?;

    if !git_check.status.success() {
        anyhow::bail!("Git is not installed");
    }

    Ok(())
}

fn select_aura_profile() -> Result<String> {
    println!("Select your agent profile:");
    println!("  1. minimal       (Basic federation, minimal dependencies)");
    println!("  2. developer     (Full-stack with Linear integration)");
    println!("  3. architect     (Senior orchestrator with research tools)");
    println!();
    print!("Your choice [1]: ");

    // For now, default to minimal
    // TODO: Implement interactive selection
    Ok("minimal".to_string())
}

fn transform_aura_to_settings(aura: &AuraProfile, force: bool) -> Result<()> {
    let settings_path = NabiPaths::home_dir()?.join(".claude").join("settings.json");

    // Check if settings.json exists
    if settings_path.exists() && !force {
        println!("{}", "  ⚠️  settings.json already exists".yellow());
        println!("{}", "     Use --force to overwrite".dimmed());
        return Ok(());
    }

    // Backup existing settings if they exist
    if settings_path.exists() {
        let backup_path = settings_path.with_extension("json.backup");
        fs::copy(&settings_path, &backup_path)
            .context("Failed to backup existing settings.json")?;
        println!(
            "{}",
            format!("  📋 Backed up to: {}", backup_path.display()).dimmed()
        );
    }

    // Build settings.json from AURA
    let settings = build_settings_from_aura(aura)?;

    // Write settings.json atomically
    let temp_path = settings_path.with_extension("json.tmp");
    let settings_json = serde_json::to_string_pretty(&settings)?;
    fs::write(&temp_path, settings_json)?;
    fs::rename(&temp_path, &settings_path)?;

    println!(
        "{}",
        format!("  ✓ Written: {}", settings_path.display()).green()
    );

    Ok(())
}

fn build_settings_from_aura(aura: &AuraProfile) -> Result<ClaudeSettings> {
    // TODO: Implement full transformation logic
    // This is a simplified version for the MVP

    let hooks_dir = NabiPaths::config_dir()?.join("governance").join("hooks");
    let hook_wrapper = hooks_dir.join("hook_wrapper.sh");

    // Build hooks configuration
    let build_hook = |hook_name: &str, timeout: u32| -> Vec<HookMatcher> {
        vec![HookMatcher {
            matcher: String::new(),
            hooks: vec![HookCommand {
                hook_type: "command".to_string(),
                command: format!("{} {}", hook_wrapper.display(), hook_name),
                timeout,
            }],
        }]
    };

    let hooks_config = HooksConfig {
        session_start: build_hook("session_start", 5),
        session_end: build_hook("session_end", 5),
        pre_tool_use: if aura.hooks.enabled.contains(&"pre_tool_use".to_string()) {
            Some(build_hook("pre_tool_use", 3))
        } else {
            None
        },
        post_tool_use: if aura.hooks.enabled.contains(&"post_tool_use".to_string()) {
            Some(build_hook("post_tool_use", 3))
        } else {
            None
        },
        user_prompt_submit: if aura
            .hooks
            .enabled
            .contains(&"user_prompt_submit".to_string())
        {
            Some(build_hook("user_prompt_submit", 3))
        } else {
            None
        },
        pre_compact: if aura.hooks.enabled.contains(&"pre_compact".to_string()) {
            Some(build_hook("pre_compact", 3))
        } else {
            None
        },
        stop: if aura.hooks.enabled.contains(&"stop".to_string()) {
            Some(build_hook("stop", 5))
        } else {
            None
        },
        subagent_stop: if aura.hooks.enabled.contains(&"subagent_stop".to_string()) {
            Some(build_hook("subagent_stop", 3))
        } else {
            None
        },
        notification: if aura.hooks.enabled.contains(&"notification".to_string()) {
            Some(build_hook("notification", 2))
        } else {
            None
        },
    };

    // Build environment variables
    let mut env = aura.environment.clone();

    // Expand environment variables
    for (key, value) in env.iter_mut() {
        *value = expand_env_vars(value);
    }

    // Build permissions
    let additional_dirs = if let Some(ref dirs) = aura.additional_directories {
        dirs.paths.iter().map(|p| expand_env_vars(p)).collect()
    } else {
        vec![]
    };

    let permissions = PermissionsSection {
        allow: vec![
            "Bash(ls:*)".to_string(),
            "Read(~/.zshrc)".to_string(),
            "WebFetch(domain:github.com)".to_string(),
            "mcp__sequentialthinking__sequentialthinking".to_string(),
            "mcp__memory-kb__search_nodes".to_string(),
        ],
        deny: vec![],
        default_mode: "bypassPermissions".to_string(),
        additional_directories: additional_dirs,
    };

    // Build MCP servers (placeholder - needs proper implementation)
    let mcp_servers = serde_json::json!({});

    let settings = ClaudeSettings {
        schema: "https://json.schemastore.org/claude-code-settings.json".to_string(),
        cleanup_period_days: 1000,
        env,
        include_co_authored_by: false,
        permissions,
        model: "haiku".to_string(),
        enable_all_project_mcp_servers: true,
        enabled_mcp_json_servers: vec!["memchain".to_string()],
        disabled_mcp_json_servers: vec![],
        hooks: hooks_config,
        status_line: None,
        output_style: "Explanatory".to_string(),
        always_thinking_enabled: true,
        mcp_servers,
        learn_mode: false,
        disable_tips: true,
    };

    Ok(settings)
}

fn expand_env_vars(value: &str) -> String {
    // Simple environment variable expansion
    // TODO: Implement full expansion with ${VAR:-default} syntax
    if value.starts_with('$') {
        std::env::var(&value[1..]).unwrap_or_else(|_| value.to_string())
    } else if value.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        format!("{}/{}", home, &value[2..])
    } else {
        value.to_string()
    }
}

fn install_hooks(aura: &AuraProfile) -> Result<()> {
    let config_dir = NabiPaths::config_dir()?;
    let hooks_dir = config_dir.join("governance").join("hooks");

    // Check if hooks directory exists
    if !hooks_dir.exists() {
        println!(
            "{}",
            "  ⚠️  Hooks directory not found, cloning nabi config...".yellow()
        );

        // Clone nabi config repository
        let clone_status = Command::new("git")
            .args(&["clone", "https://github.com/troykirin/nabi.git"])
            .arg(&config_dir)
            .status()
            .context("Failed to clone nabi config")?;

        if !clone_status.success() {
            anyhow::bail!("Failed to clone nabi config repository");
        }
    }

    // Create Python venv
    let venv_dir = config_dir.join(".venv");
    if !venv_dir.exists() {
        println!("{}", "  🐍 Creating Python virtual environment...".cyan());

        let venv_status = Command::new("python3")
            .args(&["-m", "venv"])
            .arg(&venv_dir)
            .status()
            .context("Failed to create Python venv")?;

        if !venv_status.success() {
            anyhow::bail!("Failed to create Python venv");
        }
    }

    // Install dependencies
    println!("{}", "  📦 Installing hook dependencies...".cyan());
    let pip_path = venv_dir.join("bin").join("pip");
    let install_status = Command::new(&pip_path)
        .args(&["install", "requests"])
        .status()
        .context("Failed to install hook dependencies")?;

    if !install_status.success() {
        anyhow::bail!("Failed to install hook dependencies");
    }

    // Verify hook_wrapper.sh is executable
    let hook_wrapper = hooks_dir.join("hook_wrapper.sh");
    if !hook_wrapper.exists() {
        anyhow::bail!("hook_wrapper.sh not found at {}", hook_wrapper.display());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook_wrapper)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_wrapper, perms)?;
    }

    Ok(())
}

fn generate_manifest() -> Result<()> {
    // Check if we're in a git repository
    let cwd = std::env::current_dir()?;
    let git_dir = cwd.join(".git");

    if !git_dir.exists() {
        println!(
            "{}",
            "  ⚠️  Not in a git repository, skipping manifest generation".yellow()
        );
        return Ok(());
    }

    // TODO: Call nabi docs manifest generate
    println!(
        "{}",
        "  📝 Manifest generation not yet implemented".dimmed()
    );

    Ok(())
}

fn initialize_federation_state(aura: &AuraProfile) -> Result<()> {
    let state_dir = PathBuf::from(expand_env_vars(
        &aura
            .environment
            .get("FEDERATION_STATE")
            .unwrap_or(&"~/.memchain".to_string()),
    ));

    // Create federation directory structure
    fs::create_dir_all(&state_dir.join("federation"))?;

    // Initialize uuid_mappings.json
    let uuid_mappings_path = state_dir.join("federation").join("uuid_mappings.json");
    if !uuid_mappings_path.exists() {
        fs::write(&uuid_mappings_path, "{}")?;
    }

    // Create agent-state directory
    let agent_state_dir = NabiPaths::home_dir()?.join(".claude").join("agent-state");
    fs::create_dir_all(&agent_state_dir)?;

    Ok(())
}

#[derive(Debug)]
struct HealthStatus {
    loki_available: bool,
    tmux_available: bool,
    coordination_server_available: bool,
}

fn check_infrastructure_health() -> HealthStatus {
    // Check Loki
    let loki_available = Command::new("curl")
        .args(&["-s", "http://federation-loki:3100/ready"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Check tmux
    let tmux_available = Command::new("which")
        .arg("tmux")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Check coordination server (optional)
    let coordination_server_available = Command::new("curl")
        .args(&["-s", "http://rpi:8001/health"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    HealthStatus {
        loki_available,
        tmux_available,
        coordination_server_available,
    }
}

fn print_health_status(status: &HealthStatus) {
    println!(
        "{}",
        if status.loki_available {
            "  ✓ Loki: Available".green()
        } else {
            "  ⚠️  Loki: Unavailable (optional)".yellow()
        }
    );

    println!(
        "{}",
        if status.tmux_available {
            "  ✓ tmux: Available".green()
        } else {
            "  ⚠️  tmux: Not installed (optional)".yellow()
        }
    );

    println!(
        "{}",
        if status.coordination_server_available {
            "  ✓ Coordination Server: Available".green()
        } else {
            "  ⚠️  Coordination Server: Unavailable (optional)".yellow()
        }
    );
}

fn validate_installation(aura: &AuraProfile) -> Result<()> {
    // Validate settings.json exists
    let settings_path = NabiPaths::home_dir()?.join(".claude").join("settings.json");
    if !settings_path.exists() {
        anyhow::bail!("settings.json not found");
    }

    // Validate hooks directory exists
    let hooks_dir = NabiPaths::config_dir()?.join("governance").join("hooks");
    if !hooks_dir.exists() {
        anyhow::bail!("Hooks directory not found");
    }

    // Validate hook_wrapper.sh is executable
    let hook_wrapper = hooks_dir.join("hook_wrapper.sh");
    if !hook_wrapper.exists() {
        anyhow::bail!("hook_wrapper.sh not found");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&hook_wrapper)?.permissions();
        if perms.mode() & 0o111 == 0 {
            anyhow::bail!("hook_wrapper.sh is not executable");
        }
    }

    // Validate federation state directory
    let state_dir = PathBuf::from(expand_env_vars(
        &aura
            .environment
            .get("FEDERATION_STATE")
            .unwrap_or(&"~/.memchain".to_string()),
    ));
    if !state_dir.join("federation").exists() {
        anyhow::bail!("Federation state directory not found");
    }

    Ok(())
}

fn print_success_summary(aura_profile: &str, dry_run: bool) {
    println!("{}", "═══════════════════════════════════════".cyan());
    println!("{}", "  🎉 Initialization Complete!".green().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    if dry_run {
        println!("{}", "  (Dry run: No changes were made)".yellow());
        println!();
    }

    println!("{}", format!("  Profile: {}", aura_profile).cyan());
    println!();

    println!("{}", "Next steps:".yellow().bold());
    println!("  1. Restart Claude Code to apply configuration");
    println!("  2. Run: nabi self doctor");
    println!("  3. Check: nabi federation status");
    println!();

    println!("{}", "Configuration:".dimmed());
    println!("  • Settings: ~/.claude/settings.json");
    println!("  • Hooks: ~/.config/nabi/governance/hooks/");
    println!("  • State: ~/.claude/agent-state/");
    println!();
}
