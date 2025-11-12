/// Codegraph module - Index management and symbol graph operations
///
/// **UNIFIED IMPLEMENTATION** - Uses codegraph-mcp's AST-based parser
///
/// Handles:
/// - Index creation via bun/make_graph.ts (AST parsing with edges)
/// - Hook execution (pre-index, post-index, pre-query)
/// - Symbol querying with full call graph support
/// - Federation integration with manifests
/// - Shared state with codegraph MCP server
use anyhow::{anyhow, Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ============================================================================
// Data Structures (matching codegraph-mcp graph.json format)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    #[serde(rename = "startLine")]
    pub start_line: usize,
    #[serde(rename = "startCol")]
    pub start_col: usize,
    #[serde(rename = "endLine")]
    pub end_line: usize,
    #[serde(rename = "endCol")]
    pub end_col: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub kind: String, // function, class, method, variable, module
    pub name: String,
    pub file: String, // relative path
    pub range: Range,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "parentId")]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub src: String,
    #[serde(rename = "type")]
    pub edge_type: String, // defines, call, import, member_of
    pub dst: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub symbols: Vec<Symbol>,
    pub edges: Vec<Edge>,
}

/// Index metadata for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub repository: String,
    pub created: String,
    pub symbol_count: usize,
    pub edge_count: usize,
}

/// Full codegraph index with metadata
#[derive(Debug, Clone)]
pub struct CodegraphIndex {
    pub metadata: IndexMetadata,
    pub graph: Graph,
}

// ============================================================================
// Directory Management
// ============================================================================

/// Get the state directory for codegraph indices and manifests
pub fn get_state_dir() -> Result<PathBuf> {
    let state_dir = if let Ok(xdg_state) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(xdg_state)
    } else {
        let home = std::env::var("HOME").context("HOME environment variable not set")?;
        PathBuf::from(home).join(".local").join("state")
    };

    let codegraph_dir = state_dir.join("nabi").join("codegraph");
    fs::create_dir_all(&codegraph_dir)?;
    Ok(codegraph_dir)
}

/// Get the graphs directory (unified with MCP server)
pub fn get_graphs_dir() -> Result<PathBuf> {
    let graphs_dir = get_state_dir()?.join("graphs");
    fs::create_dir_all(&graphs_dir)?;
    Ok(graphs_dir)
}

/// Get the hooks directory (XDG_DATA_HOME/nabi/bin)
fn get_hooks_dir() -> Result<PathBuf> {
    let data_dir = if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg_data)
    } else {
        let home = std::env::var("HOME").context("HOME environment variable not set")?;
        PathBuf::from(home).join(".local").join("share")
    };

    let hooks_dir = data_dir.join("nabi").join("bin");
    fs::create_dir_all(&hooks_dir)?;
    Ok(hooks_dir)
}

/// Get codegraph-mcp directory
fn get_codegraph_mcp_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    let mcp_dir = PathBuf::from(home).join("mcp-servers").join("codegraph-mcp");

    if !mcp_dir.exists() {
        return Err(anyhow!(
            "codegraph-mcp not found at {}. Please ensure it's installed.",
            mcp_dir.display()
        ));
    }

    Ok(mcp_dir)
}

// ============================================================================
// Language Detection
// ============================================================================

/// Detect the programming language of a repository
pub fn detect_language(repo_path: &str) -> Result<String> {
    let path = Path::new(repo_path);

    // Check for language indicators
    if path.join("Cargo.toml").exists() {
        return Ok("rust".to_string());
    }
    if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
        return Ok("python".to_string());
    }
    if path.join("go.mod").exists() {
        return Ok("go".to_string());
    }
    if path.join("package.json").exists() || path.join("tsconfig.json").exists() {
        return Ok("typescript".to_string());
    }

    // Default to python for mixed repos
    Ok("python".to_string())
}

// ============================================================================
// Hook Execution
// ============================================================================

/// Execute the pre-index validation hook
pub fn run_pre_index_hook(repo_path: &str, language: &str) -> Result<()> {
    let hooks_dir = get_hooks_dir()?;
    let hook_path = hooks_dir.join("codegraph-pre-index.sh");

    if !hook_path.exists() {
        eprintln!(
            "{}",
            format!("⚠  Pre-index hook not found: {}", hook_path.display()).yellow()
        );
        return Ok(());
    }

    println!("{}", "  → Running pre-index validation...".cyan());

    let output = Command::new("bash")
        .arg(hook_path)
        .arg(repo_path)
        .arg(language)
        .output()
        .context("Failed to execute pre-index hook")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Pre-index validation failed: {}", stderr));
    }

    Ok(())
}

/// Execute the post-index finalization hook
pub fn run_post_index_hook(repo_path: &str, index_path: &str) -> Result<()> {
    let hooks_dir = get_hooks_dir()?;
    let hook_path = hooks_dir.join("codegraph-post-index.sh");

    if !hook_path.exists() {
        eprintln!(
            "{}",
            format!("⚠  Post-index hook not found: {}", hook_path.display()).yellow()
        );
        return Ok(());
    }

    println!("{}", "  → Running post-index finalization...".cyan());

    let output = Command::new("bash")
        .arg(hook_path)
        .arg(repo_path)
        .arg(index_path)
        .output()
        .context("Failed to execute post-index hook")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", format!("⚠  Post-index warning: {}", stderr).yellow());
        // Non-blocking: continue even if post-index has issues
    }

    Ok(())
}

/// Execute the pre-query validation hook
pub fn run_pre_query_hook(repo_path: &str) -> Result<()> {
    let hooks_dir = get_hooks_dir()?;
    let hook_path = hooks_dir.join("codegraph-pre-query.sh");

    if !hook_path.exists() {
        // Pre-query hook is optional
        return Ok(());
    }

    let output = Command::new("bash")
        .arg(hook_path)
        .arg(repo_path)
        .output()
        .context("Failed to execute pre-query hook")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Pre-query validation failed: {}", stderr));
    }

    Ok(())
}

// ============================================================================
// Index Generation (AST-based via bun/make_graph.ts)
// ============================================================================

/// Generate index using codegraph-mcp's AST-based parser
pub fn generate_index(repo_path: &str, _language: &str) -> Result<CodegraphIndex> {
    let path = Path::new(repo_path).canonicalize()?;
    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    println!("{}", "  → Analyzing code structure (AST-based)...".cyan());

    // Determine output directory
    let graphs_dir = get_graphs_dir()?;
    let output_dir = graphs_dir.join(repo_name);
    fs::create_dir_all(&output_dir)?;

    // Get codegraph-mcp directory
    let mcp_dir = get_codegraph_mcp_dir()?;
    let make_graph_script = mcp_dir.join("src/ingest/make_graph.ts");

    if !make_graph_script.exists() {
        return Err(anyhow!(
            "make_graph.ts not found at {}",
            make_graph_script.display()
        ));
    }

    // Execute: bun run make_graph.ts --target <repo> --output <dir>
    let output = Command::new("bun")
        .arg("run")
        .arg(&make_graph_script)
        .arg("--target")
        .arg(&path)
        .arg("--output")
        .arg(&output_dir)
        .current_dir(&mcp_dir)
        .output()
        .context("Failed to execute bun make_graph.ts")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "Index generation failed:\nStdout: {}\nStderr: {}",
            stdout,
            stderr
        ));
    }

    // Load the generated graph
    let graph_file = output_dir.join("graph.json");
    if !graph_file.exists() {
        return Err(anyhow!(
            "graph.json not created at {}",
            graph_file.display()
        ));
    }

    let graph: Graph = serde_json::from_str(
        &fs::read_to_string(&graph_file).context("Failed to read graph.json")?,
    )?;

    let metadata = IndexMetadata {
        repository: repo_name.to_string(),
        created: chrono::Local::now().to_rfc3339(),
        symbol_count: graph.symbols.len(),
        edge_count: graph.edges.len(),
    };

    println!(
        "{}",
        format!(
            "  ✓ Indexed {} symbols with {} edges",
            metadata.symbol_count, metadata.edge_count
        )
        .green()
    );

    Ok(CodegraphIndex { metadata, graph })
}

// ============================================================================
// Index Loading
// ============================================================================

/// Load an index from disk (graph.json format)
pub fn load_index(repo_name: &str) -> Result<CodegraphIndex> {
    let graphs_dir = get_graphs_dir()?;
    let graph_dir = graphs_dir.join(repo_name);
    let graph_file = graph_dir.join("graph.json");

    if !graph_file.exists() {
        return Err(anyhow!(
            "Index not found for '{}'. Run 'nabi analyze repo' first.",
            repo_name
        ));
    }

    let graph: Graph = serde_json::from_str(
        &fs::read_to_string(&graph_file).context("Failed to read graph.json")?,
    )?;

    let metadata = IndexMetadata {
        repository: repo_name.to_string(),
        created: fs::metadata(&graph_file)?
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            .to_string(),
        symbol_count: graph.symbols.len(),
        edge_count: graph.edges.len(),
    };

    Ok(CodegraphIndex { metadata, graph })
}

// ============================================================================
// Query Functions
// ============================================================================

/// Find symbols by name (fuzzy matching)
pub fn find_symbol<'a>(index: &'a CodegraphIndex, name: &str) -> Vec<&'a Symbol> {
    let name_lower = name.to_lowercase();
    index
        .graph
        .symbols
        .iter()
        .filter(|s| s.name.to_lowercase().contains(&name_lower))
        .collect()
}

/// Find all references to a symbol using edges
pub fn find_references<'a>(index: &'a CodegraphIndex, symbol_id: &str) -> Vec<&'a Symbol> {
    // Find edges where this symbol is the destination (incoming references)
    let referencing_ids: Vec<&str> = index
        .graph
        .edges
        .iter()
        .filter(|e| e.dst == symbol_id)
        .map(|e| e.src.as_str())
        .collect();

    // Return symbols for those IDs
    let id_set: std::collections::HashSet<&str> = referencing_ids.into_iter().collect();
    index
        .graph
        .symbols
        .iter()
        .filter(|s| id_set.contains(s.id.as_str()))
        .collect()
}

/// Find related symbols (symbols connected via edges)
pub fn find_related<'a>(
    index: &'a CodegraphIndex,
    symbol_id: &str,
    depth: usize,
) -> Vec<&'a Symbol> {
    let mut visited = std::collections::HashSet::new();
    let mut current_ids = vec![symbol_id.to_string()];

    for _ in 0..depth {
        let mut next_ids = Vec::new();
        for id in &current_ids {
            if visited.contains(id.as_str()) {
                continue;
            }
            visited.insert(id.to_string());

            // Find connected symbols via edges
            for edge in &index.graph.edges {
                if edge.src == *id && !visited.contains(&edge.dst) {
                    next_ids.push(edge.dst.clone());
                }
                if edge.dst == *id && !visited.contains(&edge.src) {
                    next_ids.push(edge.src.clone());
                }
            }
        }
        current_ids = next_ids;
    }

    // Return symbols for visited IDs
    index
        .graph
        .symbols
        .iter()
        .filter(|s| visited.contains(&s.id))
        .collect()
}

/// Find symbol by exact ID
pub fn find_symbol_by_id<'a>(index: &'a CodegraphIndex, id: &str) -> Option<&'a Symbol> {
    index.graph.symbols.iter().find(|s| s.id == id)
}

// ============================================================================
// Legacy Compatibility (deprecated)
// ============================================================================

/// Old SymbolKind enum for backward compatibility
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    Macro,
    Constant,
    Type,
    Other,
}

impl SymbolKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "function" | "method" => Self::Function,
            "class" => Self::Struct,
            "module" => Self::Module,
            _ => Self::Other,
        }
    }
}
