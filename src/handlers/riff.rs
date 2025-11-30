use crate::routing::route_to_commander;
/// Riff command handlers
///
/// Routes all riff commands to the Python CLI layer (riff-cli).
/// Maintains Python boundary - riff-cli remains Python-based.
use anyhow::Result;
use colored::*;

pub fn handle_riff(args: Vec<String>) -> Result<()> {
    // Layer 1 → Layer 2 handoff for riff-cli
    // Route all riff commands to Python CLI layer
    // Maintains Python boundary: riff-cli stays in Python
    println!("{}", "🔍 Routing to riff-cli...".cyan().bold());

    // Convert Vec<String> to &[&str] for route_to_commander
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    
    // Route to "riff" commander via Python CLI
    // This will call: nabi-python riff <args...>
    route_to_commander("riff", &arg_refs)
}
