/// Service Orchestration - Docker Compose Management
///
/// Centralized service deployment and validation for NabiOS federation infrastructure.
/// Manages docker-compose stacks across platform, core, and memchain service groups.
///
/// Commands:
/// - status: Show all running containers with health status
/// - validate: Validate compose files against TOML configuration
/// - rebuild: Rebuild container images by service group
/// - deploy: Deploy service groups (docker-compose up -d)
///
/// Service Groups:
/// - all: All services across platform, core, and memchain
/// - platform: Platform services (monitoring, surrealdb, knowledge-graph, vigil)
/// - core: Core services (oauth-mcp-proxy)
/// - memchain: Memchain coordination (mcp-sse, coordination-server)
/// - monitoring: Just the monitoring stack (loki, prometheus, grafana)
use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

// ============================================================================
// Service Definitions
// ============================================================================

/// Service group configuration
#[derive(Debug, Clone)]
struct ServiceGroup {
    name: String,
    description: String,
    compose_files: Vec<ComposeFile>,
}

/// Docker compose file location
#[derive(Debug, Clone)]
struct ComposeFile {
    path: PathBuf,
    service_name: String,
}

/// Get home directory
fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .expect("HOME environment variable not set")
}

/// Get all service groups
fn get_service_groups() -> Vec<ServiceGroup> {
    let home = home_dir();

    vec![
        ServiceGroup {
            name: "monitoring".to_string(),
            description: "Monitoring stack (Loki, Prometheus, Grafana)".to_string(),
            compose_files: vec![
                ComposeFile {
                    path: home.join("nabia/platform/services/compose/monitoring"),
                    service_name: "monitoring".to_string(),
                },
            ],
        },
        ServiceGroup {
            name: "platform".to_string(),
            description: "Platform services (SurrealDB, Knowledge Graph, Vigil)".to_string(),
            compose_files: vec![
                ComposeFile {
                    path: home.join("nabia/platform/services/compose/surrealdb"),
                    service_name: "surrealdb-federation".to_string(),
                },
                ComposeFile {
                    path: home.join("nabia/platform/services/compose/knowledge-graph"),
                    service_name: "knowledge-graph".to_string(),
                },
                ComposeFile {
                    path: home.join("nabia/platform/services/vigil"),
                    service_name: "vigil".to_string(),
                },
            ],
        },
        ServiceGroup {
            name: "core".to_string(),
            description: "Core services (OAuth MCP Proxy)".to_string(),
            compose_files: vec![
                ComposeFile {
                    path: home.join("nabia/core/containers/oauth-mcp-proxy"),
                    service_name: "oauth-mcp-proxy".to_string(),
                },
            ],
        },
        ServiceGroup {
            name: "memchain".to_string(),
            description: "Memchain coordination (MCP SSE, Coordination Server)".to_string(),
            compose_files: vec![
                ComposeFile {
                    path: home.join("nabia/memchain"),
                    service_name: "memchain-mcp-sse".to_string(),
                },
            ],
        },
    ]
}

/// Get service groups by name
fn get_groups_for_target(target: &str) -> Vec<ServiceGroup> {
    let all_groups = get_service_groups();

    match target {
        "all" => all_groups,
        "monitoring" => all_groups.into_iter().filter(|g| g.name == "monitoring").collect(),
        "platform" => all_groups.into_iter().filter(|g| g.name == "platform").collect(),
        "core" => all_groups.into_iter().filter(|g| g.name == "core").collect(),
        "memchain" => all_groups.into_iter().filter(|g| g.name == "memchain").collect(),
        _ => {
            eprintln!("{}", format!("Unknown service group: {}", target).red());
            eprintln!("Valid groups: all, monitoring, platform, core, memchain");
            vec![]
        }
    }
}

// ============================================================================
// Command Implementations
// ============================================================================

/// Show status of all running containers
pub fn cmd_status(format: Option<&str>) -> Result<()> {
    let format_str = format.unwrap_or("table");

    match format_str {
        "table" => {
            let output = ProcessCommand::new("docker")
                .args(&["ps", "--format", "table {{.Names}}\t{{.Status}}\t{{.Ports}}"])
                .output()
                .context("Failed to execute docker ps")?;

            if output.status.success() {
                println!("{}", String::from_utf8_lossy(&output.stdout));
            } else {
                eprintln!("{}", "Failed to get container status".red());
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                return Err(anyhow::anyhow!("docker ps failed"));
            }
        }
        "json" => {
            let output = ProcessCommand::new("docker")
                .args(&["ps", "--format", "{{json .}}"])
                .output()
                .context("Failed to execute docker ps")?;

            if output.status.success() {
                println!("{}", String::from_utf8_lossy(&output.stdout));
            } else {
                return Err(anyhow::anyhow!("docker ps failed"));
            }
        }
        _ => {
            eprintln!("{}", format!("Unknown format: {}", format_str).red());
            eprintln!("Valid formats: table, json");
            return Err(anyhow::anyhow!("Invalid format"));
        }
    }

    Ok(())
}

/// Validate compose files
pub fn cmd_validate() -> Result<()> {
    println!("{}", "🔍 Validating docker-compose files...".cyan().bold());

    let groups = get_service_groups();
    let mut all_valid = true;

    for group in &groups {
        println!("\n{} {}", "📦".cyan(), group.name.bold());

        for compose_file in &group.compose_files {
            let compose_path = compose_file.path.join("docker-compose.yml");

            if !compose_path.exists() {
                println!(
                    "  {} {} - {}",
                    "✗".red(),
                    compose_file.service_name,
                    "compose file not found".yellow()
                );
                all_valid = false;
                continue;
            }

            // Validate compose file syntax
            let output = ProcessCommand::new("docker-compose")
                .args(&["-f", compose_path.to_str().unwrap(), "config", "--quiet"])
                .current_dir(&compose_file.path)
                .output()
                .context("Failed to validate compose file")?;

            if output.status.success() {
                println!(
                    "  {} {}",
                    "✓".green(),
                    compose_file.service_name
                );
            } else {
                println!(
                    "  {} {} - {}",
                    "✗".red(),
                    compose_file.service_name,
                    "validation failed".red()
                );
                if !output.stderr.is_empty() {
                    println!("    {}", String::from_utf8_lossy(&output.stderr).trim());
                }
                all_valid = false;
            }
        }
    }

    println!();
    if all_valid {
        println!("{}", "✅ All compose files are valid".green().bold());
        Ok(())
    } else {
        Err(anyhow::anyhow!("Some compose files have validation errors"))
    }
}

/// Rebuild containers for a service group
pub fn cmd_rebuild(group: &str) -> Result<()> {
    let groups = get_groups_for_target(group);

    if groups.is_empty() {
        return Err(anyhow::anyhow!("No service groups selected"));
    }

    println!(
        "{} {}",
        "🔨 Rebuilding services for group:".cyan().bold(),
        group.yellow().bold()
    );

    for service_group in &groups {
        println!("\n{} {}", "📦".cyan(), service_group.name.bold());

        for compose_file in &service_group.compose_files {
            let compose_path = compose_file.path.join("docker-compose.yml");

            if !compose_path.exists() {
                println!(
                    "  {} {} - skipped (no compose file)",
                    "⊘".yellow(),
                    compose_file.service_name
                );
                continue;
            }

            println!(
                "  {} {}",
                "🔨".cyan(),
                compose_file.service_name
            );

            let status = ProcessCommand::new("docker-compose")
                .args(&["build"])
                .current_dir(&compose_file.path)
                .status()
                .context(format!("Failed to rebuild {}", compose_file.service_name))?;

            if status.success() {
                println!(
                    "    {} Build successful",
                    "✓".green()
                );
            } else {
                println!(
                    "    {} Build failed",
                    "✗".red()
                );
                return Err(anyhow::anyhow!("Build failed for {}", compose_file.service_name));
            }
        }
    }

    println!("\n{}", "✅ Rebuild complete".green().bold());
    Ok(())
}

/// Deploy service group
pub fn cmd_deploy(group: &str) -> Result<()> {
    let groups = get_groups_for_target(group);

    if groups.is_empty() {
        return Err(anyhow::anyhow!("No service groups selected"));
    }

    println!(
        "{} {}",
        "🚀 Deploying services for group:".cyan().bold(),
        group.yellow().bold()
    );

    // Check if networks exist
    println!("\n{}", "🔍 Checking Docker networks...".cyan());
    let required_networks = vec!["platform", "memchain", "federation-network", "nabi-net"];

    for network in &required_networks {
        let output = ProcessCommand::new("docker")
            .args(&["network", "inspect", network])
            .output()
            .context("Failed to check network")?;

        if !output.status.success() {
            println!(
                "  {} Network '{}' not found - creating...",
                "⚠".yellow(),
                network
            );

            let create_status = ProcessCommand::new("docker")
                .args(&["network", "create", network])
                .status()
                .context(format!("Failed to create network {}", network))?;

            if create_status.success() {
                println!("    {} Created", "✓".green());
            } else {
                return Err(anyhow::anyhow!("Failed to create network {}", network));
            }
        } else {
            println!("  {} Network '{}' exists", "✓".green(), network);
        }
    }

    for service_group in &groups {
        println!("\n{} {}", "📦".cyan(), service_group.name.bold());

        for compose_file in &service_group.compose_files {
            let compose_path = compose_file.path.join("docker-compose.yml");

            if !compose_path.exists() {
                println!(
                    "  {} {} - skipped (no compose file)",
                    "⊘".yellow(),
                    compose_file.service_name
                );
                continue;
            }

            println!(
                "  {} {}",
                "🚀".cyan(),
                compose_file.service_name
            );

            // Determine which compose file to use
            let compose_arg = if compose_file.service_name == "memchain-mcp-sse" {
                "docker-compose.sse.yml"
            } else if compose_file.service_name == "coordination-server" {
                "docker-compose.coordination.yml"
            } else {
                "docker-compose.yml"
            };

            let status = ProcessCommand::new("docker-compose")
                .args(&["-f", compose_arg, "up", "-d"])
                .current_dir(&compose_file.path)
                .status()
                .context(format!("Failed to deploy {}", compose_file.service_name))?;

            if status.success() {
                println!(
                    "    {} Deployed successfully",
                    "✓".green()
                );
            } else {
                println!(
                    "    {} Deployment failed",
                    "✗".red()
                );
                return Err(anyhow::anyhow!("Deploy failed for {}", compose_file.service_name));
            }
        }
    }

    println!("\n{}", "✅ Deployment complete".green().bold());
    println!("\n{}", "📊 Checking deployed containers...".cyan());

    // Show status after deployment
    cmd_status(Some("table"))?;

    Ok(())
}