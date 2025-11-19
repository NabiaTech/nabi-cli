/// Graph operations - Query indexed codebase for symbols and references
///
/// **UNIFIED IMPLEMENTATION** - Works with AST-based graph.json format
///
/// Implements:
/// - nabi repo graph search <symbol>
/// - nabi repo graph references <symbol>
/// - nabi repo graph related <symbol>
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

    // First find the symbol to get its ID
    let matching_symbols = codegraph::find_symbol(&index, symbol);
    if matching_symbols.is_empty() {
        println!(
            "{}",
            format!(
                "Symbol '{}' not found. Use 'search' to find symbols first.",
                symbol
            )
            .yellow()
        );
        return Ok(());
    }

    // Use first match
    let target_symbol = matching_symbols[0];
    println!(
        "{}",
        format!(
            "Finding references to: {} ({})",
            target_symbol.name, target_symbol.kind
        )
        .cyan()
    );
    println!();

    // Find references using the symbol ID
    let results = codegraph::find_references(&index, &target_symbol.id);

    if results.is_empty() {
        println!(
            "{}",
            format!("No references found for '{}'", symbol).yellow()
        );
        return Ok(());
    }

    println!("{}", format!("Found {} references:", results.len()).cyan());
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

    // First find the symbol to get its ID
    let matching_symbols = codegraph::find_symbol(&index, symbol);
    if matching_symbols.is_empty() {
        println!(
            "{}",
            format!(
                "Symbol '{}' not found. Use 'search' to find symbols first.",
                symbol
            )
            .yellow()
        );
        return Ok(());
    }

    // Use first match
    let target_symbol = matching_symbols[0];
    println!(
        "{}",
        format!(
            "Finding symbols related to: {} ({})",
            target_symbol.name, target_symbol.kind
        )
        .cyan()
    );
    println!();

    // Find related symbols using the symbol ID
    let results = codegraph::find_related(&index, &target_symbol.id, depth);

    if results.is_empty() {
        println!(
            "{}",
            format!("No related symbols found for '{}'", symbol).yellow()
        );
        return Ok(());
    }

    println!(
        "{}",
        format!("Found {} symbols (depth: {}):", results.len(), depth).cyan()
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

    // Get repository name
    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Detect language
    let detected_lang =
        codegraph::detect_language(repo_path).unwrap_or_else(|_| "python".to_string());

    // Load index from language-aware location
    codegraph::load_index(repo_path, &detected_lang).map_err(|e| {
        anyhow!(
            "Failed to load index for '{}' (language: {}): {}\nRun 'nabi analyze repo {}' first.",
            repo_name,
            detected_lang,
            e,
            repo_path
        )
    })
}

fn output_symbol_results(format: &str, results: &[&codegraph::Symbol]) -> Result<()> {
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&results)?;
            println!("{}", json);
        }
        _ => {
            for (i, symbol) in results.iter().enumerate() {
                let kind_str = match symbol.kind.as_str() {
                    "function" => "fn".cyan(),
                    "class" => "struct".magenta(),
                    "method" => "fn".cyan(),
                    "module" => "mod".green(),
                    "variable" => "var".yellow(),
                    _ => symbol.kind.as_str().white(),
                };

                println!(
                    "  {} {} {} at {}:{}",
                    (i + 1).to_string().dimmed(),
                    kind_str,
                    symbol.name.bold(),
                    symbol.file.white(),
                    symbol.range.start_line.to_string().dimmed()
                );
            }
        }
    }

    Ok(())
}
