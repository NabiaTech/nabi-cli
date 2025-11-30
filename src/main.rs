use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Arg, Args, Command, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell as CompletionShell};
use colored::*;
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process;

mod cli;
mod commands;
mod deckgen;
mod forge;
mod handlers;
mod maturity;
mod paths;
mod repo;
mod routing;
mod spec;
mod transform;
mod utils;
use commands::events;
use commands::kernel;
use commands::mcp;
use commands::port;
use commands::services;
use commands::tmux;
use cli::{AuraCommands, BackupCommands, ScanSourceType, ValidateCommands, WatchCommands};
use handlers::aura::handle_aura;
use paths::NabiPaths;

/// nabi - Unified Federation Command Gateway
///
/// Router to specialized commanders following the principle:
/// "Router, not monolith. Coordination, not control." — Igris
#[derive(Parser)]
#[command(name = "nabi")]
#[command(version, about, long_about = None)]
#[command(author = "Nabia Federation")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Claude domain operations (sessions, projects, recovery)
    Claude {
        #[command(subcommand)]
        command: ClaudeCommands,
    },
    /// Data operations (JSONL, JSON, format conversion)
    Data {
        #[command(subcommand)]
        command: DataCommands,
    },
    /// Federation coordination (agents, memory, monitoring)
    Federation {
        #[command(subcommand)]
        command: FederationCommands,
    },
    /// Self-management (update, doctor, config)
    #[command(name = "self")]
    SelfManage {
        #[command(subcommand)]
        command: SelfCommands,
    },
    /// Feature flag management (enable/disable dynamic features)
    Forge {
        #[command(subcommand)]
        command: ForgeCommands,
    },
    /// Documentation and manifest operations
    Docs {
        #[command(subcommand)]
        command: DocsCommands,
    },
    /// Repository compliance validation
    Repo {
        #[command(subcommand)]
        command: RepoCommands,
    },
    /// Codebase analysis and indexing (alias for 'repo analyze')
    #[command(
        long_about = "Fast-track access to repository analysis and code intelligence features.\n\n\
                      This is a convenience alias for 'nabi repo analyze' providing quick access \
                      to code indexing, symbol search, and multi-agent analysis workflows.\n\n\
                      See 'nabi analyze repo --help' for detailed options.",
        after_help = "EXAMPLES:\n  \
                      nabi analyze repo ~/nabia/core\n  \
                      nabi analyze repo . --lang rust\n\n\
                      EQUIVALENT TO:\n  \
                      nabi repo analyze ~/nabia/core\n  \
                      nabi repo analyze . --lang rust\n\n\
                      RELATED:\n  \
                      nabi repo analyze - Full command path\n  \
                      nabi repo graph    - Query the indexed code graph"
    )]
    Analyze {
        #[command(subcommand)]
        command: AnalyzeCommands,
    },
    /// Tool registry operations
    Tool {
        #[command(subcommand)]
        command: ToolCommands,
    },
    /// Registration aliases (e.g., `nabi register tool`)
    Register {
        #[command(subcommand)]
        command: RegisterCommands,
    },
    /// Execute a registered tool (shorthand for `nabi tool exec`)
    Exec {
        /// Tool ID or command name
        tool: String,
        /// Arguments to pass to the tool
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Filesystem scanning and metadata generation
    ///
    /// Examples:
    ///   nabi scan --all                      # Scan all federation dirs (includes ~/docs)
    ///   nabi scan --docs nats                # Search docs for 'nats'
    ///   nabi scan --docs --type md jetstream # Search markdown for 'jetstream'
    Scan {
        /// Path to scan
        #[arg(value_name = "PATH")]
        path: Option<String>,
        /// Tags to add
        #[arg(short, long)]
        tags: Option<String>,
        /// Confidence level (0-1)
        #[arg(long)]
        confidence: Option<f32>,
        /// Scan all federation directories (quick tree overview)
        #[arg(long)]
        all: bool,
        /// Search documentation using ripgrep
        #[arg(long)]
        docs: bool,
        /// Filter by file type(s) (comma-separated extensions)
        #[arg(long, value_delimiter = ',', value_enum)]
        type_filter: Option<Vec<ScanSourceType>>,
        /// Exact string matching (skip query enhancement)
        #[arg(long, short = 'e')]
        exact: bool,
        /// Search query for docs search
        #[arg(value_name = "QUERY", last = true)]
        query: Option<String>,
    },
    /// File watching and real-time classification
    Watch {
        #[command(subcommand)]
        command: Option<WatchCommands>,
    },
    /// Organize files/directories with timestamp prefixes and topology categories
    ///
    /// Renames files/directories using their latest modification time as prefix
    /// followed by inferred topology categories for better chronological ordering.
    Orgtime {
        /// Path to organize (directory or file)
        #[arg(value_name = "PATH")]
        path: String,
        /// Custom topology category (auto-inferred if not provided)
        #[arg(short, long)]
        category: Option<String>,
        /// Operate on files within directory instead of renaming the directory itself
        #[arg(long)]
        files: bool,
        /// Preserve original modification times during rename operations
        #[arg(long)]
        preserve_times: bool,
        /// Dry run mode (show what would be done without executing)
        #[arg(long)]
        dry_run: bool,
    },
    /// Manage AURAs (Automated semantic context bubbles)
    Aura {
        #[command(subcommand)]
        command: AuraCommands,
    },
    /// Configuration management
    Configure {
        #[command(subcommand)]
        command: ConfigureCommands,
    },
    /// Database operations
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },
    /// Terminal session recording (tvmux)
    Record {
        #[command(subcommand)]
        command: RecordCommands,
    },
    /// Backup and recovery operations (federation data protection)
    ///
    /// Comprehensive backup system with XDG-compliant storage, dual-format archiving
    /// (ditto.zip for macOS fidelity + tar.tgz for portability), external drive support,
    /// and NATS queue integration for federation-wide coordination.
    Backup {
        #[command(subcommand)]
        command: BackupCommands,
    },
    /// NABIKernel agent operations (daemon, spawn, status)
    Agent {
        #[command(subcommand)]
        command: AgentKernelCommands,
    },
    /// Port registry management and validation
    Port {
        #[command(subcommand)]
        command: PortCommands,
    },
    /// Service orchestration (docker-compose deployment and management)
    ///
    /// Centralized service deployment and validation for federation infrastructure.
    /// Manages docker-compose stacks across platform, core, and memchain groups.
    Services {
        #[command(subcommand)]
        command: ServicesCommands,
    },
    /// Tmux pane coordination (multi-agent orchestration)
    ///
    /// Safely inject commands into tmux panes with guaranteed atomic delivery.
    /// Prevents race conditions by sending text and Enter in a single operation.
    /// Perfect for coordinating work across multiple agents.
    Tmux {
        #[command(subcommand)]
        command: tmux::TmuxCommands,
    },
    /// Microkernel monitoring (memchain vs isolated panes)
    Kernel {
        #[command(subcommand)]
        command: kernel::KernelCommands,
    },
    /// Federation event bus (temporal awareness for Claude sessions)
    Events {
        #[command(subcommand)]
        command: events::EventsCommands,
    },
    /// MCP federation commands (event publishing, task dispatch, acknowledgments)
    Mcp {
        #[command(subcommand)]
        command: mcp::McpCommands,
    },
    Hooks {
        #[command(subcommand)]
        command: HooksCommands,
    },
    /// Mode management (manual/auto commit enforcement)
    Mode {
        /// Mode to switch to (manual or auto)
        #[arg(value_name = "MODE")]
        mode: Option<String>,
    },
    /// Search Claude conversations & repair JSONL sessions (riff-cli)
    Riff {
        /// Riff subcommand and arguments (passed through to riff-cli)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Recover crashed Claude sessions
    Recover {
        #[command(subcommand)]
        command: RecoverCommands,
    },
    /// Health check operations (federation substrate validation and reporting)
    Health {
        #[command(subcommand)]
        command: HealthCommands,
    },
    /// Generate shell completion scripts
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: CompletionShell,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Install to shell-specific location
        #[arg(long)]
        install: bool,
    },
    /// deckgen utilities (schema + trace fixtures)
    #[command(
        long_about = "Generate test fixtures and schemas for pipeline validation.\n\n\
                      Deckgen provides utilities for creating deterministic test data that can be \
                      used to validate schema transformations, trace processing, and integration \
                      workflows. All fixtures are seeded for consistency and reproducibility.",
        after_help = "EXAMPLES:\n  \
                      nabi deckgen trace\n  \
                      nabi deckgen trace --output test-fixtures/trace.json\n\n\
                      USES:\n  \
                      • Create test fixtures for CI/CD pipelines\n  \
                      • Validate schema transformations\n  \
                      • Benchmark trace processing\n  \
                      • Generate regression test data\n\n\
                      RELATED:\n  \
                      nabi repo analyze  - Analyze actual repositories\n  \
                      nabi docs manifest - Generate repository manifests"
    )]
    Deckgen {
        #[command(subcommand)]
        command: DeckgenCommands,
    },
    /// Health check (alias for 'self doctor')
    #[command(visible_alias = "doc")]
    Doctor,
    /// Migrate directories from XDG_STATE_HOME to XDG_DATA_HOME
    ///
    /// Safely migrates directories from state to data with conflict detection,
    /// path traversal protection, and dry-run support.
    Migrate {
        #[command(subcommand)]
        command: cli::MigrateCommands,
    },
    /// Validation operations (TOML syntax and schema validation)
    Validate {
        #[command(subcommand)]
        command: ValidateCommands,
    },
}

#[derive(Subcommand)]
enum ClaudeCommands {
    /// Session management operations
    Session {
        #[command(subcommand)]
        action: SessionActions,
    },
    /// Project operations
    Project {
        #[command(subcommand)]
        action: ProjectActions,
    },
}

#[derive(Subcommand)]
enum SessionActions {
    /// List available sessions
    #[command(
        long_about = "Display all available Claude sessions with quick reference information.\n\n\
                      Shows:\n  \
                      • Session UUID\n  \
                      • Creation timestamp\n  \
                      • Last activity time\n  \
                      • Message count\n  \
                      • Current status (active, archived)\n\n\
                      Sessions include tab autocomplete for easy recovery.",
        after_help = "EXAMPLES:\n  \
                      nabi claude session list\n  \
                      nabi claude session list --limit 20\n\n\
                      TIPS:\n  \
                      • Use session UUIDs with 'nabi claude session recover'\n  \
                      • Sessions are searchable and filterable\n\n\
                      RELATED:\n  \
                      nabi claude session recover - Restore a session\n  \
                      nabi claude session view    - View session details"
    )]
    List {
        /// Limit number of sessions to display
        #[arg(short, long, default_value = "10", value_name = "COUNT")]
        limit: usize,
    },
    /// Recover a session by UUID
    #[command(
        long_about = "Recover and restore a Claude session by its UUID.\n\n\
                      This command:\n  \
                      • Loads the session state\n  \
                      • Restores conversation context\n  \
                      • Reconnects to the session memory layer\n  \
                      • Enables resuming work from where you left off\n\n\
                      Tab completion supports session UUIDs from 'nabi claude session list'.",
        after_help = "EXAMPLES:\n  \
                      nabi claude session recover 550e8400-e29b-41d4-a716-446655440000\n\n\
                      TIPS:\n  \
                      • Use the first 8 characters for partial matching\n  \
                      • Press TAB for autocomplete of recent sessions\n  \
                      • Session state is automatically persisted\n\n\
                      RELATED:\n  \
                      nabi claude session list   - See all available sessions\n  \
                      nabi claude session view   - View session details before recovering"
    )]
    Recover {
        /// Session UUID to recover (supports tab autocomplete)
        #[arg(value_name = "UUID")]
        uuid: String,
    },
    /// View session details
    #[command(
        long_about = "Display detailed information about a specific Claude session.\n\n\
                      Shows:\n  \
                      • Full session metadata\n  \
                      • Message history summary\n  \
                      • Token usage statistics\n  \
                      • Memory footprint\n  \
                      • Associated files and context",
        after_help = "EXAMPLES:\n  \
                      nabi claude session view 550e8400-e29b-41d4-a716-446655440000\n\n\
                      RELATED:\n  \
                      nabi claude session list    - List all sessions\n  \
                      nabi claude session recover - Restore this session"
    )]
    View {
        /// Session UUID to view
        #[arg(value_name = "UUID")]
        uuid: String,
    },
}

#[derive(Subcommand)]
enum ProjectActions {
    /// List all projects
    List,
    /// Migrate project to new path
    Migrate {
        /// New project path
        path: String,
    },
}

#[derive(Subcommand)]
enum DataCommands {
    /// JSONL operations
    Jsonl {
        #[command(subcommand)]
        action: JsonlActions,
    },
}

#[derive(Subcommand)]
enum JsonlActions {
    /// Validate JSONL file
    Validate {
        /// File to validate
        file: String,
    },
    /// Repair broken JSONL file
    Repair {
        /// File to repair
        file: String,
        /// Output file (defaults to file.fixed)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Interactive viewer for JSONL
    View {
        /// File to view
        file: String,
        /// Search query
        #[arg(short, long)]
        query: Option<String>,
    },
}

#[derive(Subcommand)]
enum FederationCommands {
    /// Agent coordination
    Agent {
        #[command(subcommand)]
        action: AgentActions,
    },
    /// Syncthing folder control
    Sync {
        #[command(subcommand)]
        action: SyncActions,
    },
    /// Service registry operations
    Registry {
        #[command(subcommand)]
        action: RegistryActions,
    },
    /// Federation-wide health checks
    Health,
    /// Show status of all federation nodes
    Status,
    /// List all active agents in the federation
    Agents,
    /// Validate federation configuration and state
    Validate,
    /// Open federation dashboard (Grafana visualization)
    Dashboard {
        /// Port to access dashboard
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

#[derive(Subcommand)]
enum DocsCommands {
    /// Manifest management (SHA256 tracking)
    #[command(
        long_about = "Manage repository manifests for integrity tracking and change detection.\n\n\
                      Manifests track file SHA256 hashes to detect when repository content \
                      changes, enabling automated compliance checking and documentation updates."
    )]
    Manifest {
        #[command(subcommand)]
        action: ManifestActions,
    },
}

#[derive(Subcommand)]
enum ManifestActions {
    /// List all manifests
    #[command(
        long_about = "Display all generated manifests and their metadata.\n\n\
                      Shows:\n  \
                      • Repository paths\n  \
                      • Generation timestamps\n  \
                      • File counts and sizes\n  \
                      • Hash information for integrity verification",
        after_help = "EXAMPLES:\n  \
                      nabi docs manifest list\n\n\
                      RELATED:\n  \
                      nabi docs manifest generate  - Create a new manifest\n  \
                      nabi docs manifest validate  - Check against existing manifest"
    )]
    List,
    /// Validate a repository against its manifest
    #[command(
        long_about = "Verify that repository files match their recorded manifests.\n\n\
                      This command:\n  \
                      • Compares current file hashes to manifest records\n  \
                      • Detects added, modified, or deleted files\n  \
                      • Reports integrity violations\n  \
                      • Useful for compliance checks and change detection",
        after_help = "EXAMPLES:\n  \
                      nabi docs manifest validate ~/nabia/memchain\n  \
                      nabi docs manifest validate .\n\n\
                      Exit codes:\n  \
                      • 0: Validation passed\n  \
                      • 1: Validation failed (files changed)\n\n\
                      RELATED:\n  \
                      nabi docs manifest generate - Update manifest after changes\n  \
                      nabi docs manifest list     - View all manifests"
    )]
    Validate {
        /// Repository path to validate
        #[arg(value_name = "REPO_PATH")]
        repo_path: String,
    },
    /// Generate a manifest for a repository
    #[command(
        long_about = "Create or update a manifest for a repository.\n\n\
                      Scans all files in the repository and records:\n  \
                      • SHA256 hash for each file\n  \
                      • File sizes and modification times\n  \
                      • Directory structure\n  \
                      • Exclusion patterns (.gitignore)\n\n\
                      Use this after making significant changes to update the baseline.",
        after_help = "EXAMPLES:\n  \
                      nabi docs manifest generate ~/nabia/memchain\n  \
                      nabi docs manifest generate .  # Current directory\n\n\
                      The manifest is stored at:\n  \
                      <repo>/.manifests/manifest.json\n\n\
                      RELATED:\n  \
                      nabi docs manifest validate - Verify against this manifest\n  \
                      nabi docs manifest list     - View all manifests"
    )]
    Generate {
        /// Repository path to generate for
        #[arg(value_name = "REPO_PATH")]
        repo_path: String,
    },
}

#[derive(Subcommand)]
enum RepoCommands {
    /// Check repository compliance (XDG, hardcoded paths)
    Check {
        /// Path to repository (default: current directory)
        #[arg(value_name = "PATH")]
        path: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Strict mode (fail on warnings)
        #[arg(long)]
        strict: bool,
    },

    /// Index a repository for code analysis (creates persistent graph)
    ///
    /// Analyzes a codebase and creates a searchable symbol index. The index is cached
    /// and reused on subsequent runs unless --force is specified. Multiple analyses of
    /// the same repository with different languages are stored separately to avoid
    /// overwriting. This enables multi-agent workflows where agents can share cached
    /// analysis results.
    Analyze {
        /// Path to repository to analyze
        #[arg(value_name = "PATH")]
        repo_path: String,

        /// Language hint (auto-detect if not provided: rust, python, go, typescript)
        #[arg(short, long)]
        lang: Option<String>,

        /// Force re-indexing (rebuild index even if cached version exists)
        #[arg(long)]
        force: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Query an indexed codebase (search symbols, find references)
    Graph {
        #[command(subcommand)]
        action: GraphActions,
    },
    /// Codegraph hook management (deploy, validate, show configuration)
    Codegraph {
        #[command(subcommand)]
        command: CodegraphCommands,
    },
}

#[derive(Subcommand)]
enum CodegraphCommands {
    /// Deploy hooks from schema: render templates and generate executable scripts
    ///
    /// This command implements Phase 2 of the schema-driven hook system.
    /// It reads hook template configurations from ~/.config/nabi/codegraph.toml,
    /// renders Handlebars templates with configured variables, and deploys
    /// generated scripts to ~/.local/share/nabi/bin/ with proper permissions.
    DeployHooks {
        /// Verbose output (show each hook as it's deployed)
        #[arg(short, long)]
        verbose: bool,

        /// Config path (defaults to ~/.config/nabi/codegraph.toml)
        #[arg(long)]
        config: Option<String>,
    },
    /// Validate deployed hooks (check syntax, permissions, execution)
    ValidateHooks {
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Show current hook configuration from schema
    ShowHooks {
        /// Show full template contents
        #[arg(long)]
        templates: bool,
    },
}

#[derive(Subcommand)]
enum AnalyzeCommands {
    /// Index a repository for code analysis (creates persistent graph)
    #[command(
        long_about = "Analyze a codebase and create a searchable symbol index for code intelligence.\n\n\
                      This command:\n  \
                      • Scans the repository and extracts symbols, functions, and definitions\n  \
                      • Creates a persistent code graph index for fast lookups\n  \
                      • Caches results to reuse on subsequent runs (unless --force)\n  \
                      • Supports multi-agent workflows through shared cache\n  \
                      • Auto-detects language or accepts explicit hints\n\n\
                      Useful for:\n  \
                      • Symbol search and code navigation\n  \
                      • Finding references and dependencies\n  \
                      • Multi-agent code analysis workflows\n  \
                      • Integration with IDE-like features",
        after_help = "EXAMPLES:\n  \
                      nabi analyze repo ~/nabia/core\n  \
                      nabi analyze repo . --lang rust\n  \
                      nabi analyze repo ~/project --force\n  \
                      nabi analyze repo ~/project --format json\n\n\
                      CACHING:\n  \
                      Indexes are cached automatically in ~/.local/state/nabi/\n  \
                      Use --force to rebuild even if cache exists.\n\n\
                      RELATED:\n  \
                      nabi repo analyze    - Alternative command path\n  \
                      nabi repo graph      - Query the code graph"
    )]
    Repo {
        /// Path to repository to analyze
        #[arg(value_name = "PATH")]
        repo_path: String,

        /// Language hint (auto-detect if not provided: rust, python, go, typescript)
        #[arg(short, long, value_name = "LANG")]
        lang: Option<String>,

        /// Force re-indexing (rebuild index even if cached version exists)
        #[arg(long)]
        force: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
}

#[derive(Subcommand)]
enum GraphActions {
    /// Search for a symbol in the index
    Search {
        /// Symbol name to search for
        symbol: String,

        /// Repository path (auto-detect from current directory if not provided)
        #[arg(short, long)]
        repo: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Find all references to a symbol
    References {
        /// Symbol to find references for
        symbol: String,

        /// Repository path (auto-detect if not provided)
        #[arg(short, long)]
        repo: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Find related symbols (called by or calling the given symbol)
    Related {
        /// Symbol name
        symbol: String,

        /// Repository path (auto-detect if not provided)
        #[arg(short, long)]
        repo: Option<String>,

        /// Search depth (how many levels to traverse)
        #[arg(short, long, default_value = "2")]
        depth: usize,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum AgentActions {
    /// List active agents
    List,
    /// Spawn new agent
    Spawn {
        /// Agent role
        role: String,
    },
}

#[derive(Subcommand)]
enum SyncActions {
    /// List all Syncthing folders and their states
    List,
    /// Pause a Syncthing folder
    Pause {
        /// Folder ID (e.g., "nabia-federation")
        folder: String,
    },
    /// Resume a Syncthing folder
    Resume {
        /// Folder ID (e.g., "nabia-federation")
        folder: String,
    },
    /// Show detailed folder status
    Status {
        /// Folder ID (optional, shows all if omitted)
        folder: Option<String>,
    },
}

#[derive(Subcommand)]
enum RegistryActions {
    /// List all registered services
    List,
    /// Show service health status
    Health,
    /// Add a service to registry
    Add {
        /// Service name
        name: String,
        /// Service type (syncthing, agent, etc.)
        #[arg(short, long)]
        service_type: String,
    },
    /// Remove a service from registry
    Remove {
        /// Service name
        name: String,
    },
}

#[derive(Subcommand)]
enum SelfCommands {
    /// Health check all commanders
    #[command(
        long_about = "Run comprehensive health checks on your federation infrastructure.\n\n\
                      Validates:\n  \
                      • Network connectivity (Tailscale, local services)\n  \
                      • Service availability (Loki, Grafana, coordination server)\n  \
                      • Port allocations and conflicts\n  \
                      • Configuration coherence\n  \
                      • Hook system status\n  \
                      • Federation node connectivity",
        after_help = "EXAMPLES:\n  \
                      nabi self doctor\n\n\
                      Output includes status codes:\n  \
                      ✓ = Healthy\n  \
                      ⚠ = Warning (degraded but functional)\n  \
                      ✗ = Critical (action required)\n\n\
                      RELATED:\n  \
                      nabi health       - Alias for this command\n  \
                      nabi port check   - Check port configuration specifically\n  \
                      nabi federation status - Check federation node status"
    )]
    Doctor,
    /// Update all components
    #[command(
        long_about = "Update nabi-cli and all registered tools to latest versions.\n\n\
                      Updates:\n  \
                      • The CLI itself\n  \
                      • Registered external tools\n  \
                      • Dependencies and schemas\n\n\
                      This operation is safe and can be run during development."
    )]
    Update,
    /// Show configuration
    #[command(
        long_about = "Display current nabi configuration and environment.\n\n\
                      Shows:\n  \
                      • Active configuration paths\n  \
                      • Environment variables\n  \
                      • CLI version\n  \
                      • Loaded schemas",
        after_help = "EXAMPLES:\n  \
                      nabi self config\n\n\
                      Configuration is read from (in order):\n  \
                      1. ~/.config/nabi/\n  \
                      2. ~/.nabi/ (symlinks to XDG directories)\n  \
                      3. Built-in defaults"
    )]
    Config,
    /// Generate CLI command specification
    #[command(
        long_about = "Generate a specification of all CLI commands in markdown or JSON format.\n\n\
                      Useful for:\n  \
                      • Documentation generation\n  \
                      • Integration with external tools\n  \
                      • CI/CD automation\n  \
                      • Command discovery",
        after_help = "EXAMPLES:\n  \
                      nabi self spec\n  \
                      nabi self spec --format json | jq '.'\n  \
                      nabi self spec --format markdown > docs/CLI_REFERENCE.md\n\n\
                      RELATED:\n  \
                      nabi --help  - Interactive help\n  \
                      nabi <cmd> --help - Help for specific command"
    )]
    Spec {
        /// Output format (markdown or json)
        #[arg(value_enum, default_value_t = SpecFormat::Markdown)]
        format: SpecFormat,
    },
    /// Comprehensive shell diagnostics (function, binary, aura, health)
    #[command(
        long_about = "Run comprehensive diagnostics on nabi shell integration.\n\n\
                      Checks:\n  \
                      • Nabi function definition in shell\n  \
                      • Binary availability in PATH\n  \
                      • Active aura configuration\n  \
                      • System health check (unless --quick)\n\n\
                      This helps debug nabi installation and shell integration issues."
    )]
    Diagnose {
        /// Quick mode (skip health check)
        #[arg(long)]
        quick: bool,
        /// Show full aura details
        #[arg(long)]
        aura: bool,
        /// Output format (text or json)
        #[arg(long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum ForgeCommands {
    /// Enable a feature flag
    Enable {
        /// Feature name to enable
        feature: String,
    },
    /// Disable a feature flag
    Disable {
        /// Feature name to disable
        feature: String,
    },
    /// Show status of all feature flags
    Status,
    /// List available feature flags
    List,
}

// AuraCommands moved to cli.rs

#[derive(Subcommand)]
enum ConfigureCommands {
    /// Show current configuration
    Show,
    /// Set configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Reset to defaults
    Reset,
}

#[derive(Subcommand)]
enum DbCommands {
    /// Initialize database
    Init,
    /// Export database
    Export {
        /// Export file path
        path: String,
    },
    /// Import database
    Import {
        /// Import file path
        path: String,
    },
}

#[derive(Subcommand)]
enum RecordCommands {
    /// Start recording current tmux window
    #[command(
        long_about = "Begin recording terminal session activity in the active tmux window.\n\n\
                      Records all input and output for later playback. Useful for:\n  \
                      • Session persistence\n  \
                      • Workflow documentation\n  \
                      • Debugging and incident analysis\n  \
                      • Creating training materials",
        after_help = "EXAMPLES:\n  \
                      nabi record start\n  \
                      nabi record start --output ~/session-$(date +%s).cast\n\n\
                      The default output format is asciinema (.cast) which is widely compatible.\n\n\
                      RELATED:\n  \
                      nabi record stop   - Stop active recording\n  \
                      nabi record list   - View all recordings\n  \
                      nabi record server - Manage tvmux server"
    )]
    Start {
        /// Optional output file path (defaults to auto-generated)
        #[arg(short, long, value_name = "PATH")]
        output: Option<String>,
    },
    /// Stop active recording(s)
    #[command(
        long_about = "Stop terminal session recording.\n\n\
                      • Without ID: stops all active recordings\n  \
                      • With ID: stops specific recording by ID",
        after_help = "EXAMPLES:\n  \
                      nabi record stop          # Stop all recordings\n  \
                      nabi record stop abc123   # Stop specific recording\n\n\
                      RELATED:\n  \
                      nabi record start - Start a new recording\n  \
                      nabi record list  - View recording IDs"
    )]
    Stop {
        /// Recording ID to stop (all if omitted)
        #[arg(value_name = "ID")]
        id: Option<String>,
    },
    /// List all active recordings
    #[command(
        long_about = "Display all currently active terminal recordings.\n\n\
                      Shows:\n  \
                      • Recording ID\n  \
                      • Start time\n  \
                      • Output file path\n  \
                      • Status (active, paused, etc.)",
        after_help = "EXAMPLES:\n  \
                      nabi record list\n\n\
                      RELATED:\n  \
                      nabi record start - Start a new recording\n  \
                      nabi record stop  - Stop an active recording"
    )]
    List,
    /// Manage tvmux server
    #[command(long_about = "Control the tvmux recording server daemon.\n\n\
                      The server manages recording sessions and must be running for \
                      the record command to work properly.")]
    Server {
        #[command(subcommand)]
        action: ServerActions,
    },
    /// Configuration
    #[command(long_about = "Configure tvmux recording parameters and behavior.")]
    Config {
        #[command(subcommand)]
        action: ConfigActions,
    },
}

#[derive(Subcommand)]
enum ServerActions {
    /// Start the tvmux server
    #[command(
        long_about = "Start the tvmux recording daemon.\n\n\
                      Enables recording functionality for terminal sessions.",
        after_help = "EXAMPLES:\n  \
                      nabi record server start\n\n\
                      RELATED:\n  \
                      nabi record server stop   - Stop the server\n  \
                      nabi record server status - Check server status"
    )]
    Start,
    /// Stop the tvmux server
    #[command(
        long_about = "Stop the tvmux recording daemon.\n\n\
                      Stops all active recordings and disables new recordings.",
        after_help = "EXAMPLES:\n  \
                      nabi record server stop\n\n\
                      RELATED:\n  \
                      nabi record server start  - Start the server\n  \
                      nabi record server status - Check server status"
    )]
    Stop,
    /// Check server status
    #[command(
        long_about = "Check whether the tvmux recording server is running.\n\n\
                      Displays:\n  \
                      • Server PID\n  \
                      • Memory usage\n  \
                      • Active recordings count\n  \
                      • Uptime",
        after_help = "EXAMPLES:\n  \
                      nabi record server status\n\n\
                      RELATED:\n  \
                      nabi record server start - Start the server\n  \
                      nabi record server stop  - Stop the server"
    )]
    Status,
}

#[derive(Subcommand)]
enum ConfigActions {
    /// Show tvmux configuration
    #[command(
        long_about = "Display current tvmux configuration.\n\n\
                      Shows all active settings and their values.",
        after_help = "EXAMPLES:\n  \
                      nabi record config show\n\n\
                      RELATED:\n  \
                      nabi record config set - Change configuration"
    )]
    Show,
    /// Set configuration value
    #[command(
        long_about = "Update a tvmux configuration parameter.\n\n\
                      Configuration changes take effect immediately.",
        after_help = "EXAMPLES:\n  \
                      nabi record config set output-dir ~/recordings\n  \
                      nabi record config set compression gzip\n\n\
                      RELATED:\n  \
                      nabi record config show - View current configuration"
    )]
    Set {
        /// Configuration key to set
        #[arg(value_name = "KEY")]
        key: String,
        /// New configuration value
        #[arg(value_name = "VALUE")]
        value: String,
    },
}

#[derive(Subcommand)]
enum AgentKernelCommands {
    /// Daemon control (start, stop, restart, status)
    Daemon {
        #[command(subcommand)]
        action: DaemonActions,
    },
    /// Spawn a new agent
    Spawn {
        /// Agent type
        agent_type: String,
        /// Task description
        #[arg(short, long)]
        task: Option<String>,
        /// Priority (low, normal, high, critical)
        #[arg(short, long, default_value = "normal")]
        priority: String,
    },
    /// Check agent status
    Status {
        /// Agent ID
        agent_id: String,
    },
    /// List all agents
    List,
    /// Kill an agent
    Kill {
        /// Agent ID
        agent_id: String,
    },
    /// Wait for agent completion
    Wait {
        /// Agent ID
        agent_id: String,
    },
}

#[derive(Subcommand)]
enum DaemonActions {
    /// Start the NABIKernel daemon
    Start {
        /// Run in foreground
        #[arg(short, long)]
        foreground: bool,
    },
    /// Stop the NABIKernel daemon
    Stop,
    /// Restart the NABIKernel daemon
    Restart,
    /// Check daemon status
    Status,
}

#[derive(Subcommand)]
enum PortCommands {
    /// List all registered ports
    #[command(
        long_about = "Display all port allocations across platforms with current status.\n\n\
                      Shows service names, assigned ports, protocols, and platform-specific \
                      overrides. Useful for understanding the complete federation topology.",
        after_help = "EXAMPLES:\n  \
                      nabi port list              # List all ports\n  \
                      nabi port list --platform rpi\n\n\
                      RELATED:\n  \
                      nabi port check       - Validate ports on current platform\n  \
                      nabi port cross-platform - Check for cross-platform conflicts"
    )]
    List {
        /// Platform filter (macos, wsl, rpi)
        #[arg(short, long, value_name = "PLATFORM")]
        platform: Option<String>,
    },
    /// Validate port allocations on current platform
    #[command(
        long_about = "Perform health checks on all port allocations for your platform.\n\n\
                      Validates that:\n  \
                      • All required services have ports assigned\n  \
                      • No local port conflicts exist\n  \
                      • Services match the registry schema",
        after_help = "EXAMPLES:\n  \
                      nabi port check\n\n\
                      RELATED:\n  \
                      nabi port list          - View all port allocations\n  \
                      nabi port cross-platform - Check conflicts across all platforms"
    )]
    Check,
    /// Check cross-platform conflicts
    #[command(
        long_about = "Analyze port usage across all platforms (macOS, WSL, RPi).\n\n\
                      Detects scenarios where:\n  \
                      • Same port is used on different platforms\n  \
                      • Port ranges conflict\n  \
                      • Services should use different ports for isolation",
        after_help = "EXAMPLES:\n  \
                      nabi port cross-platform\n\n\
                      RELATED:\n  \
                      nabi port check - Validate current platform only\n  \
                      nabi port drift - Investigate historical conflicts"
    )]
    CrossPlatform,
    /// Safely migrate service to new port
    #[command(
        long_about = "Migrate a service from one port to another safely.\n\n\
                      This command:\n  \
                      • Updates port registry atomically\n  \
                      • Validates new port is available\n  \
                      • Supports dry-run to preview changes\n  \
                      • Records migration in federation state",
        after_help = "EXAMPLES:\n  \
                      nabi port shift grafana 3000 3002\n  \
                      nabi port shift loki 3100 3101 --dry-run\n\n\
                      NOTE: Use --dry-run first to verify the change before committing.\n\n\
                      RELATED:\n  \
                      nabi port check  - Validate new configuration\n  \
                      nabi port list   - View all allocations"
    )]
    Shift {
        /// Service name to migrate
        #[arg(value_name = "SERVICE")]
        service: String,
        /// Current port number
        #[arg(value_name = "OLD_PORT")]
        old_port: u16,
        /// New port number
        #[arg(value_name = "NEW_PORT")]
        new_port: u16,
        /// Dry run - preview without executing
        #[arg(long)]
        dry_run: bool,
    },
    /// Perform forensic analysis of drift period
    #[command(
        long_about = "Analyze port configuration drift during a specific time window.\n\n\
                      Useful for:\n  \
                      • Understanding when conflicts appeared\n  \
                      • Tracing root cause of port migration issues\n  \
                      • Auditing historical changes\n  \
                      • Planning recovery procedures",
        after_help = "EXAMPLES:\n  \
                      nabi port drift\n  \
                      nabi port drift --forensic\n  \
                      nabi port drift --since '2 days ago'\n  \
                      nabi port drift --forensic --since '1 week ago'\n\n\
                      RELATED:\n  \
                      nabi port fix   - Auto-generate fix commands\n  \
                      nabi port check - Validate current state"
    )]
    Drift {
        /// Enable detailed forensic analysis with event logs
        #[arg(long)]
        forensic: bool,
        /// Time range to analyze (e.g., "2 days ago", "1 week ago")
        #[arg(long, value_name = "TIME")]
        since: Option<String>,
    },
    /// Auto-generate fix commands for conflicts
    #[command(
        long_about = "Analyze current port conflicts and generate automated fix commands.\n\n\
                      This command:\n  \
                      • Detects conflicts between services\n  \
                      • Suggests non-breaking migrations\n  \
                      • Outputs shell commands ready to execute\n  \
                      • Prioritizes by impact and risk",
        after_help = "EXAMPLES:\n  \
                      nabi port fix\n\n\
                      The output will be shell commands like:\n  \
                      nabi port shift service 3000 3002\n\n\
                      RELATED:\n  \
                      nabi port shift - Execute migration manually\n  \
                      nabi port drift - Investigate root causes"
    )]
    Fix,
    /// Generate .env file for docker-compose
    #[command(
        long_about = "Generate environment variables for docker-compose configuration.\n\n\
                      Creates a .env file with all port mappings and service endpoints,\n\
                      making it easy to keep docker-compose in sync with registry.",
        after_help = "EXAMPLES:\n  \
                      nabi port generate-env > .env\n  \
                      nabi port generate-env | tee .env\n\n\
                      The generated .env contains mappings like:\n  \
                      GRAFANA_PORT=3002\n  \
                      LOKI_PORT=3100\n  \
                      SERVICE_ENDPOINT=http://localhost:8100\n\n\
                      RELATED:\n  \
                      nabi port list   - View all allocations\n  \
                      nabi port check  - Validate configuration"
    )]
    GenerateEnv,
}

#[derive(Subcommand)]
enum ServicesCommands {
    /// Show status of all running containers
    #[command(
        long_about = "Display all running Docker containers with their status and ports.\n\n\
                      Shows container names, health status, and port mappings. Useful for \
                      understanding current deployment state across all service groups.",
        after_help = "EXAMPLES:\n  \
                      nabi services status\n  \
                      nabi services status --format json\n\n\
                      RELATED:\n  \
                      nabi services validate - Check compose file validity\n  \
                      nabi services deploy   - Deploy service groups"
    )]
    Status {
        /// Output format (table or json)
        #[arg(short, long, value_name = "FORMAT")]
        format: Option<String>,
    },
    /// Validate docker-compose files
    #[command(
        long_about = "Validate all docker-compose files for syntax and configuration errors.\n\n\
                      Checks compose files across platform, core, and memchain service groups. \
                      Ensures all files are valid before deployment.",
        after_help = "EXAMPLES:\n  \
                      nabi services validate\n\n\
                      RELATED:\n  \
                      nabi services deploy - Deploy after validation\n  \
                      nabi port check     - Validate port allocations"
    )]
    Validate,
    /// Rebuild container images
    #[command(
        long_about = "Rebuild Docker images for a service group.\n\n\
                      Runs docker-compose build for selected services. Use this after code \
                      changes that require image rebuilding.",
        after_help = "EXAMPLES:\n  \
                      nabi services rebuild all\n  \
                      nabi services rebuild platform\n  \
                      nabi services rebuild memchain\n\n\
                      GROUPS:\n  \
                      all        - All services\n  \
                      monitoring - Loki, Prometheus, Grafana\n  \
                      platform   - SurrealDB, Knowledge Graph, Vigil\n  \
                      core       - OAuth MCP Proxy\n  \
                      memchain   - MCP SSE, Coordination Server\n\n\
                      RELATED:\n  \
                      nabi services deploy - Deploy after rebuild"
    )]
    Rebuild {
        /// Service group to rebuild
        #[arg(value_name = "GROUP", default_value = "all")]
        group: String,
    },
    /// Deploy service group
    #[command(
        long_about = "Deploy services using docker-compose up -d.\n\n\
                      Automatically checks Docker networks exist, creates them if missing, \
                      then deploys selected service groups. Shows deployment status and \
                      container health after completion.",
        after_help = "EXAMPLES:\n  \
                      nabi services deploy all\n  \
                      nabi services deploy platform\n  \
                      nabi services deploy monitoring\n\n\
                      GROUPS:\n  \
                      all        - All services\n  \
                      monitoring - Loki, Prometheus, Grafana\n  \
                      platform   - SurrealDB, Knowledge Graph, Vigil\n  \
                      core       - OAuth MCP Proxy\n  \
                      memchain   - MCP SSE, Coordination Server\n\n\
                      RELATED:\n  \
                      nabi services status   - Check deployment status\n  \
                      nabi services validate - Validate before deploy"
    )]
    Deploy {
        /// Service group to deploy
        #[arg(value_name = "GROUP", default_value = "all")]
        group: String,
    },
}

#[derive(Subcommand)]
enum HooksCommands {
    /// Transform hooks from schema to derived state (or use stable hooks)
    Transform {
        /// Use stable hooks from ~/.nabi/src/hooks instead of transforming
        #[arg(short, long)]
        stable: bool,
    },
    /// Debug on-the-fly hooks (tail logs, search, stats, errors)
    Debug {
        #[command(subcommand)]
        action: HookDebugActions,
    },
}

#[derive(Subcommand)]
enum HookDebugActions {
    /// Tail debug logs for a hook in real-time
    Tail {
        /// Hook name to tail logs for
        hook_name: String,
    },
    /// List all debug log files
    List,
    /// Search debug logs for a term
    Search {
        /// Hook name to search
        hook_name: String,
        /// Search term
        term: String,
    },
    /// Show only error logs
    Errors {
        /// Hook name to show errors for
        hook_name: String,
    },
    /// Show execution statistics
    Stats {
        /// Hook name to show stats for
        hook_name: String,
    },
    /// View replay data (input/output capture)
    Replay {
        /// Hook name to show replay data for
        hook_name: String,
    },
    /// Clear all debug logs
    Clear,
    /// Enable debug mode (prints env vars to set)
    Enable {
        /// Debug level (info, verbose, trace)
        #[arg(default_value = "info")]
        level: String,
    },
    /// Disable debug mode (prints env vars to unset)
    Disable,
}

#[derive(Subcommand)]
enum DeckgenCommands {
    /// Emit the canonical seeded deckgen trace fixture
    #[command(
        long_about = "Generate the canonical seeded deckgen trace fixture for testing.\n\n\
                      This command produces a deterministic trace JSON file that can be used for:\n  \
                      • Testing schema transformations\n  \
                      • Validating trace processing pipelines\n  \
                      • Creating reproducible test fixtures\n  \
                      • Benchmarking analysis tools\n\n\
                      The fixture is seeded for consistency across runs, making it ideal for \
                      regression testing and continuous integration.",
        after_help = "EXAMPLES:\n  \
                      nabi deckgen trace\n  \
                      nabi deckgen trace --output ~/test-trace.json\n  \
                      nabi deckgen trace -o trace.json | jq '.'\n\n\
                      OUTPUT:\n  \
                      Generates a JSON file with:\n  \
                      • Canonical trace events\n  \
                      • Seeded random data for reproducibility\n  \
                      • Full schema validation\n\n\
                      USES:\n  \
                      • Test fixture generation\n  \
                      • Pipeline validation\n  \
                      • Regression testing"
    )]
    Trace {
        /// Optional file path to write the trace JSON; stdout if omitted
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum RecoverCommands {
    /// Recover recent Claude sessions
    Sessions {
        /// Look back this many hours
        #[arg(long, default_value = "24")]
        hours: usize,
        /// Show detailed tool and file information
        #[arg(long)]
        detailed: bool,
        /// Export report to file
        #[arg(long)]
        export: Option<String>,
    },
}

#[derive(Subcommand)]
enum HealthCommands {
    /// Quick bootstrap health check (replaces nabi doctor)
    Quick,

    /// Validate hooks, schemas, and transforms (replaces health check)
    Substrate {
        /// Auto-remediate critical issues
        #[arg(long)]
        auto_remediate: bool,
        /// Only show FSM state changes (don't run checks)
        #[arg(long)]
        fsm_only: bool,
    },

    /// Federation service registry health check
    Services,

    /// Port allocation and conflict detection
    Ports,

    /// [DEPRECATED] Run federation substrate health checks
    Check {
        /// Auto-remediate critical issues
        #[arg(long)]
        auto_remediate: bool,
        /// Only show FSM state changes (don't run checks)
        #[arg(long)]
        fsm_only: bool,
    },
    /// Show health check status and recent reports
    Status {
        /// Show detailed results
        #[arg(short, long)]
        detailed: bool,
        /// Look back this many hours
        #[arg(long, default_value = "24")]
        hours: usize,
    },
    /// Generate health report
    Report {
        /// Output format (text, json, markdown)
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Export to file
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Open Grafana dashboard with live health visualization
    Dashboard {
        /// Port to access Grafana
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

#[derive(Subcommand)]
enum ToolCommands {
    /// Register a tool manifest for federation routing
    Register(ToolRegisterArgs),
    /// List all registered tools
    List {
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Filter by status (active, inactive, deprecated)
        #[arg(short, long)]
        status: Option<String>,
        /// Filter by runtime (python, rust, bash, etc)
        #[arg(short, long)]
        runtime: Option<String>,
    },
    /// Execute a registered tool
    Exec {
        /// Tool ID or command name
        tool: String,
        /// Arguments to pass to the tool
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Promote a tool to production (deploy artifacts from source)
    Promote {
        /// Tool ID to promote (e.g., 'cursorignore', 'claude-manager')
        tool_id: String,
        /// Override version from TOML config
        #[arg(long)]
        version: Option<String>,
        /// Override promotion mode (LIVE, STABLE, INSTALL)
        #[arg(long, value_parser = parse_promotion_mode)]
        mode: Option<String>,
    },
}

#[derive(Subcommand)]
enum RegisterCommands {
    /// Register a tool manifest (alias for `nabi tool register`)
    Tool(ToolRegisterArgs),
}

#[derive(Clone, Debug, Args)]
struct ToolRegisterArgs {
    /// Path to the tool executable, script, or module entry point
    #[arg(value_name = "PATH")]
    path: String,
    /// Human-friendly tool name (defaults to inferred name)
    #[arg(long)]
    name: Option<String>,
    /// CLI command name to expose (defaults to slug)
    #[arg(long)]
    command: Option<String>,
    /// Explicit runtime selection (auto-detected if omitted)
    #[arg(long, value_enum)]
    runtime: Option<RuntimeKind>,
    /// Runtime version hint (defaults to inferred baseline)
    #[arg(long)]
    runtime_version: Option<String>,
    /// Version string to record in manifest
    #[arg(long)]
    version: Option<String>,
    /// Description to include in manifest metadata
    #[arg(long)]
    description: Option<String>,
    /// Comma separated tags for discovery and coordination
    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,
    /// Repository URL or reference source (optional)
    #[arg(long)]
    repository: Option<String>,
    /// Entry point identifier (module/function) if different from command
    #[arg(long)]
    entry_point: Option<String>,
    /// Execution string (e.g., "python -m foo") if custom
    #[arg(long)]
    execution: Option<String>,
    /// Preferred virtual environment location
    #[arg(long)]
    venv: Option<String>,
    /// Installer hint for provisioning the runtime environment
    #[arg(long)]
    installer: Option<String>,
    /// Override manifest schema version
    #[arg(long)]
    schema_version: Option<String>,
    /// Override manifest status (defaults to "active")
    #[arg(long)]
    status: Option<String>,
    /// Show what would change if manifest exists (requires manual review)
    #[arg(long)]
    force: bool,
    /// Actually overwrite existing customized manifests (DANGEROUS - bypasses safety checks)
    #[arg(long)]
    force_overwrite: bool,
    /// Mark capabilities: federation-aware tool
    #[arg(long)]
    federation_aware: bool,
    /// Mark capabilities: aura compatible
    #[arg(long)]
    aura_compatible: bool,
    /// Mark capabilities: XDG compliant
    #[arg(long)]
    xdg_compliant: bool,
    /// Mark capabilities: hook integrated
    #[arg(long)]
    hook_integrated: bool,
    /// Mark capabilities: cross-platform support
    #[arg(long)]
    cross_platform: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum RuntimeKind {
    Bash,
    Python,
    Rust,
    Typescript,
    Dotnet,
    Node,
    Deno,
    Go,
    Ruby,
    Other,
}

impl RuntimeKind {
    fn as_language(self) -> &'static str {
        match self {
            RuntimeKind::Bash => "bash",
            RuntimeKind::Python => "python",
            RuntimeKind::Rust => "rust",
            RuntimeKind::Typescript => "typescript",
            RuntimeKind::Dotnet => "dotnet",
            RuntimeKind::Node => "node",
            RuntimeKind::Deno => "deno",
            RuntimeKind::Go => "go",
            RuntimeKind::Ruby => "ruby",
            RuntimeKind::Other => "other",
        }
    }

    fn default_version_hint(self) -> &'static str {
        match self {
            RuntimeKind::Python => "3.11+",
            RuntimeKind::Bash => "5.x",
            RuntimeKind::Rust => "1.75+",
            RuntimeKind::Typescript => "5.x",
            RuntimeKind::Dotnet => "6.0+",
            RuntimeKind::Node => "18+",
            RuntimeKind::Deno => "1.41+",
            RuntimeKind::Go => "1.22+",
            RuntimeKind::Ruby => "3.1+",
            RuntimeKind::Other => "unspecified",
        }
    }

    fn default_execution(self, path: &Path) -> String {
        let path_str = shell_quote(path);
        match self {
            RuntimeKind::Python => format!("python3 {}", path_str),
            RuntimeKind::Bash => format!("bash {}", path_str),
            RuntimeKind::Rust => path_str,
            RuntimeKind::Typescript => format!("tsx {}", path_str),
            RuntimeKind::Dotnet => format!("dotnet {}", path_str),
            RuntimeKind::Node => format!("node {}", path_str),
            RuntimeKind::Deno => format!("deno run {}", path_str),
            RuntimeKind::Go => format!("go run {}", path_str),
            RuntimeKind::Ruby => format!("ruby {}", path_str),
            RuntimeKind::Other => path_str,
        }
    }
}

impl fmt::Display for RuntimeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_language())
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Claude { command } => handle_claude(command),
        Commands::Data { command } => handle_data(command),
        Commands::Federation { command } => handle_federation(command),
        Commands::SelfManage { command } => handle_self(command),
        Commands::Forge { command } => handle_forge(command),
        Commands::Docs { command } => handle_docs(command),
        Commands::Repo { command } => handle_repo(command),
        Commands::Analyze { command } => handle_analyze(command),
        Commands::Tool { command } => handle_tool(command),
        Commands::Validate { command } => handle_validate(command),
        Commands::Register { command } => handle_register(command),
        Commands::Exec { tool, args } => handle_tool_exec(&tool, args),
        Commands::Scan {
            path,
            tags,
            confidence,
            all,
            docs,
            type_filter,
            exact,
            query,
        } => handle_scan(path, tags, confidence, all, docs, type_filter, query, exact),
        Commands::Watch { command } => handlers::watch::handle_watch(command),
        Commands::Orgtime { path, category, files, preserve_times, dry_run } => {
            handle_orgtime(path, category, files, preserve_times, dry_run)
        }
        Commands::Aura { command } => handle_aura(command),
        Commands::Configure { command } => handle_configure(command),
        Commands::Db { command } => handle_db(command),
        Commands::Record { command } => handle_record(command),
        Commands::Backup { command } => handle_backup(command),
        Commands::Agent { command } => handle_agent(command),
        Commands::Port { command } => handle_port(command),
        Commands::Services { command } => handle_services(command),
        Commands::Tmux { command } => tmux::handle_tmux_commands(command),
        Commands::Kernel { command } => kernel::handle_kernel_commands(command),
        Commands::Events { command } => events::handle_events_commands(command),
        Commands::Mcp { command } => mcp::handle_mcp_commands(command),
        Commands::Hooks { command } => handle_hooks(command),
        Commands::Mode { mode } => handle_mode(mode),
        Commands::Riff { args } => handle_riff(args),
        Commands::Recover { command } => handle_recover(command),
        Commands::Health { command } => handle_health(command),
        Commands::Completions { shell, output, install } => handle_completions(shell, output, install),
        Commands::Deckgen { command } => handle_deckgen(command),
        Commands::Doctor => {
            println!("⚠️  DEPRECATED: Use 'nabi health quick' instead");
            println!("This command will be removed in 6 months
");
            handle_self(SelfCommands::Doctor)
        },
        Commands::Migrate { command } => handle_migrate(command),
    }
}

fn handle_claude(command: ClaudeCommands) -> Result<()> {
    match command {
        ClaudeCommands::Session { action } => match action {
            SessionActions::List { limit } => {
                println!("{}", "📋 Listing Claude sessions...".cyan().bold());
                route_to_commander(
                    "claude",
                    &["session", "list", "--limit", &limit.to_string()],
                )
            }
            SessionActions::Recover { uuid } => {
                println!(
                    "{}",
                    format!("🔄 Recovering session {}...", uuid).cyan().bold()
                );
                route_to_commander("claude", &["session", "recover", &uuid])
            }
            SessionActions::View { uuid } => {
                println!(
                    "{}",
                    format!("👁  Viewing session {}...", uuid).cyan().bold()
                );
                route_to_commander("claude", &["session", "view", &uuid])
            }
        },
        ClaudeCommands::Project { action } => match action {
            ProjectActions::List => {
                println!("{}", "📂 Listing Claude projects...".cyan().bold());
                route_to_commander("claude", &["project", "list"])
            }
            ProjectActions::Migrate { path } => {
                println!(
                    "{}",
                    format!("📦 Migrating project to {}...", path).cyan().bold()
                );
                route_to_commander("claude", &["project", "migrate", &path])
            }
        },
    }
}

fn handle_data(command: DataCommands) -> Result<()> {
    match command {
        DataCommands::Jsonl { action } => match action {
            JsonlActions::Validate { file } => {
                println!("{}", format!("✓ Validating {}...", file).green().bold());
                route_to_commander("data", &["jsonl", "validate", &file])
            }
            JsonlActions::Repair { file, output } => {
                let output_file = output.unwrap_or_else(|| format!("{}.fixed", file));
                println!(
                    "{}",
                    format!("🔧 Repairing {} -> {}...", file, output_file)
                        .yellow()
                        .bold()
                );
                route_to_commander(
                    "data",
                    &["jsonl", "repair", &file, "--output", &output_file],
                )
            }
            JsonlActions::View { file, query } => {
                println!("{}", format!("👁  Viewing {}...", file).cyan().bold());
                let mut args = vec!["jsonl", "view", &file];
                let query_str;
                if let Some(ref q) = query {
                    query_str = q.clone();
                    args.extend_from_slice(&["--query", &query_str]);
                }
                route_to_commander("data", &args)
            }
        },
    }
}

fn handle_federation(command: FederationCommands) -> Result<()> {
    match command {
        FederationCommands::Agent { action } => match action {
            AgentActions::List => {
                println!("{}", "🤖 Listing federation agents...".magenta().bold());
                route_to_commander("federation", &["agent", "list"])
            }
            AgentActions::Spawn { role } => {
                println!(
                    "{}",
                    format!("🚀 Spawning {} agent...", role).magenta().bold()
                );
                route_to_commander("federation", &["agent", "spawn", &role])
            }
        },
        FederationCommands::Sync { action } => match action {
            SyncActions::List => {
                println!("{}", "📂 Listing Syncthing folders...".cyan().bold());
                route_to_commander("federation", &["sync", "list"])
            }
            SyncActions::Pause { folder } => {
                println!(
                    "{}",
                    format!("⏸️  Pausing folder {}...", folder).yellow().bold()
                );
                route_to_commander("federation", &["sync", "pause", &folder])
            }
            SyncActions::Resume { folder } => {
                println!(
                    "{}",
                    format!("▶️  Resuming folder {}...", folder).green().bold()
                );
                route_to_commander("federation", &["sync", "resume", &folder])
            }
            SyncActions::Status { folder } => {
                if let Some(ref f) = folder {
                    println!("{}", format!("📊 Status for {}...", f).cyan().bold());
                    route_to_commander("federation", &["sync", "status", f])
                } else {
                    println!("{}", "📊 Status for all folders...".cyan().bold());
                    route_to_commander("federation", &["sync", "status"])
                }
            }
        },
        FederationCommands::Registry { action } => match action {
            RegistryActions::List => {
                println!("{}", "📋 Listing registered services...".blue().bold());
                route_to_commander("federation", &["registry", "list"])
            }
            RegistryActions::Health => {
                println!("{}", "🏥 Checking service health...".blue().bold());
                route_to_commander("federation", &["registry", "health"])
            }
            RegistryActions::Add { name, service_type } => {
                println!(
                    "{}",
                    format!("➕ Adding {} ({})...", name, service_type)
                        .green()
                        .bold()
                );
                route_to_commander(
                    "federation",
                    &["registry", "add", &name, "--type", &service_type],
                )
            }
            RegistryActions::Remove { name } => {
                println!("{}", format!("➖ Removing {}...", name).red().bold());
                route_to_commander("federation", &["registry", "remove", &name])
            }
        },
        FederationCommands::Health => {
            println!("{}", "🩺 Running federation health checks...".cyan().bold());
            // For now, this is a placeholder.
            // In the future, this will call the federation_health.py script or similar.
            println!("  - Loki status: {}", "Pending".yellow());
            println!("  - NATS status: {}", "Pending".yellow());
            println!("  - Tmux status: {}", "Pending".yellow());
            Ok(())
        }
        FederationCommands::Status => {
            println!("{}", "📊 Federation status...".cyan().bold());
            println!("  - Node discovery: {}", "Pending".yellow());
            Ok(())
        }
        FederationCommands::Agents => {
            println!("{}", "🤖 Listing all active agents...".cyan().bold());
            println!("  - Agent query: {}", "Pending".yellow());
            Ok(())
        }
        FederationCommands::Validate => {
            println!("{}", "🔍 Validating federation configuration...".cyan().bold());
            let status = std::process::Command::new("federation-validate").status()?;
            if !status.success() {
                anyhow::bail!("Federation validation failed");
            }
            Ok(())
        }
        FederationCommands::Dashboard { port } => {
            println!(
                "{}",
                format!("📊 Opening federation dashboard on port {}...", port)
                    .cyan()
                    .bold()
            );
            let status = std::process::Command::new("federation-dashboard")
                .arg("--port")
                .arg(port.to_string())
                .status()?;
            if !status.success() {
                anyhow::bail!("Failed to open federation dashboard");
            }
            Ok(())
        }
    }
}

fn handle_docs(command: DocsCommands) -> Result<()> {
    match command {
        DocsCommands::Manifest { action } => match action {
            ManifestActions::List => {
                println!("{}", "📄 Listing all manifests...".cyan().bold());
                route_to_commander("docs", &["manifest", "list"])
            }
            ManifestActions::Validate { repo_path } => {
                println!(
                    "{}",
                    format!("🔍 Validating manifest for {}...", repo_path)
                        .cyan()
                        .bold()
                );
                route_to_commander("docs", &["manifest", "validate", &repo_path])
            }
            ManifestActions::Generate { repo_path } => {
                println!(
                    "{}",
                    format!("✨ Generating manifest for {}...", repo_path)
                        .cyan()
                        .bold()
                );
                route_to_commander("docs", &["manifest", "generate", &repo_path])
            }
        },
    }
}

fn handle_repo(command: RepoCommands) -> Result<()> {
    match command {
        RepoCommands::Check {
            path,
            format,
            strict,
        } => {
            let repo_path = path.unwrap_or_else(|| ".".to_string());
            repo::check(&repo_path, &format, strict)
        }
        RepoCommands::Analyze {
            repo_path,
            lang,
            force,
            format,
        } => repo::analyze(&repo_path, lang.as_deref(), force, &format),
        RepoCommands::Graph { action } => handle_graph(action),
        RepoCommands::Codegraph { command } => handle_codegraph(command),
    }
}

fn handle_analyze(command: AnalyzeCommands) -> Result<()> {
    match command {
        AnalyzeCommands::Repo {
            repo_path,
            lang,
            force,
            format,
        } => repo::analyze(&repo_path, lang.as_deref(), force, &format),
    }
}

fn handle_codegraph(command: CodegraphCommands) -> Result<()> {
    match command {
        CodegraphCommands::DeployHooks { verbose, config } => {
            // Resolve config path (default to ~/.config/nabi/codegraph.toml)
            let config_path = if let Some(custom_path) = config {
                PathBuf::from(custom_path)
            } else {
                let home = std::env::var("HOME").context("HOME environment variable not set")?;
                PathBuf::from(home)
                    .join(".config")
                    .join("nabi")
                    .join("codegraph.toml")
            };

            if !config_path.exists() {
                eprintln!("Error: Configuration file not found: {}", config_path.display());
                return Err(anyhow::anyhow!(
                    "Config file not found at {}",
                    config_path.display()
                ));
            }

            // Load configuration and deploy hooks
            let deployment = repo::HookDeployment::from_toml(&config_path)?;
            let stats = deployment.deploy(verbose)?;

            if verbose {
                println!(
                    "\n✓ Hook deployment complete: {} deployed, {} skipped",
                    stats.deployed, stats.skipped
                );
            } else {
                println!(
                    "✓ {} hooks deployed to {}",
                    stats.deployed,
                    deployment.output_dir.display()
                );
            }

            Ok(())
        }
        CodegraphCommands::ValidateHooks { format: _format } => {
            println!("⚠ Hook validation not yet implemented");
            Ok(())
        }
        CodegraphCommands::ShowHooks { templates: _templates } => {
            println!("⚠ Hook configuration display not yet implemented");
            Ok(())
        }
    }
}

fn handle_validate(command: ValidateCommands) -> Result<()> {
    match command {
        ValidateCommands::Toml {
            path,
            verbose,
            schema,
            no_uv,
        } => {
            // Locate the validate_toml.sh script
            let home = dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
            let script_path = home.join("nabia/tools/validators/toml/validate_toml.sh");

            if !script_path.exists() {
                anyhow::bail!(
                    "TOML validator script not found at: {}",
                    script_path.display()
                );
            }

            // Build command arguments
            let mut args = vec![path];
            if verbose {
                args.push("--verbose".to_string());
            }
            if let Some(schema_path) = schema {
                args.push("--schema".to_string());
                args.push(schema_path);
            }
            if no_uv {
                args.push("--no-uv".to_string());
            }

            // Execute the script
            let status = std::process::Command::new("bash")
                .arg(&script_path)
                .args(&args)
                .status()
                .with_context(|| format!("Failed to execute TOML validator: {}", script_path.display()))?;

            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }

            Ok(())
        }
    }
}

fn parse_promotion_mode(s: &str) -> Result<String, String> {
    let upper = s.to_uppercase();
    match upper.as_str() {
        "LIVE" | "STABLE" | "INSTALL" => Ok(upper),
        _ => Err(format!("Invalid promotion mode: '{}'. Valid modes: LIVE, STABLE, INSTALL", s))
    }
}

fn parse_promotion_mode_internal(s: &str) -> Result<commands::promote::PromotionMode> {
    match s.to_uppercase().as_str() {
        "LIVE" => Ok(commands::promote::PromotionMode::Live),
        "STABLE" => Ok(commands::promote::PromotionMode::Stable),
        "INSTALL" => Ok(commands::promote::PromotionMode::Install),
        _ => Err(anyhow::anyhow!("Invalid promotion mode: {}", s))
    }
}

fn handle_tool(command: ToolCommands) -> Result<()> {
    match command {
        ToolCommands::Register(args) => register_tool(args),
        ToolCommands::List {
            format,
            status,
            runtime,
        } => list_tools(&format, status.as_deref(), runtime.as_deref()),
        ToolCommands::Exec { tool, args } => handle_tool_exec(&tool, args),
        ToolCommands::Promote { tool_id, version, mode } => {
            let mode_parsed = mode.as_ref()
                .map(|m| parse_promotion_mode_internal(m))
                .transpose()?;
            commands::promote::promote_tool(&tool_id, version.as_deref(), mode_parsed)?;
            Ok(())
        }
    }
}

fn handle_tool_exec(tool_id: &str, args: Vec<String>) -> Result<()> {
    // EXECUTION ABSTRACTION: Check if tool is promoted
    // If promoted, execute from deployed location instead of source
    if commands::promote::is_tool_promoted(tool_id) {
        use colored::*;

        println!("{} Using promoted artifact", "→".green());

        let status = commands::promote::execute_promoted_tool(tool_id, &args)
            .with_context(|| format!("Failed to execute promoted tool '{}'", tool_id))?;

        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }

        return Ok(());
    }

    // FALLBACK: Execute from source location (legacy path)
    println!("{} Tool not promoted, executing from source", "→".yellow());

    // 1. Load tool manifest from ~/.config/nabi/tools/{tool_id}.toml
    let manifest_path = NabiPaths::config_dir()?.join("tools").join(format!("{}.toml", tool_id));

    if !manifest_path.exists() {
        anyhow::bail!("Tool '{}' not registered. Run: nabi tool list", tool_id);
    }

    let content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

    let parsed: toml::Value = toml::from_str(&content)
        .with_context(|| format!("Failed to parse manifest for '{}'", tool_id))?;

    // 2. Extract execution command from runtime.execution
    let execution = parsed
        .get("runtime")
        .and_then(|r| r.get("execution"))
        .and_then(|e| e.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing runtime.execution in manifest for '{}'", tool_id))?;

    // 3. Check if tool uses a venv
    let venv_location = parsed
        .get("venv")
        .and_then(|v| v.get("location"))
        .and_then(|l| l.as_str());

    // 4. Construct and execute the command
    let mut cmd_parts: Vec<String> = execution.split_whitespace().map(String::from).collect();
    cmd_parts.extend(args);

    // If venv exists, prepend activation to PATH
    let status = if let Some(venv_path) = venv_location {
        let venv_expanded = expand_home(venv_path)?;
        let venv_bin = venv_expanded.join("bin");

        if !venv_bin.exists() {
            eprintln!("{}", format!("⚠️  Warning: venv not found at {}", venv_bin.display()).yellow());
            eprintln!("{}", format!("    Run: nabi tool setup {}", tool_id).yellow());
        }

        // Prepend venv/bin to PATH
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", venv_bin.display(), current_path);

        std::process::Command::new(&cmd_parts[0])
            .args(&cmd_parts[1..])
            .env("PATH", new_path)
            .env("VIRTUAL_ENV", venv_expanded.display().to_string())
            .status()
            .with_context(|| format!("Failed to execute '{}'", cmd_parts.join(" ")))?
    } else {
        std::process::Command::new(&cmd_parts[0])
            .args(&cmd_parts[1..])
            .status()
            .with_context(|| format!("Failed to execute '{}'", cmd_parts.join(" ")))?
    };

    // 5. Handle exit codes
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

fn handle_register(command: RegisterCommands) -> Result<()> {
    match command {
        RegisterCommands::Tool(args) => register_tool(args),
    }
}

/// Detect if a TOML manifest has manual customization beyond auto-generated defaults
fn detect_manual_customization(parsed: &toml::Value) -> bool {
    // Check for signs of manual customization

    // 1. Custom description (not auto-generated pattern)
    if let Some(desc) = parsed.get("tool")
        .and_then(|t| t.get("description"))
        .and_then(|d| d.as_str())
    {
        if !desc.starts_with("Auto-registered tool manifest for") {
            return true;
        }
    }

    // 2. Has repository URL defined
    if parsed.get("source")
        .and_then(|s| s.get("repository"))
        .and_then(|r| r.as_str())
        .is_some()
    {
        return true;
    }

    // 3. Has aliases defined in commands
    if let Some(aliases) = parsed.get("commands")
        .and_then(|c| c.get("aliases"))
        .and_then(|a| a.as_array())
    {
        if !aliases.is_empty() {
            return true;
        }
    }

    // 4. Has custom tags (beyond ["tool", "<runtime>"])
    if let Some(tags) = parsed.get("tags")
        .and_then(|t| t.get("tags"))
        .and_then(|t| t.as_array())
    {
        if tags.len() > 2 {
            return true;
        }
    }

    // 5. Has federation_aware or other custom capabilities set to true
    if let Some(caps) = parsed.get("capabilities").and_then(|c| c.as_table()) {
        if caps.get("federation_aware").and_then(|v| v.as_bool()) == Some(true) {
            return true;
        }
        if caps.get("xdg_compliant").and_then(|v| v.as_bool()) == Some(true) {
            return true;
        }
        if caps.get("cross_platform").and_then(|v| v.as_bool()) == Some(true) {
            return true;
        }
    }

    // 6. Has custom execution path (not pointing to .config/nabi/tools/)
    if let Some(exec) = parsed.get("runtime")
        .and_then(|r| r.get("execution"))
        .and_then(|e| e.as_str())
    {
        if !exec.contains(".config/nabi/tools/") {
            return true;
        }
    }

    // 7. Language is explicitly set (not "other")
    if let Some(lang) = parsed.get("runtime")
        .and_then(|r| r.get("language"))
        .and_then(|l| l.as_str())
    {
        if lang != "other" {
            return true;
        }
    }

    false
}

/// Print what customizations would be lost if overwriting
fn print_customization_warning(existing: &toml::Value) {
    eprintln!("\n{}", "⚠️  WARNING: This manifest has manual customization!".yellow().bold());
    eprintln!("{}", "   Overwriting will LOSE the following:".yellow());

    // Show description
    if let Some(desc) = existing.get("tool")
        .and_then(|t| t.get("description"))
        .and_then(|d| d.as_str())
    {
        if !desc.starts_with("Auto-registered") {
            eprintln!("   • Description: \"{}\"", desc.dimmed());
        }
    }

    // Show repository
    if let Some(repo) = existing.get("source")
        .and_then(|s| s.get("repository"))
        .and_then(|r| r.as_str())
    {
        eprintln!("   • Repository: {}", repo.dimmed());
    }

    // Show aliases
    if let Some(aliases) = existing.get("commands")
        .and_then(|c| c.get("aliases"))
        .and_then(|a| a.as_array())
    {
        if !aliases.is_empty() {
            let aliases_str: Vec<String> = aliases.iter()
                .filter_map(|a| a.as_str())
                .map(|s| s.to_string())
                .collect();
            eprintln!("   • Aliases: [{}]", aliases_str.join(", ").dimmed());
        }
    }

    // Show custom tags
    if let Some(tags) = existing.get("tags")
        .and_then(|t| t.get("tags"))
        .and_then(|t| t.as_array())
    {
        let tags_str: Vec<String> = tags.iter()
            .filter_map(|t| t.as_str())
            .map(|s| s.to_string())
            .collect();
        eprintln!("   • Tags: [{}]", tags_str.join(", ").dimmed());
    }

    // Show capabilities
    if let Some(caps) = existing.get("capabilities").and_then(|c| c.as_table()) {
        let mut cap_list = Vec::new();
        if caps.get("federation_aware").and_then(|v| v.as_bool()) == Some(true) {
            cap_list.push("federation_aware");
        }
        if caps.get("xdg_compliant").and_then(|v| v.as_bool()) == Some(true) {
            cap_list.push("xdg_compliant");
        }
        if caps.get("cross_platform").and_then(|v| v.as_bool()) == Some(true) {
            cap_list.push("cross_platform");
        }
        if !cap_list.is_empty() {
            eprintln!("   • Capabilities: {}", cap_list.join(", ").dimmed());
        }
    }

    // Show execution path
    if let Some(exec) = existing.get("runtime")
        .and_then(|r| r.get("execution"))
        .and_then(|e| e.as_str())
    {
        eprintln!("   • Execution: {}", exec.dimmed());
    }

    eprintln!();
    eprintln!("{}", "   To proceed anyway: use --force-overwrite".yellow());
    eprintln!("{}", "   To preserve settings: edit the TOML manually".yellow());
    eprintln!();
}

fn register_tool(args: ToolRegisterArgs) -> Result<()> {
    println!("{}", "🛠  Registering tool manifest...".cyan().bold());

    let expanded_path = expand_home(&args.path)?;
    let canonical_path = expanded_path
        .canonicalize()
        .with_context(|| format!("Tool path not found: {}", args.path))?;

    let metadata = fs::metadata(&canonical_path)
        .with_context(|| format!("Unable to read metadata for {}", canonical_path.display()))?;

    let tool_name = args
        .name
        .clone()
        .unwrap_or_else(|| derive_tool_name(&canonical_path));

    let slug = slugify(&tool_name);
    let tool_id = if slug.is_empty() {
        "tool".to_string()
    } else {
        slug
    };

    let command_name = args.command.clone().unwrap_or_else(|| tool_id.clone());

    let runtime = args
        .runtime
        .or_else(|| infer_runtime(&canonical_path))
        .unwrap_or(RuntimeKind::Other);

    if args.runtime.is_none() && matches!(runtime, RuntimeKind::Other) {
        println!(
            "{}",
            "⚠️  Could not infer runtime automatically; recorded as 'other'.".yellow()
        );
    }

    let runtime_version_hint = args
        .runtime_version
        .clone()
        .unwrap_or_else(|| runtime.default_version_hint().to_string());

    let tool_version = args.version.clone().unwrap_or_else(|| "0.1.0".to_string());

    let description = args
        .description
        .clone()
        .unwrap_or_else(|| format!("Auto-registered tool manifest for {}", tool_name));

    let entry_point = args
        .entry_point
        .clone()
        .unwrap_or_else(|| command_name.clone());

    let execution = args
        .execution
        .clone()
        .unwrap_or_else(|| runtime.default_execution(&canonical_path));

    let source_path = if metadata.is_file() {
        canonical_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| canonical_path.clone())
    } else {
        canonical_path.clone()
    };

    let manifest_source_path = path_to_tilde(&source_path)?;

    let tags = if args.tags.is_empty() {
        vec!["tool".to_string(), runtime.as_language().to_string()]
    } else {
        args.tags
            .iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect()
    };

    let venv_location = if let Some(explicit) = &args.venv {
        Some(explicit.clone())
    } else if needs_default_venv(runtime) {
        let venv_path = NabiPaths::venv_dir()?.join(tool_id.replace('-', "_"));
        Some(path_to_tilde(&venv_path)?)
    } else {
        None
    };

    // Validate and setup dependencies
    let (validated_deps, dep_messages) =
        validate_tool_dependencies(&source_path, &venv_location, runtime)?;

    // Print dependency validation messages
    for msg in &dep_messages {
        println!("  {}", msg);
    }

    let installer_hint = args
        .installer
        .clone()
        .or_else(|| default_installer(runtime).map(|value| value.to_string()));

    let checksum = if metadata.is_file() {
        Some(compute_sha256_hex(&canonical_path)?)
    } else {
        None
    };

    let schema_version = args
        .schema_version
        .clone()
        .unwrap_or_else(|| "1.0.0".to_string());

    let status = args.status.clone().unwrap_or_else(|| "active".to_string());

    let manifest = ToolManifest {
        tool: ToolSection {
            id: tool_id.clone(),
            name: tool_name.clone(),
            version: tool_version,
            description,
            status,
        },
        source: SourceSection {
            source_type: "local".to_string(),
            path: manifest_source_path,
            repository: args.repository.clone(),
            branch: None,
        },
        runtime: RuntimeSection {
            language: runtime.as_language().to_string(),
            version: runtime_version_hint,
            entry_point,
            execution,
            wrapper: "nabi".to_string(),
        },
        venv: venv_location.map(|location| VenvSection {
            location,
            setup_script: None,
            installer: installer_hint.clone(),
            dependencies: validated_deps.clone(),
        }),
        capabilities: CapabilitiesSection {
            federation_aware: args.federation_aware,
            aura_compatible: args.aura_compatible,
            xdg_compliant: args.xdg_compliant,
            hook_integrated: args.hook_integrated,
            cross_platform: args.cross_platform,
        },
        commands: CommandsSection {
            commands: vec![command_name.clone()],
        },
        integration: IntegrationSection {
            hooks: Vec::new(),
            federation_events: Vec::new(),
        },
        tags: TagsSection { tags },
        transformation: TransformationSection {
            target_directory: TOOL_TRANSFORMATION_TARGET.to_string(),
            generated_by: "nabi tool register".to_string(),
            schema_version,
        },
    };

    let rendered =
        toml::to_string_pretty(&manifest).context("Failed to serialize tool manifest to TOML")?;

    let created_on = Utc::now().format("%Y-%m-%d");

    let mut output = String::new();
    output.push_str(&format!("# Tool Registry Entry: {}\n", tool_name));
    output.push_str(&format!("# Created: {}\n", created_on));
    output.push_str(&format!("# Schema: {}\n", TOOL_SCHEMA_PATH));
    if let Some(ref checksum) = checksum {
        output.push_str(&format!("# Checksum (sha256): {}\n", checksum));
    }
    output.push('\n');
    output.push_str(&rendered);

    let tools_dir = NabiPaths::config_dir()?.join("tools");
    fs::create_dir_all(&tools_dir).with_context(|| {
        format!(
            "Failed to create tools directory at {}",
            tools_dir.display()
        )
    })?;

    let manifest_path = tools_dir.join(format!("{}.toml", tool_id));

    // Safety check: prevent overwriting manually-customized manifests
    if manifest_path.exists() {
        if !args.force && !args.force_overwrite {
            anyhow::bail!(
                "Manifest already exists at {}\n   Use --force to see what would change",
                manifest_path.display()
            );
        }

        // Read and parse existing manifest
        let existing_content = fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read existing manifest at {}", manifest_path.display()))?;

        let existing_parsed: toml::Value = toml::from_str(&existing_content)
            .with_context(|| format!("Failed to parse existing manifest at {}", manifest_path.display()))?;

        // Detect manual customization
        let has_customization = detect_manual_customization(&existing_parsed);

        if has_customization {
            // Show what would be lost
            print_customization_warning(&existing_parsed);

            if !args.force_overwrite {
                anyhow::bail!(
                    "Refusing to overwrite customized manifest (safety check)\n   \
                     Use --force-overwrite to proceed anyway (NOT RECOMMENDED)\n   \
                     Or edit {} manually",
                    manifest_path.display()
                );
            }

            // force_overwrite is set - allow but warn
            eprintln!("{}", "⚠️  Proceeding with --force-overwrite (customizations will be LOST)".red().bold());
            eprintln!();
        } else {
            // No customization detected - safe to overwrite with --force
            println!("{}", "ℹ️  Overwriting auto-generated manifest".cyan());
        }
    }

    fs::write(&manifest_path, output)
        .with_context(|| format!("Failed to write manifest to {}", manifest_path.display()))?;

    let manifest_display = path_to_tilde(&manifest_path)?;
    println!(
        "{}",
        format!("✅ Registered tool '{}'", tool_name).green().bold()
    );
    println!("  Manifest: {}", manifest_display.dimmed());
    println!("  Command: nabi {}", command_name.dimmed());
    if let Some(checksum) = checksum {
        println!("  SHA256: {}", checksum.dimmed());
    }
    println!(
        "  {}",
        "Run `nabi docs manifest validate` to pull the new manifest into the integrity graph."
            .dimmed()
    );

    Ok(())
}

/// Validate and setup dependencies for Python tools
/// Returns (dependencies_list, validation_messages)
fn validate_tool_dependencies(
    source_path: &Path,
    venv_location: &Option<String>,
    runtime: RuntimeKind,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut dependencies = Vec::new();
    let mut messages = Vec::new();

    // Only validate for Python tools
    if !matches!(runtime, RuntimeKind::Python) {
        return Ok((dependencies, messages));
    }

    // Look for TOML config with dependencies
    let mut possible_locations = vec![Some(source_path.join("requirements.txt"))];

    // Add TOML path if source file name is available
    if let Some(file_name) = source_path.file_name() {
        if let Some(parent) = source_path.parent().and_then(|p| p.parent()) {
            possible_locations.push(Some(
                parent.join("tools").join(file_name).with_extension("toml"),
            ));
        }
    }

    let mut found_deps = false;

    // Check for dependencies
    for toml_path in possible_locations.iter().flatten() {
        if !toml_path.exists() {
            continue;
        }

        if toml_path.file_name().and_then(|n| n.to_str()) == Some("requirements.txt") {
            // Parse requirements.txt
            if let Ok(content) = fs::read_to_string(toml_path) {
                dependencies = content
                    .lines()
                    .filter(|line| !line.trim().starts_with('#') && !line.trim().is_empty())
                    .map(|line| line.trim().to_string())
                    .collect();
                found_deps = !dependencies.is_empty();
                messages.push(format!(
                    "📦 Found {} dependencies in {}",
                    dependencies.len(),
                    toml_path.display()
                ));
                break;
            }
        } else if toml_path.extension().and_then(|e| e.to_str()) == Some("toml") {
            // Parse TOML for [tool.dependencies.python]
            if let Ok(content) = fs::read_to_string(toml_path) {
                if let Ok(toml_value) = toml::from_str::<toml::Value>(&content) {
                    if let Some(tool_deps) = toml_value
                        .get("tool")
                        .and_then(|t| t.get("dependencies"))
                        .and_then(|d| d.get("python"))
                        .and_then(|p| p.as_array())
                    {
                        dependencies = tool_deps
                            .iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect();
                        found_deps = !dependencies.is_empty();
                        messages.push(format!(
                            "📦 Found {} dependencies in {}",
                            dependencies.len(),
                            toml_path.display()
                        ));
                        break;
                    }
                }
            }
        }
    }

    if !found_deps {
        messages.push("⚠️  No dependencies found - tool may require manual setup".to_string());
        return Ok((dependencies, messages));
    }

    // Validate venv exists if dependencies found
    if let Some(venv_loc) = venv_location {
        let venv_path = expand_home(venv_loc)?;
        let python_bin = venv_path.join("bin").join("python");

        if !venv_path.exists() {
            messages.push(format!("🔧 Creating venv at {}...", venv_loc));

            // Create venv using uv
            let venv_str = venv_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Venv path contains invalid UTF-8: {:?}", venv_path))?;
            let status = process::Command::new("uv")
                .args(&["venv", venv_str])
                .status()
                .context("Failed to create venv with uv")?;

            if !status.success() {
                anyhow::bail!("Failed to create venv at {}", venv_loc);
            }
            messages.push("✅ Venv created".to_string());
        } else {
            messages.push(format!("✓ Venv exists at {}", venv_loc));
        }

        // Install dependencies
        if !dependencies.is_empty() {
            messages.push(format!(
                "📥 Installing {} dependencies...",
                dependencies.len()
            ));

            // Create temporary requirements file
            let temp_req = std::env::temp_dir().join("nabi_temp_requirements.txt");
            fs::write(&temp_req, dependencies.join("\n"))?;

            let temp_req_str = temp_req
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Requirements file path contains invalid UTF-8: {:?}", temp_req))?;
            let python_bin_str = python_bin
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Python binary path contains invalid UTF-8: {:?}", python_bin))?;

            let status = process::Command::new("uv")
                .args(&[
                    "pip",
                    "install",
                    "-r",
                    temp_req_str,
                    "--python",
                    python_bin_str,
                ])
                .status()
                .context("Failed to install dependencies with uv")?;

            fs::remove_file(temp_req)?;

            if !status.success() {
                messages.push("⚠️  Some dependencies failed to install".to_string());
            } else {
                messages.push(format!("✅ Installed {} packages", dependencies.len()));
            }
        }
    } else {
        messages.push("⚠️  No venv configured - dependencies not installed".to_string());
    }

    Ok((dependencies, messages))
}

const TOOL_SCHEMA_PATH: &str = "~/.config/nabi/governance/schemas/tool.schema.json";
const TOOL_TRANSFORMATION_TARGET: &str = "~/.local/state/nabi/tools";

#[derive(Serialize)]
struct ToolManifest {
    tool: ToolSection,
    source: SourceSection,
    runtime: RuntimeSection,
    #[serde(skip_serializing_if = "Option::is_none")]
    venv: Option<VenvSection>,
    capabilities: CapabilitiesSection,
    commands: CommandsSection,
    integration: IntegrationSection,
    tags: TagsSection,
    transformation: TransformationSection,
}

#[derive(Serialize)]
struct ToolSection {
    id: String,
    name: String,
    version: String,
    description: String,
    status: String,
}

#[derive(Serialize)]
struct SourceSection {
    #[serde(rename = "type")]
    source_type: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
}

#[derive(Serialize)]
struct RuntimeSection {
    language: String,
    version: String,
    entry_point: String,
    execution: String,
    wrapper: String,
}

#[derive(Serialize)]
struct VenvSection {
    location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    setup_script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    installer: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<String>,
}

#[derive(Serialize)]
struct CapabilitiesSection {
    federation_aware: bool,
    aura_compatible: bool,
    xdg_compliant: bool,
    hook_integrated: bool,
    cross_platform: bool,
}

#[derive(Serialize)]
struct CommandsSection {
    commands: Vec<String>,
}

#[derive(Serialize)]
struct IntegrationSection {
    hooks: Vec<String>,
    federation_events: Vec<String>,
}

#[derive(Serialize)]
struct TagsSection {
    tags: Vec<String>,
}

#[derive(Serialize)]
struct TransformationSection {
    target_directory: String,
    generated_by: String,
    schema_version: String,
}

fn derive_tool_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|os| os.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| "tool".to_string())
}

fn slugify(name: &str) -> String {
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

fn shell_quote(path: &Path) -> String {
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

fn infer_runtime(path: &Path) -> Option<RuntimeKind> {
    if path.is_dir() {
        let pyproject = path.join("pyproject.toml");
        if pyproject.exists() {
            return Some(RuntimeKind::Python);
        }

        let package_json = path.join("package.json");
        if package_json.exists() {
            return Some(RuntimeKind::Node);
        }

        let cargo = path.join("Cargo.toml");
        if cargo.exists() {
            return Some(RuntimeKind::Rust);
        }

        let go_mod = path.join("go.mod");
        if go_mod.exists() {
            return Some(RuntimeKind::Go);
        }

        return None;
    }

    if !path.is_file() {
        return None;
    }

    if let Some(shebang) = read_shebang(path) {
        let shebang_lower = shebang.to_lowercase();
        if shebang_lower.contains("python") {
            return Some(RuntimeKind::Python);
        }
        if shebang_lower.contains("bash") || shebang_lower.contains("sh") {
            return Some(RuntimeKind::Bash);
        }
        if shebang_lower.contains("node") {
            return Some(RuntimeKind::Node);
        }
        if shebang_lower.contains("deno") {
            return Some(RuntimeKind::Deno);
        }
        if shebang_lower.contains("ruby") {
            return Some(RuntimeKind::Ruby);
        }
        if shebang_lower.contains("ts-node") || shebang_lower.contains("tsx") {
            return Some(RuntimeKind::Typescript);
        }
    }

    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .and_then(|ext| match ext.as_str() {
            "py" => Some(RuntimeKind::Python),
            "rs" => Some(RuntimeKind::Rust),
            "sh" | "bash" | "zsh" => Some(RuntimeKind::Bash),
            "ts" | "tsx" => Some(RuntimeKind::Typescript),
            "js" | "mjs" | "cjs" => Some(RuntimeKind::Node),
            "rb" => Some(RuntimeKind::Ruby),
            "go" => Some(RuntimeKind::Go),
            _ => None,
        })
}

fn read_shebang(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut buffer = [0u8; 256];
    let read = file.read(&mut buffer).ok()?;
    let snippet = std::str::from_utf8(&buffer[..read]).ok()?;
    snippet.lines().next().map(|line| line.to_string())
}

fn needs_default_venv(runtime: RuntimeKind) -> bool {
    matches!(runtime, RuntimeKind::Python)
}

fn default_installer(runtime: RuntimeKind) -> Option<&'static str> {
    match runtime {
        RuntimeKind::Python => Some("uv"),
        _ => None,
    }
}

fn compute_sha256_hex(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("Failed to open {} for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("Failed to read {} while hashing", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn path_to_tilde(path: &Path) -> Result<String> {
    let home = NabiPaths::home_dir()?;
    if path.starts_with(&home) {
        let remainder = path.strip_prefix(&home).unwrap_or(path);
        if remainder.as_os_str().is_empty() {
            Ok("~".to_string())
        } else {
            Ok(format!("~/{}", remainder.display()))
        }
    } else {
        Ok(path.to_string_lossy().to_string())
    }
}

fn handle_graph(action: GraphActions) -> Result<()> {
    match action {
        GraphActions::Search {
            symbol,
            repo,
            format,
        } => {
            let repo_path = repo.unwrap_or_else(|| ".".to_string());
            repo::graph_search(&repo_path, &symbol, &format)
        }
        GraphActions::References {
            symbol,
            repo,
            format,
        } => {
            let repo_path = repo.unwrap_or_else(|| ".".to_string());
            repo::graph_references(&repo_path, &symbol, &format)
        }
        GraphActions::Related {
            symbol,
            repo,
            depth,
            format,
        } => {
            let repo_path = repo.unwrap_or_else(|| ".".to_string());
            repo::graph_related(&repo_path, &symbol, depth, &format)
        }
    }
}

fn handle_self(command: SelfCommands) -> Result<()> {
    match command {
        SelfCommands::Doctor => {
            println!("{}", "🏥 Running health check...".blue().bold());
            check_commander("claude")?;
            check_commander("data")?;
            check_commander("federation")?;
            check_xdg_compliance()?;
            println!("{}", "✓ All commanders healthy!".green().bold());
            Ok(())
        }
        SelfCommands::Update => {
            println!("{}", "⬆️  Updating all commanders...".blue().bold());
            update_commander("claude")?;
            update_commander("data")?;
            update_commander("federation")?;
            println!("{}", "✓ All commanders updated!".green().bold());
            Ok(())
        }
        SelfCommands::Config => {
            println!("{}", "⚙️  Configuration".blue().bold());
            let config_dir = NabiPaths::config_dir()?;
            let data_dir = NabiPaths::data_dir()?;
            let cache_dir = NabiPaths::cache_dir()?;
            println!("  Config dir: {}", config_dir.display());
            println!("  Data dir:   {}", data_dir.display());
            println!("  Cache dir:  {}", cache_dir.display());
            Ok(())
        }
        SelfCommands::Spec { format } => handle_spec(format),
        SelfCommands::Diagnose { quick, aura, format } => {
            handlers::self_manage::handle_self(crate::cli::SelfCommands::Diagnose {
                quick,
                aura,
                format,
            })
        }
    }
}

#[derive(Clone, ValueEnum)]
enum SpecFormat {
    Markdown,
    Json,
}

#[derive(Serialize)]
struct SpecEntry {
    path: Vec<String>,
    about: Option<String>,
    arguments: Vec<String>,
}

fn handle_spec(format: SpecFormat) -> Result<()> {
    let root = Cli::command();
    let mut entries = collect_spec_entries(&root);
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    match format {
        SpecFormat::Markdown => print_spec_markdown(&entries),
        SpecFormat::Json => {
            serde_json::to_writer_pretty(io::stdout(), &entries)?;
            println!();
        }
    }

    Ok(())
}

fn collect_spec_entries(command: &Command) -> Vec<SpecEntry> {
    let mut entries = Vec::new();
    for sub in command.get_subcommands() {
        let mut path = vec!["nabi".to_string(), sub.get_name().to_string()];
        collect_spec_recursive(sub, &mut path, &mut entries);
    }
    entries
}

fn collect_spec_recursive(command: &Command, path: &mut Vec<String>, entries: &mut Vec<SpecEntry>) {
    let arguments = command
        .get_arguments()
        .filter(|arg| !arg.is_hide_set())
        .map(summarize_argument)
        .collect();

    entries.push(SpecEntry {
        path: path.clone(),
        about: command.get_about().map(|s| s.to_string()),
        arguments,
    });

    for sub in command.get_subcommands() {
        path.push(sub.get_name().to_string());
        collect_spec_recursive(sub, path, entries);
        path.pop();
    }
}

fn summarize_argument(arg: &Arg) -> String {
    let mut parts = Vec::new();

    if let Some(long) = arg.get_long() {
        parts.push(format!("--{}", long));
    }

    if let Some(short) = arg.get_short() {
        parts.push(format!("-{}", short));
    }

    if parts.is_empty() {
        let id = arg.get_id().as_str().to_string();
        // Check if this is a positional argument based on id format
        if !id.starts_with("--") {
            parts.push(format!("<{}>", id));
        } else {
            parts.push(format!("--{}", id));
        }
    }

    parts.join(", ")
}

fn print_spec_markdown(entries: &[SpecEntry]) {
    println!("# nabi CLI Command Specification\n");
    println!("Generated by `nabi self spec --format markdown`.\n");

    for entry in entries {
        let command = entry.path.join(" ");
        println!("### `{}`", command);

        if let Some(about) = &entry.about {
            println!("{}.", about.trim());
        }

        if !entry.arguments.is_empty() {
            println!("\n**Arguments:**");
            for arg in &entry.arguments {
                println!("- `{}`", arg);
            }
        }

        println!();
    }
}

// Dynamic completion script for ZSH
const DYNAMIC_ZSH_COMPLETION: &[u8] = b"
# Dynamic tool completion function
_nabi_dynamic_tools() {
    local tools
    tools=(${(f)\"$(nabi tool list --format=json 2>/dev/null | jq -r '.tools[] | .id + \":\" + .description' 2>/dev/null)\"})
    _describe 'registered tools' tools
}

# Override the tool exec completion
_nabi__tool__exec_commands() {
    _nabi_dynamic_tools
}

# Override the top-level exec completion
_nabi__exec_commands() {
    _nabi_dynamic_tools
}
";

// Dynamic completion script for BASH
const DYNAMIC_BASH_COMPLETION: &[u8] = b"
# Dynamic tool completion function
_nabi_dynamic_tools() {
    local tools
    tools=$(nabi tool list --format=json 2>/dev/null | jq -r '.tools[].id' 2>/dev/null)
    COMPREPLY=($(compgen -W \"${tools}\" -- \"${COMP_WORDS[COMP_CWORD]}\"))
}

# Hook into nabi tool exec completion
_nabi_tool_exec() {
    case \"${COMP_CWORD}\" in
        3)  # After \"nabi tool exec\"
            _nabi_dynamic_tools
            ;;
    esac
}

# Hook into nabi exec completion
_nabi_exec() {
    case \"${COMP_CWORD}\" in
        2)  # After \"nabi exec\"
            _nabi_dynamic_tools
            ;;
    esac
}

complete -F _nabi_tool_exec nabi
complete -F _nabi_exec nabi
";

fn handle_completions(shell: CompletionShell, output: Option<PathBuf>, install: bool) -> Result<()> {
    let mut command = Cli::command();

    // Generate base clap completions to a buffer
    let mut buffer = Vec::new();
    generate(shell, &mut command, "nabi", &mut buffer);

    // Append shell-specific dynamic completion functions
    match shell {
        CompletionShell::Zsh => {
            buffer.extend_from_slice(b"\n# Dynamic tool discovery for nabi tool exec\n");
            buffer.extend_from_slice(DYNAMIC_ZSH_COMPLETION);
        }
        CompletionShell::Bash => {
            buffer.extend_from_slice(b"\n# Dynamic tool discovery for nabi tool exec\n");
            buffer.extend_from_slice(DYNAMIC_BASH_COMPLETION);
        }
        _ => {} // Other shells get static completions only
    }

    // Handle output options
    if install {
        let install_path = match shell {
            CompletionShell::Zsh => {
                dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
                    .join(".zsh/completions/_nabi")
            }
            CompletionShell::Bash => {
                dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
                    .join(".bash_completion.d/nabi")
            }
            _ => return Err(anyhow::anyhow!("Auto-install not supported for {:?}", shell)),
        };

        // Create parent directory if needed
        if let Some(parent) = install_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&install_path, &buffer)?;
        println!("✓ Installed completions to: {}", install_path.display());

        // Print instructions
        match shell {
            CompletionShell::Zsh => {
                println!("\nTo enable completions, ensure your ~/.zshrc contains:");
                println!("  fpath=(~/.zsh/completions $fpath)");
                println!("  autoload -Uz compinit && compinit");
            }
            CompletionShell::Bash => {
                println!("\nTo enable completions, ensure your ~/.bashrc contains:");
                println!("  [ -f ~/.bash_completion.d/nabi ] && source ~/.bash_completion.d/nabi");
            }
            _ => {}
        }
    } else if let Some(path) = output {
        fs::write(&path, &buffer)?;
        println!("✓ Wrote completions to: {}", path.display());
    } else {
        io::stdout().write_all(&buffer)?;
    }

    Ok(())
}

fn route_to_commander(commander: &str, args: &[&str]) -> Result<()> {
    // Use XDG Base Directory spec across all platforms
    let nabi_config = NabiPaths::config_dir()?;

    let commander_path = nabi_config.join("commanders").join(commander);

    // Check if a native Rust commander binary exists
    let commander_binary = commander_path.join(commander);
    if commander_binary.exists() {
        println!(
            "{}",
            format!("→ Route to {} commander (native)", commander).dimmed()
        );

        let mut cmd = process::Command::new(&commander_binary);
        cmd.args(args);
        let status = cmd.status().context(format!(
            "Failed to execute commander at {}",
            commander_binary.display()
        ))?;

        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        return Ok(());
    }

    // Fallback: Route to Python CLI for commands not yet migrated to Rust
    // This enables gradual migration: Python → Rust
    println!(
        "{}",
        format!("→ Route to Python CLI: {}", commander).dimmed()
    );

    let bin_dir = NabiPaths::bin_dir()?;
    let python_cli = bin_dir.join("nabi-python");

    if python_cli.exists() {
        let mut cmd = process::Command::new(&python_cli);
        cmd.arg(commander);
        cmd.args(args);
        let status = cmd.status().context(format!(
            "Failed to execute Python CLI at {}",
            python_cli.display()
        ))?;

        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    } else {
        eprintln!(
            "{}",
            format!(
                "❌ Commander '{}' not found and no Python CLI fallback available",
                commander
            )
            .red()
            .bold()
        );
        eprintln!(
            "{}",
            format!("Expected Python CLI: {}", python_cli.display()).yellow()
        );
        eprintln!("{}", "Run 'nabi self doctor' to diagnose issues.".yellow());
        process::exit(1);
    }
}

fn check_commander(commander: &str) -> Result<()> {
    let nabi_config = NabiPaths::config_dir()?;

    let commander_path = nabi_config.join("commanders").join(commander);

    if commander_path.exists() {
        println!("  {} {} {}", "✓".green(), commander, "present".dimmed());
        Ok(())
    } else {
        println!("  {} {} {}", "✗".red(), commander, "missing".dimmed());
        Ok(())
    }
}

fn update_commander(commander: &str) -> Result<()> {
    println!("  {} Updating {}...", "→".blue(), commander);
    Ok(())
}

fn check_xdg_compliance() -> Result<()> {
    let mut violations = Vec::new();

    // Check 1: No .venv artifacts in config directories
    let config_dir = NabiPaths::config_dir()?;
    let config_venv = config_dir.join(".venv");
    let nabi_venv = config_dir.join(".nabi").join(".venv");

    if config_venv.exists() {
        let config_venv_path = NabiPaths::config_dir()?.join(".venv");
        violations.push((
            format!(
                "Broken .venv in config directory: {}",
                config_venv.display()
            ),
            format!("rm -rf {}", config_venv_path.display()),
        ));
    }

    if nabi_venv.exists() {
        let nabi_venv_path = NabiPaths::config_dir()?.join(".nabi").join(".venv");
        violations.push((
            format!("Broken .venv in nested config: {}", nabi_venv.display()),
            format!("rm -rf {}", nabi_venv_path.display()),
        ));
    }

    // Check 2: Verify venv directory exists and is properly structured
    let venv_base = NabiPaths::venv_dir()?;

    if !venv_base.exists() {
        violations.push((
            format!("Venv directory missing: {}", venv_base.display()),
            format!("mkdir -p {}", venv_base.display()),
        ));
    }

    // Check 3: Scan for hardcoded paths in TOML files
    if let Ok(entries) = std::fs::read_dir(&config_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "toml") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    // Look for hardcoded paths starting with /Users/ or /home/
                    for (line_num, line) in content.lines().enumerate() {
                        if line.contains("/Users/")
                            || (line.contains("/home/") && !line.contains("${"))
                        {
                            // Allow comments and specific patterns
                            if !line.trim().starts_with("#") {
                                violations.push((
                                    format!(
                                        "Hardcoded path in {}: line {}",
                                        path.display(),
                                        line_num + 1
                                    ),
                                    "Replace absolute paths with ~ or ${XDG_*} variables"
                                        .to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // Report violations
    if !violations.is_empty() {
        println!("  {} XDG compliance issues found:", "⚠️ ".yellow());
        for (issue, fix) in violations {
            println!("    {} {}", "•".yellow(), issue);
            println!("      {} {}", "→".dimmed(), fix.italic().dimmed());
        }
    } else {
        println!("  {} {} {}", "✓".green(), "XDG", "compliant".dimmed());
    }

    Ok(())
}

fn handle_forge(command: ForgeCommands) -> Result<()> {
    match command {
        ForgeCommands::Enable { feature } => forge::handle_enable(feature),
        ForgeCommands::Disable { feature } => forge::handle_disable(feature),
        ForgeCommands::Status => forge::handle_status(),
        ForgeCommands::List => forge::handle_list(),
    }
}

fn handle_scan(
    path: Option<String>,
    tags: Option<String>,
    confidence: Option<f32>,
    all: bool,
    docs: bool,
    type_filter: Option<Vec<ScanSourceType>>,
    query: Option<String>,
    exact: bool,
) -> Result<()> {
    // If --all flag is set, run tree scan of federation directories
    if all {
        return handle_scan_all();
    }

    // If --docs flag is set, handle documentation scanning
    if docs {
        let doc_filters = type_filter.clone();
        // If query is provided, use ripgrep search
        if let Some(q) = query {
            return handle_scan_docs(&q, doc_filters, path.as_deref(), exact);
        }
        // If path is provided (with or without type filter), scan path for matching files
        if let Some(p) = &path {
            return handle_scan_path_with_types(p, doc_filters);
        }
        // Otherwise, require a query
        anyhow::bail!("--docs requires either a search query (e.g., nabi scan --docs nats) or a path (e.g., nabi scan --docs ~/path)");
    }

    if tags.is_some() || confidence.is_some() {
        anyhow::bail!(
            "Metadata tagging (--tags / --confidence) is not available yet. \
             Please run without these flags."
        );
    }

    println!("{}", "🔍 Scanning filesystem...".cyan().bold());
    let mut args: Vec<String> = Vec::new();
    if let Some(p) = path {
        args.push(p);
    }

    if let Some(types) = type_filter {
        if !types.is_empty() {
            let joined = types
                .iter()
                .map(ScanSourceType::as_extension)
                .collect::<Vec<&'static str>>()
                .join(",");
            args.push("--type".to_string());
            args.push(joined);
        }
    }

    if let Some(q) = query {
        args.push(q);
    }

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    route_to_commander("scan", &arg_refs)
}

fn handle_scan_all() -> Result<()> {
    println!("{}", "🔍 Scanning all federation directories...".cyan().bold());

    // Expand home directory paths
    let home = std::env::var("HOME").context("Could not determine HOME directory")?;

    // Define directories to scan
    let mut dirs = vec![
        format!("{}/nabia", home),
        format!("{}/.config/nabi", home),
        format!("{}/legen", home),
        format!("{}/MemRiff.deprecated", home),
        format!("{}/.claude/agents", home),
        format!("{}/.claude/commands", home),
        format!("{}/.claude/output-styles", home),
        format!("{}/.claude/skills", home),
        format!("{}/.nabi", home),
        format!("{}/docs", home),
    ];

    // Add LaunchAgents *nabi* items (glob expansion)
    let launchagents_dir = format!("{}/Library/LaunchAgents", home);
    if let Ok(entries) = std::fs::read_dir(&launchagents_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str() {
                if filename.contains("nabi") {
                    if let Some(path) = entry.path().to_str() {
                        dirs.push(path.to_string());
                    }
                }
            }
        }
    }

    // Define exclusion patterns for tree command
    let exclusions = vec![
        "target",
        "*.o",
        "*.d",
        "deps",
        "node_modules",
        ".venv",
        "*.pyc",
        "*.log",
        "crates",
        "zed/*",
        "deprecated*",
        "*.deprecated",
        "*archive*/",
        "*fumadocs.old*/",
        "*backup*/",
        "*checkpoint*/",
        "logs",
        "__pycache__",
        "htmlcov",
    ];

    // Build tree command
    let mut cmd = std::process::Command::new("tree");
    cmd.arg("-I")
        .arg(exclusions.join("|"))
        .arg("-L")
        .arg("3")
        .arg("-C")
        .arg("-i")
        .arg("-f");

    // Add all directories to scan
    for dir in &dirs {
        if std::path::Path::new(dir).exists() {
            cmd.arg(dir);
        }
    }

    println!();
    let output = cmd
        .output()
        .context("Failed to execute tree command")?;

    // Print stdout
    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    // Print stderr if present (but don't fail on it)
    if !output.stderr.is_empty() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    }

    // Tree command exit code 2 is OK (means some directories couldn't be read)
    // Only fail on truly critical errors
    if !output.status.success() && output.status.code() != Some(2) {
        eprintln!(
            "{}",
            format!(
                "⚠️  tree command returned non-zero exit code ({})",
                output.status.code().unwrap_or(-1)
            )
            .yellow()
        );
    }

    Ok(())
}

/// Locate the intent enhancer Python script
///
/// Search order:
/// 1. ~/.local/share/nabi/bin/intent_enhancer (installed location)
/// 2. ~/nabia/tools/riff-cli/src/integration/enhance_cli.py (development)
fn locate_intent_enhancer() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("Could not determine HOME directory")?;

    // Try installed location first
    let installed_path = PathBuf::from(format!("{}/.local/share/nabi/bin/intent_enhancer", home));
    if installed_path.exists() {
        return Ok(installed_path);
    }

    // Fallback to development location
    let dev_path = PathBuf::from(format!(
        "{}/nabia/tools/riff-cli/src/integration/enhance_cli.py",
        home
    ));
    if dev_path.exists() {
        return Ok(dev_path);
    }

    anyhow::bail!(
        "Intent enhancer not found. Tried:\n  - {}\n  - {}",
        installed_path.display(),
        dev_path.display()
    )
}

/// JSON request structure for intent enhancer
#[derive(Serialize)]
struct EnhanceRequest {
    query: String,
    context: String,
}

/// JSON response structure from intent enhancer
#[derive(Deserialize)]
struct EnhanceResponse {
    enhanced_keywords: Vec<String>,
    original_query: String,
    #[serde(default)]
    keyword_count: usize,
}

/// Enhance query via Python intent enhancer subprocess
///
/// This function spawns the Python intent enhancer as a subprocess, sends
/// the query via JSON over stdin, and reads enhanced keywords from stdout.
///
/// Returns:
/// - Ok(Some(enhanced_query)) if enhancement succeeded
/// - Ok(None) if enhancer not available or failed (graceful fallback)
/// - Err only for critical failures
fn enhance_query_via_python(query: &str) -> Result<Option<String>> {
    // Check if NABI_DEBUG is set for verbose output
    let debug = std::env::var("NABI_DEBUG").is_ok();

    // Locate the intent enhancer script
    let enhancer_path = match locate_intent_enhancer() {
        Ok(path) => path,
        Err(e) => {
            if debug {
                eprintln!("Debug: Intent enhancer not found: {}", e);
            }
            return Ok(None); // Graceful fallback
        }
    };

    if debug {
        eprintln!("Debug: Using intent enhancer at: {}", enhancer_path.display());
    }

    // Build request JSON
    let request = EnhanceRequest {
        query: query.to_string(),
        context: "docs".to_string(),
    };
    let request_json = serde_json::to_string(&request)
        .context("Failed to serialize enhance request")?;

    // Spawn Python subprocess with timeout
    let mut child = std::process::Command::new("python3")
        .arg(&enhancer_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn intent enhancer subprocess")?;

    // Write request to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(request_json.as_bytes())
            .context("Failed to write to enhancer stdin")?;
    }

    // Wait for completion with timeout (2 seconds)
    let output = match wait_with_timeout(child, std::time::Duration::from_secs(2)) {
        Ok(output) => output,
        Err(e) => {
            if debug {
                eprintln!("Debug: Intent enhancer timeout or error: {}", e);
            }
            return Ok(None); // Graceful fallback
        }
    };

    // Check exit status
    if !output.status.success() {
        if debug {
            eprintln!("Debug: Intent enhancer failed with status: {:?}", output.status);
            if !output.stderr.is_empty() {
                eprintln!("Debug: stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        return Ok(None); // Graceful fallback
    }

    // Parse response JSON
    let response: EnhanceResponse = match serde_json::from_slice(&output.stdout) {
        Ok(r) => r,
        Err(e) => {
            if debug {
                eprintln!("Debug: Failed to parse enhancer response: {}", e);
                eprintln!("Debug: stdout: {}", String::from_utf8_lossy(&output.stdout));
            }
            return Ok(None); // Graceful fallback
        }
    };

    if debug {
        eprintln!("Debug: Enhanced {} -> {} keywords", query, response.keyword_count);
        eprintln!("Debug: Keywords: {:?}", response.enhanced_keywords);
    }

    // Build enhanced ripgrep pattern: (word1|word2|word3)
    if response.enhanced_keywords.is_empty() {
        return Ok(None);
    }

    let enhanced_pattern = format!("({})", response.enhanced_keywords.join("|"));

    if debug {
        eprintln!("Debug: Enhanced pattern: {}", enhanced_pattern);
    }

    Ok(Some(enhanced_pattern))
}

/// Wait for child process with timeout
///
/// This is a simple timeout implementation that polls the child process.
/// For production use, consider using tokio::time::timeout with async.
fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: std::time::Duration,
) -> Result<std::process::Output> {
    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(50);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process completed
                let stdout = {
                    let mut buf = Vec::new();
                    if let Some(mut out) = child.stdout.take() {
                        out.read_to_end(&mut buf)?;
                    }
                    buf
                };
                let stderr = {
                    let mut buf = Vec::new();
                    if let Some(mut err) = child.stderr.take() {
                        err.read_to_end(&mut buf)?;
                    }
                    buf
                };
                return Ok(std::process::Output { status, stdout, stderr });
            }
            Ok(None) => {
                // Process still running
                if start.elapsed() > timeout {
                    child.kill()?;
                    anyhow::bail!("Process timed out after {:?}", timeout);
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }
}

fn handle_scan_docs(query: &str, type_filter: Option<Vec<ScanSourceType>>, path: Option<&str>, exact: bool) -> Result<()> {
    println!(
        "{}",
        format!("📚 Searching docs for: '{}'", query).cyan().bold()
    );

    // Determine target directory: use provided path or default to ~/docs
    let docs_dir = if let Some(p) = path {
        expand_home(p)?.to_string_lossy().to_string()
    } else {
        let home = std::env::var("HOME").context("Could not determine HOME directory")?;
        format!("{}/docs", home)
    };

    // Verify docs directory exists
    if !std::path::Path::new(&docs_dir).exists() {
        anyhow::bail!(
            "Documentation directory not found: {}",
            docs_dir
        );
    }

    // Skip enhancement if --exact flag is set
    let search_pattern = if exact {
        query.to_string()
    } else {
        // Enhance query via Python intent enhancer (graceful fallback on failure)
        match enhance_query_via_python(query) {
            Ok(Some(enhanced)) => {
                // Debug output for enhanced queries
                if std::env::var("NABI_DEBUG").is_ok() {
                    eprintln!("Debug: Using enhanced pattern");
                }
                enhanced
            }
            Ok(None) => {
                // Enhancement not available or failed - use original query
                if std::env::var("NABI_DEBUG").is_ok() {
                    eprintln!("Debug: Using original query (enhancement unavailable)");
                }
                query.to_string()
            }
            Err(e) => {
                // Critical error in enhancement - warn but continue
                eprintln!("Warning: Query enhancement failed: {}", e);
                query.to_string()
            }
        }
    };

    // Build ripgrep command
    let mut cmd = std::process::Command::new("rg");

    // Color and formatting options
    cmd.arg("--color=always")
        .arg("--heading")
        .arg("--line-number")
        .arg("--smart-case");

    // Apply type filtering if specified
    if let Some(types) = type_filter {
        for ext in types {
            cmd.arg("--glob")
                .arg(format!("*.{}", ext.as_extension()));
        }
    }

    // Add search pattern (enhanced or original) and target directory
    cmd.arg(&search_pattern).arg(&docs_dir);

    println!();

    // Execute ripgrep
    let output = cmd
        .output()
        .context("Failed to execute ripgrep. Is 'rg' installed?")?;

    // Print stdout (preserving colors)
    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        println!(
            "{}",
            format!("No matches found for '{}'", query).yellow()
        );
    }

    // Print stderr if present (but don't fail on it)
    if !output.stderr.is_empty() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    }

    // Exit code 1 from ripgrep means "no matches" which is not an error
    if !output.status.success() && output.status.code() != Some(1) {
        eprintln!(
            "{}",
            format!(
                "⚠️  ripgrep returned non-zero exit code ({})",
                output.status.code().unwrap_or(-1)
            )
            .yellow()
        );
    }

    Ok(())
}

fn handle_scan_path_with_types(path: &str, type_filter: Option<Vec<ScanSourceType>>) -> Result<()> {
    // Expand tilde in path
    let expanded_path = expand_home(path)?;

    // Verify path exists
    if !expanded_path.exists() {
        anyhow::bail!("Path not found: {}", path);
    }

    println!(
        "{}",
        format!("📚 Scanning {} for matching files", expanded_path.display()).cyan().bold()
    );

    // Build find command
    let mut cmd = std::process::Command::new("find");
    cmd.arg("-P") // Don't follow symlinks
        .arg(&expanded_path)
        .arg("-type")
        .arg("f");

    // Apply type filtering if specified
    if let Some(types) = type_filter {
        if !types.is_empty() {
            // Build -name pattern: -name "*.md" -o -name "*.txt" etc.
            let mut name_args: Vec<String> = Vec::new();
            for (i, ext) in types.iter().enumerate() {
                if i > 0 {
                    name_args.push("-o".to_string());
                }
                name_args.push("-name".to_string());
                name_args.push(format!("*.{}", ext.as_extension()));
            }
            // Convert Vec<String> to Vec<&str> for args()
            let name_args_refs: Vec<&str> = name_args.iter().map(|s| s.as_str()).collect();
            cmd.arg("(").args(&name_args_refs).arg(")");
        }
    }

    // Exclude common unwanted directories and files
    cmd.args(&[
        "!", "-path", "*/.git/*",
        "!", "-path", "*/__pycache__/*",
        "!", "-path", "*/node_modules/*",
        "!", "-path", "*/target/*",
        "!", "-name", "*.pyc",
        "!", "-name", ".DS_Store",
    ]);

    println!();

    // Execute find
    let output = cmd
        .output()
        .context("Failed to execute find command")?;

    // Print stdout
    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        println!(
            "{}",
            format!("No matching files found in {}", expanded_path.display()).yellow()
        );
    }

    // Print stderr if present (but don't fail on it)
    if !output.stderr.is_empty() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}

// Removed: handle_watch - now in handlers/watch.rs with subcommand support

fn handle_orgtime(path: String, category: Option<String>, files: bool, preserve_times: bool, dry_run: bool) -> Result<()> {
    crate::commands::orgtime::cmd_run(path, category, files, preserve_times, dry_run)
}

// handle_aura moved to handlers/aura.rs

fn handle_configure(command: ConfigureCommands) -> Result<()> {
    match command {
        ConfigureCommands::Show => {
            println!("{}", "⚙️  Configuration".cyan().bold());
            route_to_python_cli(&["configure", "show"])
        }
        ConfigureCommands::Set { key, value } => {
            println!(
                "{}",
                format!("✏️  Setting {} = {}...", key, value).cyan().bold()
            );
            route_to_python_cli(&["configure", "set", &key, &value])
        }
        ConfigureCommands::Reset => {
            println!("{}", "🔄 Resetting configuration...".yellow().bold());
            route_to_python_cli(&["configure", "reset"])
        }
    }
}

fn handle_db(command: DbCommands) -> Result<()> {
    match command {
        DbCommands::Init => {
            println!("{}", "🗄️  Initializing database...".blue().bold());
            route_to_python_cli(&["db", "init"])
        }
        DbCommands::Export { path } => {
            println!(
                "{}",
                format!("💾 Exporting database to {}...", path)
                    .blue()
                    .bold()
            );
            route_to_python_cli(&["db", "export", &path])
        }
        DbCommands::Import { path } => {
            println!(
                "{}",
                format!("📥 Importing database from {}...", path)
                    .blue()
                    .bold()
            );
            route_to_python_cli(&["db", "import", &path])
        }
    }
}

fn handle_record(command: RecordCommands) -> Result<()> {
    match command {
        RecordCommands::Start { output } => {
            println!("{}", "🎬 Starting recording...".cyan().bold());
            if let Some(o) = output {
                route_to_commander("record", &["start", "--output", &o])
            } else {
                route_to_commander("record", &["start"])
            }
        }
        RecordCommands::Stop { id } => {
            println!("{}", "⏹️  Stopping recording...".cyan().bold());
            if let Some(i) = id {
                route_to_commander("record", &["stop", &i])
            } else {
                route_to_commander("record", &["stop"])
            }
        }
        RecordCommands::List => {
            println!("{}", "📋 Listing recordings...".cyan().bold());
            route_to_commander("record", &["list"])
        }
        RecordCommands::Server { action } => match action {
            ServerActions::Start => {
                println!("{}", "🚀 Starting server...".green().bold());
                route_to_commander("record", &["server", "start"])
            }
            ServerActions::Stop => {
                println!("{}", "⏹️  Stopping server...".yellow().bold());
                route_to_commander("record", &["server", "stop"])
            }
            ServerActions::Status => {
                println!("{}", "📊 Server status...".cyan().bold());
                route_to_commander("record", &["server", "status"])
            }
        },
        RecordCommands::Config { action } => match action {
            ConfigActions::Show => {
                println!("{}", "⚙️  Configuration...".cyan().bold());
                route_to_commander("record", &["config", "show"])
            }
            ConfigActions::Set { key, value } => {
                println!(
                    "{}",
                    format!("✏️  Setting {} = {}...", key, value).cyan().bold()
                );
                route_to_commander("record", &["config", "set", &key, &value])
            }
        },
    }
}

/// Kernel configuration spec (matches Jupyter's kernel.json pattern)
///
/// This allows decoupling the Python venv path from the binary.
/// When renaming memchain → core, only update kernel.json, no recompile needed.
///
/// Location: ~/.config/nabi/commanders/kernel.json
/// Pattern: Follows Jupyter's kernel spec standard
///
/// Rename workflow when you change venv names:
///   1. mv ~/.nabi/venvs/memchain ~/.nabi/venvs/core
///   2. Edit ~/.config/nabi/commanders/kernel.json:
///      "kernel_venv": "~/.nabi/venvs/core"
///   3. Done! No code changes needed.
#[derive(Debug, Deserialize)]
struct KernelConfig {
    kernel_venv: String,
    #[serde(default)]
    daemon_script: Option<String>,
    #[serde(default)]
    version: Option<String>,
}

/// Expand ~ to home directory
fn expand_home(path: &str) -> Result<PathBuf> {
    if path.starts_with("~") {
        let home = NabiPaths::home_dir()?;
        Ok(home.join(&path[2..]))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// Load kernel configuration from ~/.config/nabi/commanders/kernel.json
fn load_kernel_config() -> Result<KernelConfig> {
    let config_path = NabiPaths::config_dir()?
        .join("commanders")
        .join("kernel.json");

    if !config_path.exists() {
        eprintln!("{}", "❌ Kernel configuration not found".red().bold());
        eprintln!(
            "{}",
            format!("Expected at: {}", config_path.display()).yellow()
        );
        anyhow::bail!("Missing kernel.json configuration");
    }

    let json_content =
        std::fs::read_to_string(&config_path).context("Failed to read kernel.json")?;

    let config: KernelConfig =
        serde_json::from_str(&json_content).context("Failed to parse kernel.json")?;

    Ok(config)
}

fn handle_backup(command: BackupCommands) -> Result<()> {
    use handlers::backup::handle_backup as backup_handler;
    backup_handler(command)
}

fn handle_agent(command: AgentKernelCommands) -> Result<()> {
    // Load kernel configuration (decoupled from venv name)
    let kernel_cfg = load_kernel_config()?;

    // Resolve Python executable from configured venv
    let kernel_venv = expand_home(&kernel_cfg.kernel_venv)?;
    let python_exe = kernel_venv.join("bin").join("python3");

    if !python_exe.exists() {
        eprintln!(
            "{}",
            "❌ Python executable not found in kernel venv".red().bold()
        );
        eprintln!(
            "{}",
            format!("Expected at: {}", python_exe.display()).yellow()
        );
        let kernel_config_path = NabiPaths::config_dir()?.join("commanders/kernel.json");
        eprintln!(
            "{}",
            format!("   Configured in: {}", kernel_config_path.display()).yellow()
        );
        eprintln!("{}", "   Try: nabi self doctor".yellow());
        process::exit(1);
    }

    // Get agent commander path
    let nabi_config = NabiPaths::config_dir()?.join("commanders").join("agent");

    match command {
        AgentKernelCommands::Daemon { action } => {
            // DEPRECATION WARNING
            eprintln!();
            eprintln!("{}", "⚠️  DEPRECATED COMMAND".yellow().bold());
            eprintln!("{}", "━".repeat(60).yellow());
            eprintln!();
            eprintln!("{}", "  The command 'nabi agent daemon' is deprecated and will be removed in v0.2.0".yellow());
            eprintln!();
            eprintln!("{}", "  Please use 'nabi kernel daemon' instead:".bright_white());
            eprintln!("    {} → {}", "nabi agent daemon start".red(), "nabi kernel daemon start".green());
            eprintln!("    {} → {}", "nabi agent daemon stop".red(), "nabi kernel daemon stop".green());
            eprintln!("    {} → {}", "nabi agent daemon restart".red(), "nabi kernel daemon restart".green());
            eprintln!("    {} → {}", "nabi agent daemon status".red(), "nabi kernel daemon status".green());
            eprintln!();
            eprintln!("{}", "  Reason: 'nabi agent' semantically suggests individual agent control,".bright_black());
            eprintln!("{}", "          but this command controls the NABIKernel orchestration daemon.".bright_black());
            eprintln!();
            eprintln!("{}", "━".repeat(60).yellow());
            eprintln!();
            std::thread::sleep(std::time::Duration::from_secs(2)); // Give user time to read warning

            let daemon_script = nabi_config.join("daemon");
            if !daemon_script.exists() {
                eprintln!("{}", "❌ Agent daemon script not found".red().bold());
                eprintln!(
                    "{}",
                    format!("Expected at: {}", daemon_script.display()).yellow()
                );
                process::exit(1);
            }

            match action {
                DaemonActions::Start { foreground } => {
                    println!("{}", "🚀 Starting NABIKernel daemon...".green().bold());
                    let mut cmd = process::Command::new(&python_exe);
                    cmd.arg(&daemon_script);
                    cmd.arg("start");
                    if foreground {
                        cmd.arg("--foreground");
                    }
                    let status = cmd.status().context(format!(
                        "Failed to execute daemon script at {}",
                        daemon_script.display()
                    ))?;
                    if !status.success() {
                        process::exit(status.code().unwrap_or(1));
                    }
                    Ok(())
                }
                DaemonActions::Stop => {
                    println!("{}", "⏹️  Stopping NABIKernel daemon...".yellow().bold());
                    let mut cmd = process::Command::new(&python_exe);
                    cmd.arg(&daemon_script).arg("stop");
                    let status = cmd.status().context(format!(
                        "Failed to execute daemon script at {}",
                        daemon_script.display()
                    ))?;
                    if !status.success() {
                        process::exit(status.code().unwrap_or(1));
                    }
                    Ok(())
                }
                DaemonActions::Restart => {
                    println!("{}", "🔄 Restarting NABIKernel daemon...".cyan().bold());
                    let mut cmd = process::Command::new(&python_exe);
                    cmd.arg(&daemon_script).arg("restart");
                    let status = cmd.status().context(format!(
                        "Failed to execute daemon script at {}",
                        daemon_script.display()
                    ))?;
                    if !status.success() {
                        process::exit(status.code().unwrap_or(1));
                    }
                    Ok(())
                }
                DaemonActions::Status => {
                    println!(
                        "{}",
                        "📊 Checking NABIKernel daemon status...".cyan().bold()
                    );
                    let mut cmd = process::Command::new(&python_exe);
                    cmd.arg(&daemon_script).arg("status");
                    let status = cmd.status().context(format!(
                        "Failed to execute daemon script at {}",
                        daemon_script.display()
                    ))?;
                    if !status.success() {
                        process::exit(status.code().unwrap_or(1));
                    }
                    Ok(())
                }
            }
        }
        AgentKernelCommands::Spawn {
            agent_type,
            task,
            priority,
        } => {
            println!(
                "{}",
                format!("🤖 Spawning {} agent...", agent_type)
                    .magenta()
                    .bold()
            );
            let spawn_script = nabi_config.join("spawn");
            let mut cmd = process::Command::new(&python_exe);
            cmd.arg(&spawn_script);
            cmd.arg(&agent_type);
            cmd.arg("--priority").arg(&priority);
            if let Some(t) = task {
                cmd.arg("--task").arg(&t);
            }
            let status = cmd.status().context(format!(
                "Failed to execute spawn script at {}",
                spawn_script.display()
            ))?;
            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
            Ok(())
        }
        AgentKernelCommands::Status { agent_id } => {
            println!(
                "{}",
                format!("📊 Checking status for {}...", agent_id)
                    .cyan()
                    .bold()
            );
            let status_script = nabi_config.join("status");
            let mut cmd = process::Command::new(&python_exe);
            cmd.arg(&status_script).arg(&agent_id);
            let status = cmd.status().context(format!(
                "Failed to execute status script at {}",
                status_script.display()
            ))?;
            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
            Ok(())
        }
        AgentKernelCommands::List => {
            println!("{}", "📋 Listing all agents...".cyan().bold());
            let list_script = nabi_config.join("list");
            let mut cmd = process::Command::new(&python_exe);
            cmd.arg(&list_script);
            let status = cmd.status().context(format!(
                "Failed to execute list script at {}",
                list_script.display()
            ))?;
            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
            Ok(())
        }
        AgentKernelCommands::Kill { agent_id } => {
            println!(
                "{}",
                format!("⚔️  Killing agent {}...", agent_id).red().bold()
            );
            let kill_script = nabi_config.join("kill");
            let mut cmd = process::Command::new(&python_exe);
            cmd.arg(&kill_script).arg(&agent_id);
            let status = cmd.status().context(format!(
                "Failed to execute kill script at {}",
                kill_script.display()
            ))?;
            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
            Ok(())
        }
        AgentKernelCommands::Wait { agent_id } => {
            println!(
                "{}",
                format!("⏳ Waiting for agent {}...", agent_id)
                    .yellow()
                    .bold()
            );
            let wait_script = nabi_config.join("wait");
            let mut cmd = process::Command::new(&python_exe);
            cmd.arg(&wait_script).arg(&agent_id);
            let status = cmd.status().context(format!(
                "Failed to execute wait script at {}",
                wait_script.display()
            ))?;
            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
            Ok(())
        }
    }
}

fn handle_port(command: PortCommands) -> Result<()> {
    match command {
        PortCommands::List { platform } => {
            println!("{}", "📋 Listing port allocations...".cyan().bold());
            port::cmd_list(platform.as_deref())
        }
        PortCommands::Check => {
            println!("{}", "🔍 Validating port allocations...".cyan().bold());
            port::cmd_check()
        }
        PortCommands::CrossPlatform => {
            println!(
                "{}",
                "🌐 Checking cross-platform conflicts...".cyan().bold()
            );
            port::cmd_cross_platform()
        }
        PortCommands::Shift {
            service,
            old_port,
            new_port,
            dry_run,
        } => {
            println!(
                "{}",
                format!(
                    "🔄 Migrating {} from {} to {}...",
                    service, old_port, new_port
                )
                .yellow()
                .bold()
            );
            port::cmd_shift(&service, old_port, new_port, dry_run)
        }
        PortCommands::Drift { forensic, since } => {
            println!("{}", "🔎 Analyzing port drift...".yellow().bold());
            port::cmd_drift(forensic, since.as_deref())
        }
        PortCommands::Fix => {
            println!("{}", "🔧 Generating fix commands...".green().bold());
            port::cmd_fix()
        }
        PortCommands::GenerateEnv => {
            println!("{}", "📝 Generating .env file...".cyan().bold());
            port::cmd_generate_env()
        }
    }
}

fn handle_services(command: ServicesCommands) -> Result<()> {
    match command {
        ServicesCommands::Status { format } => {
            println!("{}", "📊 Checking service status...".cyan().bold());
            services::cmd_status(format.as_deref())
        }
        ServicesCommands::Validate => {
            services::cmd_validate()
        }
        ServicesCommands::Rebuild { group } => {
            services::cmd_rebuild(&group)
        }
        ServicesCommands::Deploy { group } => {
            services::cmd_deploy(&group)
        }
    }
}

fn handle_hooks(command: HooksCommands) -> Result<()> {
    match command {
        HooksCommands::Debug { action } => handle_hook_debug(action),
        HooksCommands::Transform { stable } => {
            if stable {
                println!(
                    "{}",
                    "🔗 Using stable hooks from ~/.nabi/src/hooks..."
                        .cyan()
                        .bold()
                );

                // Use stable hooks: copy from ~/.nabi/src/hooks/src/ to deployment location
                let home = dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
                let stable_hooks_src = home.join(".nabi/src/hooks/src");
                let hooks_deploy = NabiPaths::data_dir()?.join("bin").join("hooks");

                // Ensure deployment directory exists
                fs::create_dir_all(&hooks_deploy).context(format!(
                    "Failed to create hooks directory at {}",
                    hooks_deploy.display()
                ))?;

                if !stable_hooks_src.exists() {
                    eprintln!(
                        "{}",
                        format!(
                            "❌ Stable hooks not found at: {}",
                            stable_hooks_src.display()
                        )
                        .red()
                        .bold()
                    );
                    eprintln!(
                        "{}",
                        "Expected stable hooks at ~/.nabi/src/hooks/src/".yellow()
                    );
                    process::exit(1);
                }

                // Generate hook_wrapper.sh first (required for hook execution)
                let hook_wrapper_script = stable_hooks_src
                    .parent()
                    .and_then(|p| p.parent())
                    .map(|p| p.join("src").join("transform_hook_wrapper.py"))
                    .ok_or_else(|| anyhow::anyhow!("Could not resolve transform script path"))?;

                if hook_wrapper_script.exists() {
                    println!("{}", "  Generating hook_wrapper.sh...".dimmed());

                    // Find Python executable
                    let python_exe = std::env::var("NABI_PYTHON")
                        .ok()
                        .map(PathBuf::from)
                        .or_else(|| {
                            std::process::Command::new("python3")
                                .arg("--version")
                                .output()
                                .ok()
                                .map(|_| PathBuf::from("python3"))
                        })
                        .or_else(|| {
                            std::process::Command::new("python")
                                .arg("--version")
                                .output()
                                .ok()
                                .map(|_| PathBuf::from("python"))
                        })
                        .ok_or_else(|| anyhow::anyhow!("Python not found"))?;

                    let status = process::Command::new(&python_exe)
                        .arg(&hook_wrapper_script)
                        .arg("--output")
                        .arg(hooks_deploy.join("hook_wrapper.sh"))
                        .status()
                        .context("Failed to generate hook_wrapper.sh")?;

                    if !status.success() {
                        eprintln!("{}", "❌ Failed to generate hook_wrapper.sh".red().bold());
                        process::exit(status.code().unwrap_or(1));
                    }
                }

                // Copy hook files from stable location
                let mut copied = 0;
                if let Ok(entries) = fs::read_dir(&stable_hooks_src) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |e| e == "py") && path.is_file() {
                            let filename = path.file_name()
                                .ok_or_else(|| anyhow::anyhow!("Path has no filename: {}", path.display()))?;
                            let dest = hooks_deploy.join(filename);
                            fs::copy(&path, &dest).context(format!(
                                "Failed to copy {} to {}",
                                path.display(),
                                dest.display()
                            ))?;
                            copied += 1;
                        }
                    }
                }

                println!(
                    "{}",
                    format!(
                        "✓ Copied {} hook files to {}",
                        copied,
                        hooks_deploy.display()
                    )
                    .green()
                );
                Ok(())
            } else {
                println!(
                    "{}",
                    "🔄 Transforming hooks from schema to derived state..."
                        .cyan()
                        .bold()
                );

                // Run transformation scripts
                let home = dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
                let transform_scripts_dir = home.join(".nabi/src/hooks/src");

                // Find Python executable
                let python_exe = std::env::var("NABI_PYTHON")
                    .ok()
                    .map(PathBuf::from)
                    .or_else(|| {
                        std::process::Command::new("python3")
                            .arg("--version")
                            .output()
                            .ok()
                            .map(|_| PathBuf::from("python3"))
                    })
                    .or_else(|| {
                        std::process::Command::new("python")
                            .arg("--version")
                            .output()
                            .ok()
                            .map(|_| PathBuf::from("python"))
                    })
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Python not found. Set NABI_PYTHON or ensure python3/python is in PATH"
                        )
                    })?;

                // Generate hook_wrapper.sh first (required for hook execution)
                let hooks_deploy = NabiPaths::data_dir()?.join("bin").join("hooks");
                fs::create_dir_all(&hooks_deploy).context(format!(
                    "Failed to create hooks directory at {}",
                    hooks_deploy.display()
                ))?;

                let hook_wrapper_script = transform_scripts_dir.join("transform_hook_wrapper.py");
                if hook_wrapper_script.exists() {
                    println!("{}", "  Generating hook_wrapper.sh...".dimmed());

                    let status = process::Command::new(&python_exe)
                        .arg(&hook_wrapper_script)
                        .arg("--output")
                        .arg(hooks_deploy.join("hook_wrapper.sh"))
                        .status()
                        .context(format!("Failed to generate hook_wrapper.sh"))?;

                    if !status.success() {
                        eprintln!("{}", "❌ Failed to generate hook_wrapper.sh".red().bold());
                        process::exit(status.code().unwrap_or(1));
                    }
                } else {
                    eprintln!(
                        "{}",
                        format!(
                            "⚠️  hook_wrapper.sh generator not found: {}",
                            hook_wrapper_script.display()
                        )
                        .yellow()
                        .bold()
                    );
                }

                // Find and execute all transform_*.py scripts
                let mut executed = 0;
                if let Ok(entries) = fs::read_dir(&transform_scripts_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map_or(false, |n| {
                                    n.starts_with("transform_") && n.ends_with(".py")
                                })
                        {
                            // Skip hook_wrapper transform (already done above)
                            if path.file_name().and_then(|n| n.to_str())
                                == Some("transform_hook_wrapper.py")
                            {
                                continue;
                            }

                            let filename = path.file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_else(|| format!("{}", path.display()));

                            println!(
                                "{}",
                                format!("  Running {}...", filename)
                                .dimmed()
                            );

                            let status = process::Command::new(&python_exe)
                                .arg(&path)
                                .status()
                                .context(format!(
                                    "Failed to execute transform script: {}",
                                    path.display()
                                ))?;

                            if !status.success() {
                                eprintln!(
                                    "{}",
                                    format!("❌ Transform script failed: {}", path.display())
                                        .red()
                                        .bold()
                                );
                                process::exit(status.code().unwrap_or(1));
                            }

                            executed += 1;
                        }
                    }
                }

                if executed == 0 {
                    eprintln!(
                        "{}",
                        format!(
                            "⚠️  No transformation scripts found at: {}",
                            transform_scripts_dir.display()
                        )
                        .yellow()
                        .bold()
                    );
                    eprintln!(
                        "{}",
                        "Expected transform_*.py scripts in ~/.nabi/src/hooks/src/".yellow()
                    );
                } else {
                    println!(
                        "{}",
                        format!("✓ Executed {} transformation script(s)", executed).green()
                    );
                }

                Ok(())
            }
        }
    }
}

fn handle_deckgen(command: DeckgenCommands) -> Result<()> {
    match command {
        DeckgenCommands::Trace { output } => {
            let trace = deckgen::TRACE_SAMPLE.trim_end();
            if let Some(path) = output {
                fs::write(&path, trace)?;
                println!(
                    "{}",
                    format!("📝 Wrote deck trace to {}", path.display())
                        .green()
                        .bold()
                );
            } else {
                println!("{}", trace);
            }
            Ok(())
        }
    }
}

fn handle_hook_debug(action: HookDebugActions) -> Result<()> {
    let debug_dir = NabiPaths::state_dir()?.join("hook-debug");

    match action {
        HookDebugActions::Tail { hook_name } => {
            let today = chrono::Local::now().format("%Y%m%d").to_string();
            let log_file = debug_dir.join(format!("{}_{}.jsonl", hook_name, today));

            if !log_file.exists() {
                eprintln!(
                    "{}",
                    format!("❌ No debug log found for {} today", hook_name)
                        .red()
                        .bold()
                );
                eprintln!(
                    "{}",
                    format!("Looking for: {}", log_file.display()).yellow()
                );
                eprintln!(
                    "{}",
                    "\nMake sure NABI_HOOK_DEBUG=1 is set and the hook has been called".yellow()
                );
                process::exit(1);
            }

            println!(
                "{}",
                format!("🔍 Tailing debug log: {}", log_file.display())
                    .green()
                    .bold()
            );
            println!();

            // Use tail -f to follow the file
            let mut cmd = process::Command::new("tail");
            cmd.arg("-f").arg(&log_file);
            let status = cmd
                .status()
                .context(format!("Failed to tail log file: {}", log_file.display()))?;

            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
            Ok(())
        }
        HookDebugActions::List => {
            if !debug_dir.exists() {
                println!("{}", "No debug directory found".yellow());
                return Ok(());
            }

            println!("{}", "📋 Debug logs:".blue().bold());

            let mut log_files: Vec<_> = fs::read_dir(&debug_dir)?
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("jsonl"))
                .collect();

            log_files.sort_by_key(|e| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            });

            for entry in log_files.iter().rev() {
                let path = entry.path();
                let metadata = entry.metadata()?;
                let size = metadata.len();
                let modified = metadata
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();

                let size_str = if size < 1024 {
                    format!("{}B", size)
                } else if size < 1024 * 1024 {
                    format!("{}KB", size / 1024)
                } else {
                    format!("{}MB", size / (1024 * 1024))
                };

                let modified_str =
                    chrono::DateTime::<chrono::Local>::from(std::time::UNIX_EPOCH + modified)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string();

                let filename = path.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("{}", path.display()));

                println!(
                    "  {} - {} - {}",
                    filename,
                    size_str.dimmed(),
                    modified_str.dimmed()
                );
            }

            Ok(())
        }
        HookDebugActions::Search { hook_name, term } => {
            let today = chrono::Local::now().format("%Y%m%d").to_string();
            let log_file = debug_dir.join(format!("{}_{}.jsonl", hook_name, today));

            if !log_file.exists() {
                eprintln!("{}", "No debug log found".yellow());
                process::exit(1);
            }

            println!(
                "{}",
                format!("🔍 Searching for '{}' in {} logs:", term, hook_name)
                    .green()
                    .bold()
            );

            let content = fs::read_to_string(&log_file)?;
            for line in content.lines() {
                if line.to_lowercase().contains(&term.to_lowercase()) {
                    // Try to parse as JSON and extract key fields
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                        let timestamp =
                            json.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
                        let level = json.get("level").and_then(|v| v.as_str()).unwrap_or("");
                        let message = json.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        println!("  [{}] {}: {}", timestamp, level, message);
                    } else {
                        println!("  {}", line);
                    }
                }
            }

            Ok(())
        }
        HookDebugActions::Errors { hook_name } => {
            let today = chrono::Local::now().format("%Y%m%d").to_string();
            let log_file = debug_dir.join(format!("{}_{}.jsonl", hook_name, today));

            if !log_file.exists() {
                eprintln!("{}", "No debug log found".yellow());
                process::exit(1);
            }

            println!("{}", format!("❌ Errors for {}:", hook_name).red().bold());

            let content = fs::read_to_string(&log_file)?;
            for line in content.lines() {
                if line.contains("\"level\":\"error\"") {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                        let timestamp =
                            json.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
                        let message = json.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        let exception = json
                            .get("data")
                            .and_then(|d| d.get("exception"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        println!("  [{}] {} {}", timestamp, message, exception);
                    } else {
                        println!("  {}", line);
                    }
                }
            }

            Ok(())
        }
        HookDebugActions::Stats { hook_name } => {
            let today = chrono::Local::now().format("%Y%m%d").to_string();
            let log_file = debug_dir.join(format!("{}_{}.jsonl", hook_name, today));

            if !log_file.exists() {
                eprintln!("{}", "No debug log found".yellow());
                process::exit(1);
            }

            println!(
                "{}",
                format!("📊 Statistics for {}:", hook_name).blue().bold()
            );
            println!();

            let content = fs::read_to_string(&log_file)?;
            let mut total_calls = 0;
            let mut error_count = 0;
            let mut durations = Vec::new();
            let mut level_counts = std::collections::HashMap::new();

            for line in content.lines() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                    if json.get("message").and_then(|v| v.as_str()) == Some("Hook started") {
                        total_calls += 1;
                    }

                    if json.get("level").and_then(|v| v.as_str()) == Some("error") {
                        error_count += 1;
                    }

                    if let Some(level) = json.get("level").and_then(|v| v.as_str()) {
                        *level_counts.entry(level.to_string()).or_insert(0) += 1;
                    }

                    if let Some(data) = json.get("data") {
                        if let Some(duration) = data.get("total_duration_ms") {
                            if let Some(d) = duration.as_f64() {
                                durations.push(d);
                            }
                        }
                    }
                }
            }

            println!("Total calls: {}", total_calls);

            if !durations.is_empty() {
                let sum: f64 = durations.iter().sum();
                let avg = sum / durations.len() as f64;
                println!("Average duration: {:.2}ms", avg);
            }

            println!("Errors: {}", error_count);

            if !level_counts.is_empty() {
                println!();
                println!("Log level breakdown:");
                let mut levels: Vec<_> = level_counts.iter().collect();
                levels.sort_by(|a, b| b.1.cmp(a.1));
                for (level, count) in levels {
                    println!("  {}: {}", level, count);
                }
            }

            Ok(())
        }
        HookDebugActions::Replay { hook_name } => {
            let today = chrono::Local::now().format("%Y%m%d").to_string();
            let replay_file = debug_dir.join(format!("{}_{}.replay.json", hook_name, today));

            if !replay_file.exists() {
                eprintln!("{}", "No replay file found".yellow());
                process::exit(1);
            }

            let content = fs::read_to_string(&replay_file)?;
            let json: serde_json::Value = serde_json::from_str(&content)?;
            println!("{}", serde_json::to_string_pretty(&json)?);

            Ok(())
        }
        HookDebugActions::Clear => {
            if !debug_dir.exists() {
                println!("{}", "No debug directory found".yellow());
                return Ok(());
            }

            println!("{}", "⚠️  Clearing all debug logs...".yellow().bold());
            print!("Are you sure? [y/N] ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if input.trim().to_lowercase() == "y" {
                for entry in fs::read_dir(&debug_dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_file() {
                        fs::remove_file(&path)?;
                    }
                }
                println!("{}", "✓ Debug logs cleared".green());
            } else {
                println!("{}", "Cancelled".yellow());
            }

            Ok(())
        }
        HookDebugActions::Enable { level } => {
            println!("{}", "✅ Debug mode enabled".green().bold());
            println!();
            println!("Add these to your shell profile or run before calling hooks:");
            println!();
            println!("export NABI_HOOK_DEBUG=1");
            println!("export NABI_HOOK_DEBUG_LEVEL={}", level);
            println!("export NABI_HOOK_STDERR=1");
            println!();
            println!("To enable Python debugger (ipdb/pdb):");
            println!("export NABI_HOOK_PDB=1");

            Ok(())
        }
        HookDebugActions::Disable => {
            println!("{}", "⚠️  Debug mode disabled".yellow().bold());
            println!();
            println!("Unset these environment variables:");
            println!();
            println!("unset NABI_HOOK_DEBUG");
            println!("unset NABI_HOOK_DEBUG_LEVEL");
            println!("unset NABI_HOOK_PDB");
            println!("unset NABI_HOOK_STDERR");

            Ok(())
        }
    }
}

fn handle_mode(mode: Option<String>) -> Result<()> {
    // Route to bash CLI for mode management
    let nabi_config = NabiPaths::config_dir()?;

    let bash_cli = nabi_config.join("lib").join("nabi-cli.sh");

    if !bash_cli.exists() {
        eprintln!(
            "{}",
            format!("❌ Bash CLI not found at: {}", bash_cli.display())
                .red()
                .bold()
        );
        process::exit(1);
    }

    let mut cmd = process::Command::new(&bash_cli);
    cmd.arg("mode");
    if let Some(m) = mode {
        cmd.arg(m);
    }

    let status = cmd.status().context(format!(
        "Failed to execute bash CLI at {}",
        bash_cli.display()
    ))?;

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}


fn handle_recover(command: RecoverCommands) -> Result<()> {
    match command {
        RecoverCommands::Sessions {
            hours,
            detailed,
            export,
        } => {
            println!(
                "{}",
                format!("🔄 Recovering sessions from last {} hours...", hours)
                    .cyan()
                    .bold()
            );

            let hours_str = hours.to_string();
            let detailed_str = "--detailed".to_string();
            let export_flag = "--export".to_string();

            let mut args = vec!["recover", "sessions", "--hours", &hours_str];

            if detailed {
                args.push(&detailed_str);
            }

            if let Some(ref path) = export {
                args.push(&export_flag);
                args.push(path);
            }

            route_to_python_cli(&args)
        }
    }
}

fn handle_health(command: HealthCommands) -> Result<()> {
    match command {
        HealthCommands::Quick => {
            println!("⚠️  NOTE: Replaces deprecated 'nabi doctor'");
            handlers::health::health_quick()?;
            Ok(())
        },

        HealthCommands::Substrate { auto_remediate, fsm_only } => {
            println!("⚠️  NOTE: Replaces deprecated 'nabi health check'");
            println!("{}", "🏥 Running federation substrate health checks...".green().bold());

            let mut args = vec!["health", "check"];
            if auto_remediate {
                args.push("--auto-remediate");
            }
            if fsm_only {
                args.push("--fsm-only");
            }
            route_to_python_cli(&args)
        },

        HealthCommands::Services => {
            println!("⚠️  NOTE: Replaces deprecated 'nabi federation health'");
            handlers::health::health_services()
        },

        HealthCommands::Ports => {
            handlers::health::health_ports()
        },

        HealthCommands::Check {
            auto_remediate,
            fsm_only,
        } => {
            println!(
                "{}",
                "🏥 Running federation substrate health checks..."
                    .green()
                    .bold()
            );

            let mut args = vec!["health", "check"];

            if auto_remediate {
                args.push("--auto-remediate");
            }

            if fsm_only {
                args.push("--fsm-only");
            }

            route_to_python_cli(&args)
        }
        HealthCommands::Status { detailed, hours } => {
            println!("{}", "📊 Checking health status...".cyan().bold());

            let hours_str = hours.to_string();
            let mut args = vec!["health", "status", "--hours", &hours_str];

            if detailed {
                args.push("--detailed");
            }

            route_to_python_cli(&args)
        }
        HealthCommands::Report { format, output } => {
            println!(
                "{}",
                format!("📋 Generating health report ({} format)...", format)
                    .cyan()
                    .bold()
            );

            let mut args = vec!["health", "report", "--format", &format];

            if let Some(ref path) = output {
                args.push("--output");
                args.push(path);
            }

            route_to_python_cli(&args)
        }
        HealthCommands::Dashboard { port } => {
            println!(
                "{}",
                format!(
                    "📈 Opening Grafana dashboard at http://localhost:{}...",
                    port
                )
                .blue()
                .bold()
            );

            let port_str = port.to_string();
            route_to_python_cli(&["health", "dashboard", "--port", &port_str])
        }
    }
}

/// Route commands to Python CLI for filesystem operations
fn route_to_python_cli(args: &[&str]) -> Result<()> {
    let bin_dir = NabiPaths::bin_dir()?;
    let python_cli = bin_dir.join("nabi-python");

    if python_cli.exists() {
        let mut cmd = process::Command::new(&python_cli);
        cmd.args(args);
        let status = cmd.status().context(format!(
            "Failed to execute Python CLI at {}",
            python_cli.display()
        ))?;

        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    } else {
        eprintln!(
            "{}",
            format!("❌ Python CLI not found at: {}", python_cli.display())
                .red()
                .bold()
        );
        eprintln!("{}", "Run 'nabi self doctor' to diagnose issues.".yellow());
        process::exit(1);
    }
}

fn list_tools(
    format: &str,
    status_filter: Option<&str>,
    runtime_filter: Option<&str>,
) -> Result<()> {
    // Only show header for non-JSON formats (JSON needs clean output for parsing)
    if format != "json" {
        println!("{}", "📋 Listing registered tools...".cyan().bold());
    }

    let tools_dir = NabiPaths::config_dir()?.join("tools");

    if !tools_dir.exists() {
        println!("{}", "  No tools registered yet".dimmed());
        return Ok(());
    }

    let mut tools: Vec<(String, String, String, String, String, String, String)> = Vec::new();

    // Read all TOML files from the tools directory
    for entry in fs::read_dir(&tools_dir)
        .with_context(|| format!("Failed to read tools directory at {}", tools_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
            // Extract tool information
            if let Some(tool_section) = parsed.get("tool").and_then(|v| v.as_table()) {
                let id = tool_section
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let name = tool_section
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&id)
                    .to_string();
                let version = tool_section
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0")
                    .to_string();
                let description = tool_section
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let tool_status = tool_section
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let runtime = parsed
                    .get("runtime")
                    .and_then(|r| r.get("language"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let commands = parsed
                    .get("commands")
                    .and_then(|c| c.get("commands"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();

                // Apply filters
                if let Some(status) = status_filter {
                    if tool_status != status {
                        continue;
                    }
                }

                if let Some(rt) = runtime_filter {
                    if !runtime.contains(rt) {
                        continue;
                    }
                }

                tools.push((
                    id,
                    name,
                    version,
                    description,
                    tool_status,
                    runtime,
                    commands,
                ));
            }
        }
    }

    // Sort by name
    tools.sort_by(|a, b| a.1.cmp(&b.1));

    match format {
        "json" => output_tools_json(&tools)?,
        "text" | _ => output_tools_text(&tools)?,
    }

    Ok(())
}

fn output_tools_text(
    tools: &[(String, String, String, String, String, String, String)],
) -> Result<()> {
    if tools.is_empty() {
        println!("{}", "  No tools found matching the criteria".dimmed());
        return Ok(());
    }

    println!(
        "\n  {:<20} {:<15} {:<12} {:<12} {}",
        "Name".bold(),
        "Version".bold(),
        "Runtime".bold(),
        "Status".bold(),
        "Commands"
    );
    println!("  {}", "─".repeat(80).dimmed());

    for (_id, name, version, _description, status, runtime, commands) in tools {
        let status_colored = match status.as_str() {
            "active" => status.green(),
            "inactive" => status.yellow(),
            "deprecated" => status.red(),
            _ => status.normal(),
        };

        println!(
            "  {:<20} {:<15} {:<12} {:<12} {}",
            name.normal(),
            version.dimmed(),
            runtime.cyan(),
            status_colored,
            commands.dimmed()
        );
    }

    println!("\n  Total: {} tool(s)", tools.len());

    Ok(())
}

fn output_tools_json(
    tools: &[(String, String, String, String, String, String, String)],
) -> Result<()> {
    let json_tools: Vec<serde_json::Value> = tools
        .iter()
        .map(
            |(id, name, version, description, status, runtime, commands)| {
                serde_json::json!({
                    "id": id,
                    "name": name,
                    "version": version,
                    "description": description,
                    "status": status,
                    "runtime": runtime,
                    "commands": commands.split(", ").collect::<Vec<_>>(),
                })
            },
        )
        .collect();

    let output = serde_json::json!({
        "tools": json_tools,
        "count": json_tools.len(),
    });

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

fn handle_migrate(command: cli::MigrateCommands) -> Result<()> {
    use crate::handlers::migrate;

    match command {
        cli::MigrateCommands::Run { dir, dry_run, force, verbose } => {
            migrate::handle_migrate_run(&dir, dry_run, force, verbose)
        }
        cli::MigrateCommands::Verify { dir, verbose } => {
            migrate::handle_migrate_verify(&dir, verbose)
        }
    }
}
