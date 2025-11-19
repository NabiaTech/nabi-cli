/// Utility functions used across the CLI
///
/// Common helper functions for path manipulation, string formatting, etc.
use std::path::Path;

/// Convert a name to a URL-friendly slug
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in name.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
        } else if matches!(ch, ' ' | '-' | '_' | '.') {
            if !previous_dash && !slug.is_empty() {
                slug.push('-');
                previous_dash = true;
            }
        }
    }
    slug.trim_matches('-').to_string()
}

/// Shell-quote a path for safe use in shell commands
pub fn shell_quote(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let needs_quotes = raw.chars().any(|c| {
        matches!(
            c,
            ' ' | '"' | '\'' | '(' | ')' | '$' | '`' | '!' | '&' | ';' | '<' | '>' | '|'
        )
    });
    if !needs_quotes {
        raw.to_string()
    } else {
        let escaped = raw.replace('\'', "'\\''");
        format!("'{}'", escaped)
    }
}

/// Derive a tool name from a file path
pub fn derive_tool_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|os| os.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| "tool".to_string())
}
