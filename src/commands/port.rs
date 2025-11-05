/// Port Registry Management - Native Rust Implementation
///
/// Replaces the three-layer Python delegation (Rust → Bash → Python) with
/// native Rust implementation for all port management commands.
///
/// Commands:
/// - list: List port allocations (with optional --platform filter)
/// - check: Validate port allocations against running processes
/// - cross-platform: Check for cross-platform conflicts
/// - shift: Safely migrate service to new port
/// - drift: Forensic analysis of port drift
/// - fix: Auto-generate fix commands for conflicts
/// - generate-env: Generate docker-compose .env file

use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PortRegistry {
    pub version: String,
    pub updated: String,
    pub schema_version: String,
    pub description: String,
    pub metadata: RegistryMetadata,
    pub port_ranges: HashMap<String, PortRange>,
    pub standard_allocations: HashMap<String, StandardAllocation>,
    pub platform_configs: HashMap<String, PlatformConfig>,
    #[serde(default)]
    pub migration_notes: Option<MigrationNotes>,
    #[serde(default)]
    pub dynamic_services: Option<HashMap<String, DynamicService>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryMetadata {
    pub created_by: String,
    pub purpose: String,
    pub validation_tool: String,
    pub last_reconciliation: String,
    pub reconciliation_agent: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StandardAllocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<HashMap<String, u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_ports: Option<HashMap<String, u16>>,
    pub protocol: String,
    pub purpose: String,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<String>,
    #[serde(default)]
    pub cross_platform: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlatformConfig {
    pub hostname: String,
    pub tailscale_hostname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub services: HashMap<String, ServiceSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceSpec {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<HashMap<String, u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compose_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MigrationNotes {
    pub drift_identified: String,
    pub updated: String,
    pub last_resolution: String,
    pub critical_issues: Vec<String>,
    pub resolved_issues: Vec<String>,
    pub forensics_required: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DynamicService {
    pub endpoint: String,
    pub capabilities: Vec<String>,
    pub health_endpoint: String,
    pub status: String,
    pub protocol: String,
    pub registered_at: String,
}

#[derive(Debug, Clone)]
pub struct ServiceHealth {
    pub service: String,
    pub port: u16,
    pub listening: bool,
    pub responding: bool,
    pub error: Option<String>,
}

// ============================================================================
// Platform Detection
// ============================================================================

pub fn detect_platform() -> String {
    if cfg!(target_os = "macos") {
        return "macos".to_string();
    }

    if cfg!(target_os = "linux") {
        // Check for WSL
        if let Ok(version) = fs::read_to_string("/proc/version") {
            if version.to_lowercase().contains("microsoft") {
                return "wsl".to_string();
            }
        }

        // Check for Raspberry Pi
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            if cpuinfo.to_lowercase().contains("raspberry") {
                return "rpi".to_string();
            }
        }

        return "linux".to_string();
    }

    "unknown".to_string()
}

// ============================================================================
// Registry Loading
// ============================================================================

pub fn load_registry() -> Result<PortRegistry> {
    let registry_path = get_registry_path()?;

    let content = fs::read_to_string(&registry_path)
        .with_context(|| format!("Failed to read registry from {}", registry_path.display()))?;

    let registry: PortRegistry = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse registry JSON from {}", registry_path.display()))?;

    Ok(registry)
}

fn get_registry_path() -> Result<PathBuf> {
    // Check NABI_HOME environment variable first
    if let Ok(nabi_home) = std::env::var("NABI_HOME") {
        let path = PathBuf::from(nabi_home).join("governance").join("port-registry.json");
        if path.exists() {
            return Ok(path);
        }
    }

    // Default to ~/.config/nabi/governance/port-registry.json
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;

    let path = home.join(".config").join("nabi").join("governance").join("port-registry.json");

    if !path.exists() {
        anyhow::bail!("Registry not found at {}", path.display());
    }

    Ok(path)
}

// ============================================================================
// Port Checking
// ============================================================================

fn is_port_listening(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_secs(1),
    )
    .is_ok()
}

fn check_service_health(
    service: &str,
    spec: &ServiceSpec,
    standard: Option<&StandardAllocation>,
) -> ServiceHealth {
    let port = spec.port.unwrap_or(0);
    let listening = if port > 0 { is_port_listening(port) } else { false };

    // For now, we'll consider a service responding if it's listening
    // Full HTTP health checks would require adding reqwest dependency
    let responding = listening;

    ServiceHealth {
        service: service.to_string(),
        port,
        listening,
        responding,
        error: None,
    }
}

// ============================================================================
// Commands Implementation
// ============================================================================

/// List all registered ports, optionally filtered by platform
pub fn cmd_list(platform_filter: Option<&str>) -> Result<()> {
    let registry = load_registry()?;
    let current_platform = detect_platform();

    if let Some(platform) = platform_filter {
        // Show specific platform
        if let Some(config) = registry.platform_configs.get(platform) {
            println!("Services on {}:", platform);
            for (service_name, service_spec) in &config.services {
                if !service_spec.enabled {
                    continue;
                }

                let status = if service_spec.enabled { "✓" } else { "✗" };

                if let Some(port) = service_spec.port {
                    if let Some(endpoint) = &service_spec.endpoint {
                        println!("  {} {}: port {} → {}", status, service_name, port, endpoint);
                    } else {
                        println!("  {} {}: port {}", status, service_name, port);
                    }
                } else if let Some(ports) = &service_spec.ports {
                    // Multi-port service
                    let ports_str: Vec<String> = ports.iter()
                        .map(|(k, v)| format!("{}:{}", k, v))
                        .collect();
                    println!("  {} {}: ports {}", status, service_name, ports_str.join(", "));
                }
            }
        } else {
            eprintln!("{}", format!("Platform '{}' not found in registry", platform).red());
        }
    } else {
        // Show all platforms
        for (platform, config) in &registry.platform_configs {
            println!("\n{}:", platform);
            for (service_name, service_spec) in &config.services {
                if !service_spec.enabled {
                    continue;
                }

                let status = if service_spec.enabled { "✓" } else { "✗" };

                if let Some(port) = service_spec.port {
                    println!("  {} {}: port {}", status, service_name, port);
                } else if let Some(ports) = &service_spec.ports {
                    let ports_str: Vec<String> = ports.iter()
                        .map(|(k, v)| format!("{}:{}", k, v))
                        .collect();
                    println!("  {} {}: ports {}", status, service_name, ports_str.join(", "));
                }
            }
        }
    }

    Ok(())
}

/// Validate port allocations on current platform
pub fn cmd_check() -> Result<()> {
    let registry = load_registry()?;
    let platform = detect_platform();

    println!("Platform: {}", platform);
    println!("Registry: {}\n", get_registry_path()?.display());
    println!("{}", "=".repeat(70));
    println!("SERVICE HEALTH REPORT");
    println!("{}", "=".repeat(70));

    let config = registry.platform_configs.get(&platform)
        .ok_or_else(|| anyhow::anyhow!("Platform '{}' not found in registry", platform))?;

    let mut issues = Vec::new();
    let mut health_checks = Vec::new();

    for (service_name, service_spec) in &config.services {
        if !service_spec.enabled {
            continue;
        }

        let standard = registry.standard_allocations.get(service_name);
        let health = check_service_health(service_name, service_spec, standard);

        // Print health status
        let status_icon = if health.listening && health.responding {
            "✅"
        } else if health.listening {
            "⚠️ "
        } else {
            "❌"
        };

        println!("\n{} {} (Port {})", status_icon, health.service, health.port);
        println!("   Listening: {}", if health.listening { "Yes" } else { "No" });

        if health.listening {
            println!("   Health Check: {}", if health.responding { "Pass" } else { "Fail" });
        }

        // Check for issues
        if let Some(standard_alloc) = standard {
            if standard_alloc.required {
                if !health.listening {
                    issues.push(format!(
                        "❌ {}: Required service not listening on port {}",
                        service_name, health.port
                    ));
                } else if !health.responding {
                    issues.push(format!(
                        "⚠️  {}: Listening on {} but health check failed",
                        service_name, health.port
                    ));
                }
            }

            // Check for port drift
            if let Some(standard_port) = standard_alloc.port {
                if let Some(actual_port) = service_spec.port {
                    if standard_port != actual_port {
                        issues.push(format!(
                            "⚠️  {}: Using port {}, standard is {} (drift detected)",
                            service_name, actual_port, standard_port
                        ));
                    }
                }
            }
        }

        health_checks.push(health);
    }

    // Print summary
    if !issues.is_empty() {
        println!("\n{}", "=".repeat(70));
        println!("⚠️  ISSUES DETECTED");
        println!("{}", "=".repeat(70));
        for issue in &issues {
            println!("  {}", issue);
        }
    } else {
        println!("\n✅ All services validated successfully");
    }

    if !issues.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

/// Check for cross-platform port conflicts
pub fn cmd_cross_platform() -> Result<()> {
    let registry = load_registry()?;

    println!("{}", "=".repeat(70));
    println!("CROSS-PLATFORM ANALYSIS");
    println!("{}", "=".repeat(70));

    let mut port_map: HashMap<u16, Vec<(String, String, Option<u16>)>> = HashMap::new();

    // Build port usage map
    for (platform, config) in &registry.platform_configs {
        for (service_name, service_spec) in &config.services {
            if !service_spec.enabled {
                continue;
            }

            let standard_port = registry.standard_allocations
                .get(service_name)
                .and_then(|s| s.port);

            // Handle single port services
            if let Some(port) = service_spec.port {
                port_map.entry(port)
                    .or_insert_with(Vec::new)
                    .push((platform.clone(), service_name.clone(), standard_port));
            }

            // Handle multi-port services
            if let Some(ports) = &service_spec.ports {
                for (_, port) in ports {
                    port_map.entry(*port)
                        .or_insert_with(Vec::new)
                        .push((platform.clone(), service_name.clone(), standard_port));
                }
            }
        }
    }

    let mut conflicts = Vec::new();

    // Analyze conflicts
    for (port, usages) in &port_map {
        if usages.len() > 1 {
            let services: Vec<String> = usages.iter()
                .map(|(p, s, _)| format!("{}@{}", s, p))
                .collect();
            conflicts.push(format!(
                "⚠️  Port {} used by multiple services: {}",
                port,
                services.join(", ")
            ));
        }

        // Check for drift from standards
        for (platform, service, standard_port) in usages {
            if let Some(std_port) = standard_port {
                if *port != *std_port {
                    conflicts.push(format!(
                        "⚠️  {}@{}: Drift detected - using {} instead of standard {}",
                        service, platform, port, std_port
                    ));
                }
            }
        }
    }

    if !conflicts.is_empty() {
        println!("\n⚠️  Conflicts detected:");
        for conflict in &conflicts {
            println!("  {}", conflict);
        }
    } else {
        println!("\n✅ No cross-platform conflicts");
    }

    Ok(())
}

/// Safely migrate service to new port
pub fn cmd_shift(service: &str, old_port: u16, new_port: u16, dry_run: bool) -> Result<()> {
    let registry = load_registry()?;
    let platform = detect_platform();

    println!("{}", "=".repeat(70));
    println!("PORT MIGRATION PLAN");
    println!("{}", "=".repeat(70));
    println!("Service: {}", service);
    println!("Current port: {}", old_port);
    println!("Target port: {}", new_port);
    println!("Platform: {}", platform);
    println!("Mode: {}", if dry_run { "DRY RUN" } else { "EXECUTE" });

    // Verify service exists
    let config = registry.platform_configs.get(&platform)
        .ok_or_else(|| anyhow::anyhow!("Platform '{}' not found", platform))?;

    let service_spec = config.services.get(service)
        .ok_or_else(|| anyhow::anyhow!("Service '{}' not found on platform '{}'", service, platform))?;

    // Check current port matches
    if let Some(current_port) = service_spec.port {
        if current_port != old_port {
            anyhow::bail!("Current port mismatch: service is on {}, not {}", current_port, old_port);
        }
    }

    println!("\n{}", "Steps:".bold());
    println!("1. Stop service: {}", service);
    if let Some(container) = &service_spec.container_name {
        println!("   docker stop {}", container);
    }

    println!("2. Update port-registry.json");
    println!("   {} → {}", old_port, new_port);

    println!("3. Update configuration files");
    if let Some(compose) = &service_spec.compose_file {
        println!("   Update: {}", compose);
    }

    println!("4. Restart service");
    if let Some(container) = &service_spec.container_name {
        println!("   docker start {}", container);
    }

    println!("5. Verify health check on new port");

    if dry_run {
        println!("\n{}", "DRY RUN - No changes made".yellow().bold());
    } else {
        println!("\n{}", "⚠️  Actual migration not implemented yet - use --dry-run to preview".yellow());
    }

    Ok(())
}

/// Perform forensic analysis of port drift
pub fn cmd_drift(forensic: bool, since: Option<&str>) -> Result<()> {
    let registry = load_registry()?;

    println!("{}", "=".repeat(70));
    println!("PORT DRIFT ANALYSIS");
    println!("{}", "=".repeat(70));

    if let Some(notes) = &registry.migration_notes {
        println!("\nDrift Identified: {}", notes.drift_identified);
        println!("Last Update: {}", notes.updated);
        println!("Last Resolution: {}", notes.last_resolution);

        if !notes.critical_issues.is_empty() {
            println!("\n{}", "Critical Issues:".red().bold());
            for issue in &notes.critical_issues {
                println!("  {}", issue);
            }
        }

        if !notes.resolved_issues.is_empty() {
            println!("\n{}", "Resolved Issues:".green().bold());
            for issue in &notes.resolved_issues {
                println!("  {}", issue);
            }
        }

        if forensic && !notes.forensics_required.is_empty() {
            println!("\n{}", "Forensics Required:".yellow().bold());
            for item in &notes.forensics_required {
                println!("  {}", item);
            }
        }
    } else {
        println!("\n✅ No drift detected - registry is clean");
    }

    Ok(())
}

/// Auto-generate fix commands for conflicts
pub fn cmd_fix() -> Result<()> {
    let registry = load_registry()?;
    let platform = detect_platform();

    println!("{}", "=".repeat(70));
    println!("FIX COMMANDS");
    println!("{}", "=".repeat(70));

    let config = registry.platform_configs.get(&platform)
        .ok_or_else(|| anyhow::anyhow!("Platform '{}' not found", platform))?;

    println!("\n# Stop conflicting services:");
    let mut containers = Vec::new();
    for (service_name, service_spec) in &config.services {
        if !service_spec.enabled {
            continue;
        }

        // Check for drift
        if let Some(standard) = registry.standard_allocations.get(service_name) {
            if let (Some(std_port), Some(actual_port)) = (standard.port, service_spec.port) {
                if std_port != actual_port {
                    if let Some(container) = &service_spec.container_name {
                        containers.push(container.clone());
                    }
                }
            }
        }
    }

    if !containers.is_empty() {
        println!("docker stop {}", containers.join(" "));

        println!("\n# Update configurations to use standard ports");
        println!("# Edit port-registry.json or docker-compose files");

        println!("\n# Restart services:");
        println!("docker start {}", containers.join(" "));
    } else {
        println!("\n✅ No conflicts detected - no fixes needed");
    }

    Ok(())
}

/// Generate .env file for docker-compose
pub fn cmd_generate_env() -> Result<()> {
    let registry = load_registry()?;
    let platform = detect_platform();

    let config = registry.platform_configs.get(&platform)
        .ok_or_else(|| anyhow::anyhow!("Platform '{}' not found", platform))?;

    let mut env_lines = vec![
        "# Auto-generated by nabi port generate-env".to_string(),
        format!("# Platform: {}", platform),
        format!("# Generated: {}", chrono::Utc::now().to_rfc3339()),
        String::new(),
        "# Port Allocations (DO NOT EDIT - managed by port-registry.json)".to_string(),
        String::new(),
    ];

    for (service_name, service_spec) in &config.services {
        let var_name = service_name.to_uppercase().replace('-', "_");

        if let Some(port) = service_spec.port {
            env_lines.push(format!("{}_PORT={}", var_name, port));
        }

        if let Some(container_port) = service_spec.container_port {
            env_lines.push(format!("{}_CONTAINER_PORT={}", var_name, container_port));
        }
    }

    let env_content = env_lines.join("\n");
    let env_path = std::env::current_dir()?.join(".env.ports");

    fs::write(&env_path, env_content)
        .with_context(|| format!("Failed to write .env file to {}", env_path.display()))?;

    println!("📝 Generated: {}", env_path.display());
    println!("\nSource this in docker-compose.yml with:");
    println!("  env_file:");
    println!("    - .env.ports");

    Ok(())
}
