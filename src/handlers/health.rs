/// Health command handlers - Federation substrate validation and reporting

use anyhow::Result;
use colored::*;
use crate::cli::HealthCommands;
use crate::routing::route_to_commander;

pub fn handle_health(command: HealthCommands) -> Result<()> {
    match command {
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
    }
}
