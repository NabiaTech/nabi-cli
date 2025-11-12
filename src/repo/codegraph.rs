/// Codegraph module - Index management and symbol graph operations
///
/// Handles:
/// - Index creation and management (in ~/.local/state/nabi/codegraph/)
/// - Hook execution (pre-index, post-index, pre-query)
/// - Symbol querying and analysis
/// - Federation integration with manifests
use anyhow::{anyhow, Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub line: usize,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub repository: String,
    pub language: String,
    pub created: String,
    pub symbol_count: usize,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodegraphIndex {
    pub metadata: IndexMetadata,
    pub symbols: Vec<Symbol>,
}

/// Get the cache directory for codegraph indices
pub fn get_cache_dir() -> Result<PathBuf> {
    let cache_dir = if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg_cache)
    } else {
        let home = std::env::var("HOME").context("HOME environment variable not set")?;
        PathBuf::from(home).join(".cache")
    };

    let codegraph_dir = cache_dir.join("nabi").join("codebase-graphs");
    fs::create_dir_all(&codegraph_dir)?;
    Ok(codegraph_dir)
}

/// Get the state directory for codegraph locks and manifests
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

    // Default to rust if cannot detect
    Ok("rust".to_string())
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
        // Pre-query hook is optional for MVP
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

/// Generate a simple index by walking the repository
/// This is an MVP implementation - full tree-sitter integration can come later
pub fn generate_index(repo_path: &str, language: &str) -> Result<CodegraphIndex> {
    let path = Path::new(repo_path).canonicalize()?;

    println!("{}", "  → Analyzing code structure...".cyan());

    let mut symbols = Vec::new();
    let mut file_count = 0;

    // Walk the repository and find symbols based on language
    for entry in walkdir::WalkDir::new(&path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();

        // Skip common directories
        if let Some(parent) = file_path.parent() {
            if let Some(parent_name) = parent.file_name() {
                let parent_str = parent_name.to_string_lossy();
                if parent_str.starts_with('.')
                    || ["node_modules", "target", "__pycache__", ".venv"]
                        .contains(&parent_str.as_ref())
                {
                    continue;
                }
            }
        }

        // Parse based on language
        match language {
            "rust" => {
                if file_path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    if let Ok(content) = fs::read_to_string(file_path) {
                        parse_rust_symbols(&mut symbols, file_path, &content)?;
                    }
                    file_count += 1;
                }
            }
            "python" => {
                if file_path.extension().and_then(|s| s.to_str()) == Some("py") {
                    if let Ok(content) = fs::read_to_string(file_path) {
                        parse_python_symbols(&mut symbols, file_path, &content)?;
                    }
                    file_count += 1;
                }
            }
            _ => {
                // Generic text search for other languages
                file_count += 1;
            }
        }
    }

    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let metadata = IndexMetadata {
        repository: repo_name,
        language: language.to_string(),
        created: chrono::Local::now().to_rfc3339(),
        symbol_count: symbols.len(),
        file_count,
    };

    Ok(CodegraphIndex { metadata, symbols })
}

/// Simple Rust symbol parser (basic pattern matching)
fn parse_rust_symbols(symbols: &mut Vec<Symbol>, file_path: &Path, content: &str) -> Result<()> {
    let mut line_num = 0;
    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        // Detect public functions
        if trimmed.starts_with("pub fn ") {
            if let Some(name) = trimmed
                .strip_prefix("pub fn ")
                .and_then(|s| s.split('(').next())
                .map(|s| s.trim())
            {
                symbols.push(Symbol {
                    id: format!("fn_{}", line_num),
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    file: file_path.to_path_buf(),
                    line: line_num,
                    visibility: "public".to_string(),
                });
            }
        }

        // Detect structs
        if trimmed.starts_with("pub struct ") {
            if let Some(name) = trimmed
                .strip_prefix("pub struct ")
                .and_then(|s| s.split(|c| c == '{' || c == '(').next())
                .map(|s| s.trim())
            {
                symbols.push(Symbol {
                    id: format!("struct_{}", line_num),
                    name: name.to_string(),
                    kind: SymbolKind::Struct,
                    file: file_path.to_path_buf(),
                    line: line_num,
                    visibility: "public".to_string(),
                });
            }
        }

        // Detect traits
        if trimmed.starts_with("pub trait ") {
            if let Some(name) = trimmed
                .strip_prefix("pub trait ")
                .and_then(|s| s.split(|c| c == '{' || c == ':').next())
                .map(|s| s.trim())
            {
                symbols.push(Symbol {
                    id: format!("trait_{}", line_num),
                    name: name.to_string(),
                    kind: SymbolKind::Trait,
                    file: file_path.to_path_buf(),
                    line: line_num,
                    visibility: "public".to_string(),
                });
            }
        }
    }

    Ok(())
}

/// Simple Python symbol parser (basic pattern matching)
fn parse_python_symbols(symbols: &mut Vec<Symbol>, file_path: &Path, content: &str) -> Result<()> {
    let mut line_num = 0;
    for line in content.lines() {
        line_num += 1;
        let trimmed = line.trim();

        // Detect class definitions
        if trimmed.starts_with("class ") {
            if let Some(name) = trimmed
                .strip_prefix("class ")
                .and_then(|s| s.split(|c| c == '(' || c == ':').next())
                .map(|s| s.trim())
            {
                symbols.push(Symbol {
                    id: format!("class_{}", line_num),
                    name: name.to_string(),
                    kind: SymbolKind::Struct,
                    file: file_path.to_path_buf(),
                    line: line_num,
                    visibility: "public".to_string(),
                });
            }
        }

        // Detect function definitions (top-level and class methods)
        if trimmed.starts_with("def ") && !trimmed.starts_with("def _") {
            if let Some(name) = trimmed
                .strip_prefix("def ")
                .and_then(|s| s.split('(').next())
                .map(|s| s.trim())
            {
                let visibility = if line.starts_with("def _") {
                    "private"
                } else {
                    "public"
                };
                symbols.push(Symbol {
                    id: format!("fn_{}", line_num),
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    file: file_path.to_path_buf(),
                    line: line_num,
                    visibility: visibility.to_string(),
                });
            }
        }
    }

    Ok(())
}

/// Load an index from disk
pub fn load_index(index_path: &str) -> Result<CodegraphIndex> {
    let path = PathBuf::from(index_path);
    let metadata_file = path.join("metadata.json");
    let symbols_file = path.join("symbols.json");

    let metadata: IndexMetadata = serde_json::from_str(
        &fs::read_to_string(&metadata_file).context("Failed to read metadata.json")?,
    )?;

    let symbols: Vec<Symbol> = serde_json::from_str(
        &fs::read_to_string(&symbols_file).context("Failed to read symbols.json")?,
    )?;

    Ok(CodegraphIndex { metadata, symbols })
}

/// Save an index to disk
pub fn save_index(index_path: &str, index: &CodegraphIndex) -> Result<()> {
    let path = PathBuf::from(index_path);
    fs::create_dir_all(&path)?;

    // Write metadata
    let metadata_file = path.join("metadata.json");
    fs::write(
        &metadata_file,
        serde_json::to_string_pretty(&index.metadata)?,
    )?;

    // Write symbols
    let symbols_file = path.join("symbols.json");
    fs::write(&symbols_file, serde_json::to_string_pretty(&index.symbols)?)?;

    Ok(())
}

/// Find a symbol by name
pub fn find_symbol<'a>(index: &'a CodegraphIndex, name: &str) -> Vec<&'a Symbol> {
    index
        .symbols
        .iter()
        .filter(|s| s.name.contains(name))
        .collect()
}

/// Find all references (for now, simple substring matching in symbol names)
/// In a real implementation, this would use call graph analysis
pub fn find_references<'a>(index: &'a CodegraphIndex, symbol: &str) -> Vec<&'a Symbol> {
    index
        .symbols
        .iter()
        .filter(|s| s.name.contains(symbol) || symbol.contains(&s.name))
        .collect()
}

/// Find related symbols (symbols that might be related by usage)
pub fn find_related<'a>(index: &'a CodegraphIndex, symbol: &str, _depth: usize) -> Vec<&'a Symbol> {
    // Simple implementation: find symbols in the same file or with similar names
    let matching = index
        .symbols
        .iter()
        .filter(|s| s.name.contains(symbol))
        .next();

    if let Some(main_symbol) = matching {
        return index
            .symbols
            .iter()
            .filter(|s| s.file == main_symbol.file || s.name.len() < symbol.len() + 10)
            .collect();
    }

    Vec::new()
}
