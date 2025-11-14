/// Health command handlers - Federation substrate validation and reporting

use std::env;
use anyhow::Result;
use colored::*;
use crate::cli::HealthCommands;
use crate::routing::route_to_commander;

pub fn handle_health(command: HealthCommands) -> Result<()> {
    match command {
        HealthCommands::Quick => {
            health_quick()
        },
        HealthCommands::Substrate { auto_remediate, fsm_only } => {
            println!("{}", "🏥 Running federation substrate health checks...".green().bold());

            let mut args = vec!["check"];

            if auto_remediate {
                args.push("--auto-remediate");
            }

            if fsm_only {
                args.push("--fsm-only");
            }

            route_to_commander("health", &args)
        },
        HealthCommands::Services => {
            health_services()
        },
        HealthCommands::Ports => {
            health_ports()
        },
        HealthCommands::Check { auto_remediate, fsm_only } => {
            println!("{}", "🏥 Running federation substrate health checks...".green().bold());

            let mut args = vec!["check"];

            if auto_remediate {
                args.push("--auto-remediate");
            }

            if fsm_only {
                args.push("--fsm-only");
            }

            route_to_commander("health", &args)
        }
        HealthCommands::Status { detailed, hours } => {
            println!("{}", "📊 Checking health status...".cyan().bold());

            let hours_str = hours.to_string();
            let detailed_str = "--detailed";
            let hours_flag = "--hours";

            let mut args = vec!["status", hours_flag, &hours_str];

            if detailed {
                args.push(detailed_str);
            }

            route_to_commander("health", &args)
        }
        HealthCommands::Report { format, output } => {
            println!("{}", format!("📋 Generating health report ({} format)...", format).cyan().bold());

            let format_flag = "--format";
            let output_flag = "--output";
            let mut args = vec!["report", format_flag, &format];

            if let Some(ref path) = output {
                args.push(output_flag);
                args.push(path);
            }

            route_to_commander("health", &args)
        }
        HealthCommands::Dashboard { port } => {
            println!("{}", format!("📈 Opening Grafana dashboard at http://localhost:{}...", port).blue().bold());

            let port_str = port.to_string();
            let port_flag = "--port";
            route_to_commander("health", &["dashboard", port_flag, &port_str])
        }
        HealthCommands::Api { port, host, debug } => {
            println!("{}", format!("🌐 Starting Health Monitoring API on http://{}:{}...", host, port).green().bold());

            let port_str = port.to_string();
            let mut args = vec!["api", &port_str, &host];

            if debug {
                args.push("--debug");
            }

            route_to_commander("health", &args)
        }
    }
}

/// Quick bootstrap health check (replaces `nabi doctor`)
pub fn health_quick() -> Result<()> {
    println!("🔍 Quick Health Check (Bootstrap Validation)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Check commander binaries exist
    let commanders = vec!["claude", "data", "federation"];
    let mut all_ok = true;
    
    for cmd in commanders {
        let binary_path = format!("{}/.local/bin/nabi-{}", env::var("HOME").unwrap(), cmd);
        if std::path::Path::new(&binary_path).exists() {
            println!("✅ Commander '{cmd}' found");
        } else {
            println!("❌ Commander '{cmd}' missing at {binary_path}");
            all_ok = false;
        }
    }
    
    // XDG compliance check
    let xdg_dirs = vec![
        ("CONFIG", "~/.config/nabi"),
        ("DATA", "~/.local/share/nabi"),
        ("STATE", "~/.local/state/nabi"),
        ("CACHE", "~/.cache/nabi"),
    ];
    
    println!("\n📁 XDG Directory Compliance:");
    for (name, path) in xdg_dirs {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let expanded: String = path.replace("~", &home);
        if std::path::Path::new(&expanded).exists() {
            println!("✅ {name}: {path}");
        } else {
            println!("❌ {name}: {path} (missing)");
            all_ok = false;
        }
    }
    
    if all_ok {
        println!("\n✅ Bootstrap health: PASS");
        Ok(())
    } else {
        println!("\n❌ Bootstrap health: FAIL");
        Err(anyhow::anyhow!("Bootstrap health check failed"))
    }
}

/// Port health check (wraps port::check for health namespace)
pub fn health_ports() -> Result<()> {
    use crate::commands::port;
    
    println!("🔌 Port Health Check");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Call existing port check logic
    port::cmd_check()?;
    
    println!("\n📊 Port validation complete");
    println!("💡 For detailed port management, use: nabi port check");
    
    Ok(())
}

/// Federation service health check (replaces `nabi federation health`)
pub fn health_services() -> Result<()> {
    use std::process::Command;
    use std::collections::HashMap;
    use serde_json::json;
    
    println!("🏥 Federation Services Health Check");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Load registry
    let registry_path = format!("{}/.config/nabi/federation-registry.toml", env::var("HOME")?);
    let registry_content = std::fs::read_to_string(&registry_path)?;
    let registry: HashMap<String, HashMap<String, toml::Value>> = toml::from_str(&registry_content)?;
    
    let services = registry.get("services").ok_or_else(|| anyhow::anyhow!("No services in registry"))?;
    
    let mut healthy = 0;
    let mut unhealthy = 0;
    let mut results = Vec::new();
    
    for (name, config) in services {
        let service_type = config.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");
        let is_blocking = config.get("blocking").and_then(|v| v.as_bool()).unwrap_or(false);
        
        let status = match service_type {
            "docker" => {
                // Check Docker container
                let output = Command::new("docker")
                    .args(&["ps", "--filter", &format!("name={}", name), "--format", "{{.Status}}"])
                    .output()?;
                if output.stdout.is_empty() {
                    "❌ Not Running"
                } else {
                    "✅ Running"
                }
            },
            "launchagent" => {
                // Check LaunchAgent
                let output = Command::new("launchctl")
                    .args(&["list", name])
                    .output()?;
                if output.status.success() {
                    "✅ Running"
                } else {
                    "❌ Not Loaded"
                }
            },
            "nats" => {
                // Check NATS connectivity
                let output = Command::new("nc")
                    .args(&["-z", "localhost", "4222"])
                    .output()?;
                if output.status.success() {
                    "✅ Connected"
                } else {
                    "❌ Unreachable"
                }
            },
            _ => "⚠️  Unknown Type"
        };
        
        let blocking_marker = if is_blocking { "🔒" } else { "" };
        println!("{}{}: {} ({})", blocking_marker, name, status, service_type);
        
        if status.starts_with("✅") {
            healthy += 1;
        } else {
            unhealthy += 1;
        }
        
        results.push((name.clone(), status.to_string(), service_type.to_string()));
    }
    
    // Save report
    let output_path = format!("{}/.local/state/nabi/health-checks/services/report.json", env::var("HOME")?);
    let report = json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "total": services.len(),
        "healthy": healthy,
        "unhealthy": unhealthy,
        "services": results.iter().map(|(n, s, t)| {
            json!({"name": n, "status": s, "type": t})
        }).collect::<Vec<_>>()
    });
    
    std::fs::create_dir_all(std::path::Path::new(&output_path).parent().unwrap())?;
    std::fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;
    
    println!("\n📊 Summary: {}/{} services healthy", healthy, services.len());
    println!("📄 Report saved: {}", output_path);
    
    if unhealthy > 0 {
        Err(anyhow::anyhow!("{} services unhealthy", unhealthy))
    } else {
        Ok(())
    }
}
