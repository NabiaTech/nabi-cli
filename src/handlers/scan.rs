use crate::routing::route_to_commander;
/// Scan command handler
use anyhow::Result;
use colored::*;

pub fn handle_scan(
    path: Option<String>,
    tags: Option<String>,
    confidence: Option<f32>,
) -> Result<()> {
    println!("{}", "🔍 Scanning filesystem...".cyan().bold());
    let mut args: Vec<&str> = vec!["scan"];
    let mut path_str = String::new();
    let mut tags_str = String::new();
    let mut conf_str = String::new();

    if let Some(p) = &path {
        path_str = p.clone();
        args.push(&path_str);
    }
    if let Some(t) = &tags {
        args.push("--tags");
        tags_str = t.clone();
        args.push(&tags_str);
    }
    if let Some(c) = confidence {
        args.push("--confidence");
        conf_str = c.to_string();
        args.push(&conf_str);
    }
    route_to_commander("scan", &args)
}
