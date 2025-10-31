/// XDG Base Directory Specification compliant path resolution
///
/// Provides centralized, environment-aware path resolution for nabi-cli.
/// Respects XDG_CONFIG_HOME, XDG_DATA_HOME, XDG_STATE_HOME, XDG_CACHE_HOME
/// with intelligent fallbacks for cross-platform compatibility.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// XDG-compliant path resolver for nabi system directories
pub struct NabiPaths;

impl NabiPaths {
    /// Get XDG_CONFIG_HOME/nabi directory (operational config, git-managed)
    ///
    /// Priority:
    /// 1. $XDG_CONFIG_HOME/nabi (environment override)
    /// 2. ~/.config/nabi (Linux/macOS default)
    /// 3. Error if neither available
    pub fn config_dir() -> Result<PathBuf> {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|_| {
                dirs::config_dir()
                    .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))
            })
            .map(|p| p.join("nabi"))
            .context("Failed to resolve nabi config directory")
    }

    /// Get XDG_DATA_HOME/nabi directory (persistent application data)
    ///
    /// Priority:
    /// 1. $XDG_DATA_HOME/nabi (environment override)
    /// 2. ~/.local/share/nabi (Linux/macOS default)
    /// 3. Error if neither available
    pub fn data_dir() -> Result<PathBuf> {
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|_| {
                dirs::data_dir()
                    .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))
            })
            .map(|p| p.join("nabi"))
            .context("Failed to resolve nabi data directory")
    }

    /// Get XDG_STATE_HOME/nabi directory (session-scoped state)
    ///
    /// Priority:
    /// 1. $XDG_STATE_HOME/nabi (environment override)
    /// 2. ~/.local/state/nabi (Linux/macOS default)
    /// 3. Error if neither available
    pub fn state_dir() -> Result<PathBuf> {
        std::env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|_| {
                dirs::state_dir()
                    .ok_or_else(|| anyhow::anyhow!("Could not determine state directory"))
            })
            .map(|p| p.join("nabi"))
            .context("Failed to resolve nabi state directory")
    }

    /// Get XDG_CACHE_HOME/nabi directory (temporary cache)
    ///
    /// Priority:
    /// 1. $XDG_CACHE_HOME/nabi (environment override)
    /// 2. ~/.cache/nabi (Linux/macOS default)
    /// 3. Error if neither available
    pub fn cache_dir() -> Result<PathBuf> {
        std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|_| {
                dirs::cache_dir()
                    .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))
            })
            .map(|p| p.join("nabi"))
            .context("Failed to resolve nabi cache directory")
    }

    /// Get nabi CLI binary directory (~/.local/bin)
    ///
    /// Used for installing CLI wrappers and executables
    pub fn bin_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .context("Could not determine home directory")?;
        Ok(home.join(".local").join("bin"))
    }

    /// Get nabi venv directory (~/.nabi/venvs)
    ///
    /// Stores Python virtual environments for tool runtime isolation
    pub fn venv_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .context("Could not determine home directory")?;
        Ok(home.join(".nabi").join("venvs"))
    }

    /// Get home directory (fallback for operations that need it)
    pub fn home_dir() -> Result<PathBuf> {
        dirs::home_dir()
            .context("Could not determine home directory")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir_default() {
        let config = NabiPaths::config_dir().expect("config_dir failed");
        assert!(config.to_string_lossy().contains("nabi"));
        assert!(config.to_string_lossy().contains("config"));
    }

    #[test]
    fn test_data_dir_default() {
        let data = NabiPaths::data_dir().expect("data_dir failed");
        assert!(data.to_string_lossy().contains("nabi"));
        assert!(data.to_string_lossy().contains("share"));
    }

    #[test]
    fn test_state_dir_default() {
        let state = NabiPaths::state_dir().expect("state_dir failed");
        assert!(state.to_string_lossy().contains("nabi"));
        assert!(state.to_string_lossy().contains("state"));
    }

    #[test]
    fn test_cache_dir_default() {
        let cache = NabiPaths::cache_dir().expect("cache_dir failed");
        assert!(cache.to_string_lossy().contains("nabi"));
        assert!(cache.to_string_lossy().contains("cache"));
    }
}
