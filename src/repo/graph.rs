/// Graph operations - Query indexed codebase for symbols and references
///
/// Implements:
/// - nabi graph repo search <symbol>
/// - nabi graph repo references <symbol>
/// - nabi graph repo related <symbol>
use super::codegraph;
use anyhow::{anyhow, Result};
use colored::*;
use std::path::Path;

pub fn search(repo_path: &str, symbol: &str, format: &str) -> Result<()> {
    println!();
    println!("{}", "🔍 Symbol Search".cyan().bold());
    println!();

    // Validate index
    let index = load_and_validate_index(repo_path)?;

    // Search for symbol
    let results = codegraph::find_symbol(&index, symbol);

    if results.is_empty() {
        println!(
            "{}",
            format!("No symbols found matching '{}'", symbol).yellow()
        );
        return Ok(());
    }

    output_symbol_results(format, &results)?;

    Ok(())
}

pub fn references(repo_path: &str, symbol: &str, format: &str) -> Result<()> {
    println!();
    println!("{}", "🔗 Symbol References".cyan().bold());
    println!();

    // Validate index
    let index = load_and_validate_index(repo_path)?;

    // Find references
    let results = codegraph::find_references(&index, symbol);

    if results.is_empty() {
        println!(
            "{}",
            format!("No references found for '{}'", symbol).yellow()
        );
        return Ok(());
    }

    println!(
        "{}",
        format!("Found {} references for '{}':", results.len(), symbol).cyan()
    );
    println!();

    output_symbol_results(format, &results)?;

    Ok(())
}

pub fn related(repo_path: &str, symbol: &str, depth: usize, format: &str) -> Result<()> {
    println!();
    println!("{}", "🔀 Related Symbols".cyan().bold());
    println!();

    // Validate index
    let index = load_and_validate_index(repo_path)?;

    // Find related symbols
    let results = codegraph::find_related(&index, symbol, depth);

    if results.is_empty() {
        println!(
            "{}",
            format!("No related symbols found for '{}'", symbol).yellow()
        );
        return Ok(());
    }

    println!(
        "{}",
        format!(
            "Found {} symbols related to '{}' (depth: {}):",
            results.len(),
            symbol,
            depth
        )
        .cyan()
    );
    println!();

    output_symbol_results(format, &results)?;

    Ok(())
}

fn load_and_validate_index(repo_path: &str) -> Result<codegraph::CodegraphIndex> {
    let path = Path::new(repo_path)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(repo_path).to_path_buf());

    // Run pre-query validation hook
    let _ = codegraph::run_pre_query_hook(repo_path);

    // Find index directory (matches analyze.rs naming: repo-hash-lang)
    let cache_dir = codegraph::get_cache_dir()?;
    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Calculate repo hash (same as analyze.rs)
    let repo_hash = format_hash(repo_path);

    // Try to detect language first for better matching
    let detected_lang =
        codegraph::detect_language(repo_path).unwrap_or_else(|_| "rust".to_string());
    let preferred_pattern = format!("{}-{}-{}", repo_name, repo_hash, detected_lang);

    // Look for matching index directories
    let entries = std::fs::read_dir(&cache_dir)?;
    let mut matching_indices: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) {
            // Match pattern: repo-hash-lang or repo-hash-* (any language)
            let expected_prefix = format!("{}-{}-", repo_name, repo_hash);
            if name.starts_with(&expected_prefix) {
                // Check if it's a valid index directory
                if entry_path.join("metadata.json").exists() {
                    // Get modification time for sorting (prefer most recent)
                    if let Ok(metadata) = entry_path.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            matching_indices.push((entry_path, modified));
                        }
                    }
                }
            }
        }
    }

    // Prefer index matching detected language, otherwise use most recent
    let index_dir = matching_indices
        .iter()
        .find(|(p, _)| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == preferred_pattern.as_str())
                .unwrap_or(false)
        })
        .map(|(p, _)| p.clone())
        .or_else(|| {
            // Sort by modification time (most recent first) and take first
            matching_indices.sort_by(|a, b| b.1.cmp(&a.1));
            matching_indices.first().map(|(p, _)| p.clone())
        })
        .ok_or_else(|| {
            anyhow!(
                "No index found for repository '{}'. Run 'nabi analyze repo {}' first.",
                repo_name,
                repo_path
            )
        })?;

    codegraph::load_index(index_dir.to_str().unwrap())
        .map_err(|e| anyhow!("Failed to load index: {}", e))
}

fn format_hash(input: &str) -> String {
    // Same hash function as analyze.rs
    let hash = input
        .chars()
        .fold(0u64, |acc, c| acc.wrapping_mul(31).wrapping_add(c as u64));
    // Pad to 8 characters with zeros, then take first 8
    format!("{:08x}", hash)[..8].to_string()
}

fn output_symbol_results(format: &str, results: &[&codegraph::Symbol]) -> Result<()> {
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&results)?;
            println!("{}", json);
        }
        _ => {
            for (i, symbol) in results.iter().enumerate() {
                let kind_str = match symbol.kind {
                    codegraph::SymbolKind::Function => "fn".cyan(),
                    codegraph::SymbolKind::Struct => "struct".magenta(),
                    codegraph::SymbolKind::Trait => "trait".blue(),
                    codegraph::SymbolKind::Enum => "enum".yellow(),
                    codegraph::SymbolKind::Module => "mod".green(),
                    _ => "other".white(),
                };

                println!(
                    "  {} {} {} at {}:{}",
                    (i + 1).to_string().dimmed(),
                    kind_str,
                    symbol.name.bold(),
                    symbol
                        .file
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?")
                        .white(),
                    symbol.line.to_string().dimmed()
                );
            }
        }
    }

    Ok(())
}
