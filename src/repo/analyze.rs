/// Analyze repository - Index code and create searchable graph
///
/// **UNIFIED IMPLEMENTATION** - Uses AST-based parser via codegraph-mcp
///
/// Implements: nabi analyze repo <path> [--lang <language>]
///
/// Flow:
/// 1. Run pre-index validation hook
/// 2. Detect language (or use provided)
/// 3. Generate symbol index via bun/make_graph.ts (AST-based)
/// 4. Save to ~/.local/state/nabi/codegraph/graphs/{repo-name}/
/// 5. Run post-index finalization hook
use super::codegraph;
use anyhow::Result;
use colored::*;
use std::path::Path;

pub fn analyze(repo_path: &str, language: Option<&str>, force: bool, format: &str) -> Result<()> {
    println!();
    println!("{}", "📚 Codebase Analysis".cyan().bold());
    println!("{}", "═".repeat(70).cyan());

    // Resolve to absolute path
    let path = Path::new(repo_path)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(repo_path).to_path_buf());

    if !path.exists() {
        println!(
            "{}",
            format!("✗ Repository not found: {}", path.display()).red()
        );
        return Ok(());
    }

    println!(
        "{}",
        format!("Repository: {}", path.display()).white().bold()
    );
    println!();

    // Detect language
    let detected_lang = language.map(|l| l.to_string()).unwrap_or_else(|| {
        codegraph::detect_language(repo_path).unwrap_or_else(|_| "python".to_string())
    });

    println!("{}", format!("Language: {}", detected_lang).white());
    println!();

    // Get repository name
    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Check if index already exists (with language-aware path)
    let graphs_dir = codegraph::get_graphs_dir()?;
    let repo_hash = codegraph::compute_repo_hash(repo_path)?;
    let cache_dir_name = format!("{}-{}-{}", repo_name, repo_hash, &detected_lang);
    let index_dir = graphs_dir.join(&cache_dir_name);
    let graph_file = index_dir.join("graph.json");
    let index_exists = graph_file.exists();

    let index = if index_exists && !force {
        // Reuse existing index
        println!("{}", "Phase 1: Loading Cached Index".bold().cyan());
        println!(
            "{}",
            format!("  → Language: {}", detected_lang).cyan()
        );
        let cached_index = codegraph::load_index(repo_path, &detected_lang)?;
        println!(
            "{}",
            format!(
                "  ✓ Loaded {} symbols with {} edges",
                cached_index.metadata.symbol_count, cached_index.metadata.edge_count
            )
            .green()
        );
        println!(
            "{}",
            format!("  Created: {}", cached_index.metadata.created).dimmed()
        );
        println!("{}", "  → Use --force to rebuild the index".dimmed());
        println!();
        cached_index
    } else {
        // Generate new index
        if index_exists && force {
            println!("{}", "Phase 1: Force Rebuild".bold().cyan());
            println!(
                "{}",
                format!("  → Rebuilding index (--force specified)").yellow()
            );
            println!();
        }

        // Run pre-index validation
        println!("{}", "Phase 1: Pre-Index Validation".bold().cyan());
        codegraph::run_pre_index_hook(repo_path, &detected_lang)?;
        println!("{}", "✓ Pre-index validation passed".green());
        println!();

        // Generate index (AST-based via bun)
        println!("{}", "Phase 2: Index Generation".bold().cyan());
        let new_index = codegraph::generate_index(repo_path, &detected_lang)?;
        println!();

        // Run post-index finalization
        println!("{}", "Phase 3: Post-Index Finalization".bold().cyan());
        codegraph::run_post_index_hook(repo_path, index_dir.to_str().unwrap())?;
        println!("{}", "✓ Post-index finalization complete".green());
        println!();

        new_index
    };

    // Output summary
    output_result(format, &index)?;

    println!("{}", "═".repeat(70).cyan());
    println!("{}", "✓ Analysis complete".green().bold());
    println!();

    Ok(())
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
            println!("  Symbols: {}", index.metadata.symbol_count);
            println!("  Edges: {}", index.metadata.edge_count);
            println!("  Created: {}", index.metadata.created);

            if index.metadata.symbol_count > 0 && index.metadata.symbol_count <= 20 {
                println!();
                println!("{}", "Indexed symbols:".bold().cyan());
                for symbol in index.graph.symbols.iter().take(20) {
                    let kind_str = match symbol.kind.as_str() {
                        "function" => "fn",
                        "class" => "class",
                        "method" => "method",
                        "module" => "mod",
                        "variable" => "var",
                        _ => &symbol.kind,
                    };
                    println!(
                        "  {} {} ({}:{})",
                        kind_str.dimmed(),
                        symbol.name.bold(),
                        symbol.file,
                        symbol.range.start_line
                    );
                }
            }
        }
    }

    Ok(())
}
