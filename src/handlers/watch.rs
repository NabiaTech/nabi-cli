/// Watch command handler

use anyhow::Result;
use colored::*;
use crate::routing::route_to_commander;

pub fn handle_watch(path: Option<String>) -> Result<()> {
    println!("{}", "👁  Watching filesystem...".cyan().bold());
    let mut args: Vec<&str> = vec!["watch"];
    let mut path_str = String::new();

    if let Some(p) = &path {
        path_str = p.clone();
        args.push(&path_str);
    }
    route_to_commander("watch", &args)
}
