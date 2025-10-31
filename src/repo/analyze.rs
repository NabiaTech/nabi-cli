/// Analyze repository - Index code and create searchable graph
///
/// Implements: nabi analyze repo <path> [--lang <language>]
///
/// Flow:
/// 1. Run pre-index validation hook
/// 2. Detect language (or use provided)
/// 3. Generate symbol index
/// 4. Save to ~/.cache/nabi/codebase-graphs/
/// 5. Run post-index finalization hook

use super::codegraph;
use anyhow::Result;
use colored::*;
use std::path::Path;

pub fn analyze(repo_path: &str, language: Option<&str>, _force: bool, format: &str) -> Result<()> {
    println!();
    println!("{}", "📚 Codebase Analysis".cyan().bold());
    println!("{}", "═".repeat(70).cyan());

    // Resolve to absolute path
    let path = Path::new(repo_path).canonicalize()
        .unwrap_or_else(|_| Path::new(repo_path).to_path_buf());

    if !path.exists() {
        println!("{}", format!("✗ Repository not found: {}", path.display()).red());
        return Ok(());
    }

    println!("{}", format!("Repository: {}", path.display()).white().bold());
    println!();

    // Detect language
    let detected_lang = language
        .map(|l| l.to_string())
        .unwrap_or_else(|| {
            codegraph::detect_language(repo_path).unwrap_or_else(|_| "rust".to_string())
        });

    println!("{}", format!("Language: {}", detected_lang).white());
    println!();

    // Run pre-index validation
    println!("{}", "Phase 1: Pre-Index Validation".bold().cyan());
    codegraph::run_pre_index_hook(repo_path, &detected_lang)?;
    println!("{}", "✓ Pre-index validation passed".green());
    println!();

    // Generate index
    println!("{}", "Phase 2: Index Generation".bold().cyan());
    let index = codegraph::generate_index(repo_path, &detected_lang)?;

    println!(
        "{}",
        format!("  ✓ Indexed {} symbols in {} files",
            index.metadata.symbol_count,
            index.metadata.file_count
        ).green()
    );
    println!();

    // Save index
    println!("{}", "Phase 3: Index Storage".bold().cyan());
    let cache_dir = codegraph::get_cache_dir()?;
    let repo_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let repo_hash = format_hash(repo_path);
    let index_dir = cache_dir.join(format!("{}-{}", repo_name, repo_hash));

    codegraph::save_index(index_dir.to_str().unwrap(), &index)?;
    println!("{}", format!("  ✓ Index saved to: {}", index_dir.display()).green());
    println!();

    // Run post-index finalization
    println!("{}", "Phase 4: Post-Index Finalization".bold().cyan());
    codegraph::run_post_index_hook(repo_path, index_dir.to_str().unwrap())?;
    println!("{}", "✓ Post-index finalization complete".green());
    println!();

    // Output summary
    output_result(format, &index)?;

    println!("{}", "═".repeat(70).cyan());
    println!("{}", "✓ Analysis complete".green().bold());
    println!();

    Ok(())
}

fn format_hash(input: &str) -> String {
    // Simple hash for now - in production would use actual git commit hash
    let hash = input.chars()
        .fold(0u64, |acc, c| acc.wrapping_mul(31).wrapping_add(c as u64));
    format!("{:x}", hash)[..8].to_string()
}

fn output_result(format: &str, index: &codegraph::CodegraphIndex) -> Result<()> {
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&index.metadata)?;
            println!("{}", json);
        }
        _ => {
            println!("{}", "Summary:".bold().cyan());
            println!("  Repository: {}", index.metadata.repository);
            println!("  Language: {}", index.metadata.language);
            println!("  Symbols: {}", index.metadata.symbol_count);
            println!("  Files: {}", index.metadata.file_count);
            println!("  Created: {}", index.metadata.created);

            if index.metadata.symbol_count > 0 && index.metadata.symbol_count <= 20 {
                println!();
                println!("{}", "Indexed symbols:".bold().cyan());
                for symbol in &index.symbols {
                    println!("  {} {} ({}:{})",
                        match symbol.kind {
                            codegraph::SymbolKind::Function => "fn",
                            codegraph::SymbolKind::Struct => "struct",
                            codegraph::SymbolKind::Trait => "trait",
                            codegraph::SymbolKind::Enum => "enum",
                            codegraph::SymbolKind::Module => "mod",
                            _ => "other",
                        },
                        symbol.name.bold(),
                        symbol.file.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("?"),
                        symbol.line
                    );
                }
            }
        }
    }

    Ok(())
}
