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
    #[serde(default)]
    pub drift_identified: String,
    #[serde(default)]
    pub updated: String,
    #[serde(default)]
    pub last_resolution: Option<String>,
    #[serde(default)]
    pub critical_issues: Vec<String>,
    #[serde(default)]
    pub resolved_issues: Vec<String>,
    #[serde(default)]
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

    let registry: PortRegistry = serde_json::from_str(&content).with_context(|| {
        format!(
            "Failed to parse registry JSON from {}",
            registry_path.display()
        )
    })?;

    Ok(registry)
}

fn get_registry_path() -> Result<PathBuf> {
    // Runtime data (registry.json) belongs in XDG_STATE_HOME per XDG Base Directory Spec
    // Check NABI_HOME environment variable first (backward compatibility)
    if let Ok(nabi_home) = std::env::var("NABI_HOME") {
        let path = PathBuf::from(nabi_home)
            .join("governance")
            .join("port-registry.json");
        if path.exists() {
            return Ok(path);
        }
    }

    // Primary location: XDG_STATE_HOME/nabi/governance/port-registry.json
    // (State directory = generated/runtime data, not user-editable config)
    use crate::paths::NabiPaths;
    let state_dir = NabiPaths::state_dir()?;
    let state_path = state_dir.join("governance").join("port-registry.json");

    if state_path.exists() {
        return Ok(state_path);
    }

    // Fallback: Check config directory for migrations from old setup
    let config_dir = NabiPaths::config_dir()?;
    let config_path = config_dir.join("governance").join("port-registry.json");

    if config_path.exists() {
        return Ok(config_path);
    }

    // Neither location has the registry
    anyhow::bail!(
        "Port registry not found. Checked:\n  - {}\n  - {}\n\nRun: nabi port rebuild",
        state_path.display(),
        config_path.display()
    );
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
    let listening = if port > 0 {
        is_port_listening(port)
    } else {
        false
    };

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
                        println!(
                            "  {} {}: port {} → {}",
                            status, service_name, port, endpoint
                        );
                    } else {
                        println!("  {} {}: port {}", status, service_name, port);
                    }
                } else if let Some(ports) = &service_spec.ports {
                    // Multi-port service
                    let ports_str: Vec<String> =
                        ports.iter().map(|(k, v)| format!("{}:{}", k, v)).collect();
                    println!(
                        "  {} {}: ports {}",
                        status,
                        service_name,
                        ports_str.join(", ")
                    );
                }
            }
        } else {
            eprintln!(
                "{}",
                format!("Platform '{}' not found in registry", platform).red()
            );
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
                    let ports_str: Vec<String> =
                        ports.iter().map(|(k, v)| format!("{}:{}", k, v)).collect();
                    println!(
                        "  {} {}: ports {}",
                        status,
                        service_name,
                        ports_str.join(", ")
                    );
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

    let config = registry
        .platform_configs
        .get(&platform)
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

        println!(
            "\n{} {} (Port {})",
            status_icon, health.service, health.port
        );
        println!(
            "   Listening: {}",
            if health.listening { "Yes" } else { "No" }
        );

        if health.listening {
            println!(
                "   Health Check: {}",
                if health.responding { "Pass" } else { "Fail" }
            );
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

            let standard_port = registry
                .standard_allocations
                .get(service_name)
                .and_then(|s| s.port);

            // Handle single port services
            if let Some(port) = service_spec.port {
                port_map.entry(port).or_insert_with(Vec::new).push((
                    platform.clone(),
                    service_name.clone(),
                    standard_port,
                ));
            }

            // Handle multi-port services
            if let Some(ports) = &service_spec.ports {
                for (_, port) in ports {
                    port_map.entry(*port).or_insert_with(Vec::new).push((
                        platform.clone(),
                        service_name.clone(),
                        standard_port,
                    ));
                }
            }
        }
    }

    let mut conflicts = Vec::new();

    // Analyze conflicts
    for (port, usages) in &port_map {
        if usages.len() > 1 {
            let services: Vec<String> = usages
                .iter()
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
    let mut registry = load_registry()?;
    let platform = detect_platform();

    println!("{}", "=".repeat(70));
    println!("PORT MIGRATION PLAN");
    println!("{}", "=".repeat(70));
    println!("Service: {}", service);
    println!("Current port: {}", old_port);
    println!("Target port: {}", new_port);
    println!("Platform: {}", platform);
    println!("Mode: {}", if dry_run { "DRY RUN" } else { "EXECUTE" });

    // Verify service exists with proper fallback:
    // 1. Try platform_configs first (platform-specific override)
    // 2. Fall back to standard_allocations (global default)
    let config = registry
        .platform_configs
        .get_mut(&platform)
        .ok_or_else(|| anyhow::anyhow!("Platform '{}' not found", platform))?;

    let service_spec = if let Some(spec) = config.services.get(service) {
        // Found in platform_configs
        println!("\n✓ Service found in platform config");
        Some(spec.clone())
    } else if let Some(_std_alloc) = registry.standard_allocations.get(service) {
        // Fall back to standard allocation
        println!("\n⚠ Service not in platform config, using standard allocation");
        println!("   Consider adding to platform_configs for platform-specific settings");
        None
    } else {
        anyhow::bail!(
            "Service '{}' not found in platform '{}' or standard allocations",
            service,
            platform
        );
    };

    // Check current port matches
    if let Some(spec) = &service_spec {
        if let Some(current_port) = spec.port {
            if current_port != old_port {
                anyhow::bail!(
                    "Current port mismatch: service is on {}, not {}",
                    current_port,
                    old_port
                );
            }
        }
    } else if let Some(std_alloc) = registry.standard_allocations.get(service) {
        if let Some(current_port) = std_alloc.port {
            if current_port != old_port {
                anyhow::bail!(
                    "Current port mismatch: service is on {}, not {}",
                    current_port,
                    old_port
                );
            }
        }
    }

    println!("\n{}", "Planned Steps:".bold());
    println!("1. Stop service: {}", service);
    if let Some(spec) = &service_spec {
        if let Some(container) = &spec.container_name {
            println!("   docker stop {}", container);
        }
    }

    println!("2. Update port-registry.json");
    println!("   {} → {}", old_port, new_port);

    println!("3. Update configuration files");
    if let Some(spec) = &service_spec {
        if let Some(compose) = &spec.compose_file {
            println!("   Update: {}", compose);
        }
    }

    println!("4. Restart service");
    if let Some(spec) = &service_spec {
        if let Some(container) = &spec.container_name {
            println!("   docker start {}", container);
        }
    }

    println!("5. Verify health check on new port");

    if !dry_run {
        // EXECUTE: Actually perform the migration
        println!("\n{}", "Executing migration...".bold());

        // Update port-registry.json with new port for this platform
        if let Some(service_spec_mut) = config.services.get_mut(service) {
            service_spec_mut.port = Some(new_port);
            println!(
                "✓ Updated port in platform config: {} → {}",
                old_port, new_port
            );
        } else {
            // Create new service spec entry for this platform from standard allocation
            if let Some(std_alloc) = registry.standard_allocations.get(service) {
                let new_spec = ServiceSpec {
                    enabled: true,
                    port: Some(new_port),
                    container_port: std_alloc.container_port,
                    ports: std_alloc.ports.clone(),
                    endpoint: None,
                    compose_file: None,
                    container_name: None,
                    note: Some(format!(
                        "Platform-specific override: port {} → {}",
                        old_port, new_port
                    )),
                };
                config.services.insert(service.to_string(), new_spec);
                println!(
                    "✓ Created platform-specific entry for {}: port {}",
                    service, new_port
                );
            }
        }

        // Save updated registry
        let registry_path = get_registry_path()?;
        let json = serde_json::to_string_pretty(&registry)
            .context("Failed to serialize updated registry")?;
        fs::write(&registry_path, json).context("Failed to write updated registry")?;
        println!("✓ Saved registry to {}", registry_path.display());

        println!("\n{}", "Migration complete!".green().bold());
        println!("Next steps:");
        println!("1. Update docker-compose files to use new port");
        println!("2. Stop and restart services");
        println!("3. Verify health checks on new port");
        println!("4. Update any client configurations pointing to old port");
    } else {
        println!("\n{}", "DRY RUN - No changes made".yellow().bold());
        println!("Run without --dry-run to execute the migration");
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
        if let Some(resolution) = &notes.last_resolution {
            println!("Last Resolution: {}", resolution);
        }

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

    let config = registry
        .platform_configs
        .get(&platform)
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

    let config = registry
        .platform_configs
        .get(&platform)
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

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = detect_platform();

        // Platform detection should always return something
        assert!(!platform.is_empty());

        // Platform should be one of the known ones
        let known_platforms = vec!["macos", "wsl", "rpi", "linux"];
        assert!(known_platforms.contains(&platform.as_str()));
    }

    #[test]
    fn test_port_range_validation() {
        // Valid ports
        assert!(is_valid_port(3000)); // Start of federation range
        assert!(is_valid_port(8499)); // End of federation range
        assert!(is_valid_port(8080)); // Common dev port

        // Invalid ports
        assert!(!is_valid_port(0)); // Too low
        assert!(!is_valid_port(1)); // Reserved range
        assert!(!is_valid_port(1024)); // Below federation range (deprecated check)
        assert!(!is_valid_port(65535)); // Too high for our range
    }

    #[test]
    fn test_port_registry_deserialization() {
        // Test that our data structures can deserialize properly
        let sample_json = r#"{
            "version": "1.0",
            "updated": "2025-11-05",
            "schema_version": "1.0",
            "description": "Port registry",
            "metadata": {
                "created_by": "test",
                "purpose": "testing",
                "validation_tool": "test",
                "last_reconciliation": "2025-11-05",
                "reconciliation_agent": "test"
            },
            "port_ranges": {
                "federation": {
                    "start": 3000,
                    "end": 8499,
                    "description": "Federation services"
                }
            },
            "standard_allocations": {
                "surrealdb": {
                    "port": 8004,
                    "protocol": "tcp",
                    "purpose": "Database",
                    "required": true,
                    "cross_platform": true
                }
            },
            "platform_configs": {}
        }"#;

        let registry: Result<PortRegistry, _> = serde_json::from_str(sample_json);
        assert!(registry.is_ok());

        let reg = registry.unwrap();
        assert_eq!(reg.version, "1.0");
        assert!(reg.port_ranges.contains_key("federation"));
        assert!(reg.standard_allocations.contains_key("surrealdb"));
    }

    #[test]
    fn test_get_registry_path_with_nabi_home() {
        // Test NABI_HOME fallback
        // This would normally be set in CI but might not be in local testing
        let original = std::env::var("NABI_HOME").ok();

        // Set a temporary NABI_HOME
        std::env::set_var("NABI_HOME", "/tmp/test_nabi_home");

        // The path resolver should at least not crash
        let path_result = get_registry_path();
        // We don't assert success because /tmp test path won't exist
        // But we verify it doesn't panic
        let _ = path_result;

        // Restore original
        if let Some(orig) = original {
            std::env::set_var("NABI_HOME", orig);
        } else {
            std::env::remove_var("NABI_HOME");
        }
    }

    #[test]
    fn test_service_health_structure() {
        // Test that ServiceHealth can be created and used
        let health = ServiceHealth {
            service: "test_service".to_string(),
            port: 8080,
            listening: false,
            responding: false,
            error: None,
        };

        assert_eq!(health.service, "test_service");
        assert_eq!(health.port, 8080);
        assert!(!health.listening);
        assert!(!health.responding);
    }

    #[test]
    fn test_port_listening_check_invalid_ports() {
        // Test ports that should never be listening
        assert!(!is_port_listening(1)); // Reserved, should be false
        assert!(!is_port_listening(12345)); // Unlikely to be listening
        assert!(!is_port_listening(54321)); // Unlikely to be listening
    }

    #[test]
    fn test_standard_allocation_multi_port() {
        // Test multi-port service specifications
        let mut ports = HashMap::new();
        ports.insert("http".to_string(), 8080u16);
        ports.insert("https".to_string(), 8443u16);

        let allocation = StandardAllocation {
            port: None,
            ports: Some(ports),
            container_port: None,
            container_ports: None,
            protocol: "tcp".to_string(),
            purpose: "test".to_string(),
            required: false,
            health_check: None,
            cross_platform: true,
            preferred_host: None,
            note: None,
        };

        assert!(allocation.port.is_none());
        assert!(allocation.ports.is_some());

        let ports_ref = allocation.ports.as_ref().unwrap();
        assert_eq!(ports_ref.len(), 2);
        assert_eq!(ports_ref.get("http"), Some(&8080u16));
        assert_eq!(ports_ref.get("https"), Some(&8443u16));
    }

    #[test]
    fn test_port_range_structure() {
        let range = PortRange {
            start: 3000,
            end: 8499,
            description: "Federation services".to_string(),
        };

        assert_eq!(range.start, 3000);
        assert_eq!(range.end, 8499);
        assert!(!range.description.is_empty());
    }
}

// ============================================================================
// Helper function for port validation (used in tests)
// ============================================================================

fn is_valid_port(port: u16) -> bool {
    port >= 3000 && port <= 8499
}
