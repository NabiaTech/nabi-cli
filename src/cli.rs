use crate::commands::{events, kernel, tmux};
/// CLI command definitions
///
/// This module contains all the command enum definitions for the nabi CLI.
/// These are separated from handlers to keep the codebase modular and maintainable.
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use std::fmt;
use std::path::{Path, PathBuf};

/// nabi - Unified Federation Command Gateway
///
/// Router to specialized commanders following the principle:
/// "Router, not monolith. Coordination, not control." — Igris
#[derive(Parser)]
#[command(name = "nabi")]
#[command(version, about, long_about = None)]
#[command(author = "Nabia Federation")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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
        /// Search query for docs search
        #[arg(value_name = "QUERY", last = true)]
        query: Option<String>,
    },
    /// File watching and real-time classification
    Watch {
        /// Path to watch
        #[arg(value_name = "PATH")]
        path: Option<String>,
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
        shell: clap_complete::Shell,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Install to shell-specific location
        #[arg(long)]
        install: bool,
    },
    /// deckgen utilities (schema + trace fixtures)
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
        command: MigrateCommands,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ScanSourceType {
    #[value(name = "md", alias = "markdown", help = "Markdown files (*.md)")]
    Markdown,
    #[value(name = "txt", alias = "text", help = "Plain text files (*.txt)")]
    Text,
    #[value(name = "toml", help = "TOML files (*.toml)")]
    Toml,
    #[value(name = "json", help = "JSON files (*.json)")]
    Json,
    #[value(name = "yaml", alias = "yml", help = "YAML files (*.yaml, *.yml)")]
    Yaml,
}

impl ScanSourceType {
    pub fn as_extension(&self) -> &'static str {
        match self {
            ScanSourceType::Markdown => "md",
            ScanSourceType::Text => "txt",
            ScanSourceType::Toml => "toml",
            ScanSourceType::Json => "json",
            ScanSourceType::Yaml => "yaml",
        }
    }
}

#[derive(Subcommand)]
pub enum ClaudeCommands {
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
pub enum SessionActions {
    /// List available sessions
    List {
        /// Limit number of sessions to display
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Recover a session by UUID
    Recover {
        /// Session UUID to recover
        uuid: String,
    },
    /// View session details
    View {
        /// Session UUID to view
        uuid: String,
    },
}

#[derive(Subcommand)]
pub enum ProjectActions {
    /// List all projects
    List,
    /// Migrate project to new path
    Migrate {
        /// New project path
        path: String,
    },
}

#[derive(Subcommand)]
pub enum DataCommands {
    /// JSONL operations
    Jsonl {
        #[command(subcommand)]
        action: JsonlActions,
    },
}

#[derive(Subcommand)]
pub enum JsonlActions {
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
pub enum FederationCommands {
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
    /// Federation-wide health substrates
    Health,
    /// Show status of all federation nodes
    Status,
    /// List all active agents in the federation
    Agents,
}

#[derive(Subcommand)]
pub enum DocsCommands {
    /// Manifest management (SHA256 tracking)
    Manifest {
        #[command(subcommand)]
        action: ManifestActions,
    },
}

#[derive(Subcommand)]
pub enum ManifestActions {
    /// List all manifests
    List,
    /// Validate a repository against its manifest
    Validate {
        /// Repository path to validate
        repo_path: String,
    },
    /// Generate a manifest for a repository
    Generate {
        /// Repository path to generate for
        repo_path: String,
    },
}

#[derive(Subcommand)]
pub enum RepoCommands {
    /// Check repository compliance (XDG, hardcoded paths)
    Check {
        /// Path to repository (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Strict mode (exit non-zero on warnings)
        #[arg(short, long)]
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
pub enum CodegraphCommands {
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
pub enum AnalyzeCommands {
    /// Index a repository for code analysis (creates persistent graph)
    ///
    /// Analyzes a codebase and creates a searchable symbol index. The index is cached
    /// and reused on subsequent runs unless --force is specified. Multiple analyses of
    /// the same repository with different languages are stored separately to avoid
    /// overwriting. This enables multi-agent workflows where agents can share cached
    /// analysis results.
    Repo {
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
}

#[derive(Subcommand)]
pub enum GraphActions {
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
pub enum AgentActions {
    /// List active agents
    List,
    /// Spawn new agent
    Spawn {
        /// Agent role
        role: String,
    },
}

#[derive(Subcommand)]
pub enum SyncActions {
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
pub enum RegistryActions {
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
pub enum SelfCommands {
    /// Health check all commanders
    Doctor,
    /// Update all components
    Update,
    /// Show configuration
    Config,
    /// Generate CLI command specification
    Spec {
        /// Output format (markdown or json)
        #[arg(value_enum, default_value_t = SpecFormat::Markdown)]
        format: SpecFormat,
    },
}

#[derive(Clone, ValueEnum)]
pub enum SpecFormat {
    Markdown,
    Json,
}

#[derive(Subcommand)]
pub enum ForgeCommands {
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

#[derive(Subcommand)]
pub enum AuraCommands {
    /// List all AURAs
    List,
    /// Show AURA details
    Show {
        /// AURA identifier
        name: String,
    },
    /// Create a new AURA
    Create {
        /// AURA name
        name: String,
    },
    /// Switch to a different AURA
    Switch {
        /// AURA name to activate
        name: String,
        /// Force switch even if already active
        #[arg(long)]
        force: bool,
    },
    /// Show currently active AURA
    Status,
}

#[derive(Subcommand)]
pub enum ConfigureCommands {
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
pub enum DbCommands {
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
pub enum RecordCommands {
    /// Start recording current tmux window
    Start {
        /// Optional output file path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Stop active recording(s)
    Stop {
        /// Recording ID to stop (all if omitted)
        id: Option<String>,
    },
    /// List all active recordings
    List,
    /// Manage tvmux server
    Server {
        #[command(subcommand)]
        action: ServerActions,
    },
    /// Configuration
    Config {
        #[command(subcommand)]
        action: ConfigActions,
    },
}

#[derive(Subcommand)]
pub enum ServerActions {
    /// Start the tvmux server
    Start,
    /// Stop the tvmux server
    Stop,
    /// Check server status
    Status,
}

#[derive(Subcommand)]
pub enum ConfigActions {
    /// Show tvmux configuration
    Show,
    /// Set configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
}

#[derive(Subcommand)]
pub enum AgentKernelCommands {
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
pub enum DaemonActions {
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
pub enum BackupCommands {
    /// Create a new backup of federation data
    Create {
        /// Backup mode: xdg (config + data + docs) or full (entire nabia)
        #[arg(short, long, default_value = "xdg")]
        mode: Option<String>,

        /// Dry run mode (show what would be done)
        #[arg(long)]
        dry_run: bool,

        /// Explicitly specified targets (comma-separated paths)
        #[arg(long)]
        targets: Option<String>,

        /// Backup to external drive if detected
        #[arg(long)]
        external: bool,
    },

    /// List available backups
    List {
        /// Output format: text or json
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Restore from a previous backup
    Restore {
        /// Backup ID to restore from
        backup_id: String,

        /// Target restoration directory (defaults to original location)
        #[arg(long)]
        target: Option<String>,

        /// Dry run mode (show what would be done)
        #[arg(long)]
        dry_run: bool,
    },

    /// Show/validate backup configuration
    Config {
        /// Validate configuration file
        #[arg(long)]
        validate: bool,
    },

    /// Monitor NATS backup queue (federation coordination)
    Queue {
        /// Queue action: status, list, retry
        #[arg(short, long)]
        action: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PortCommands {
    /// List all registered ports
    List {
        /// Platform filter (macos, wsl, rpi)
        #[arg(short, long)]
        platform: Option<String>,
    },
    /// Validate port allocations on current platform
    Check,
    /// Check cross-platform conflicts
    CrossPlatform,
    /// Safely migrate service to new port
    Shift {
        /// Service name
        service: String,
        /// Current port
        old_port: u16,
        /// New port
        new_port: u16,
        /// Dry run (don't execute)
        #[arg(long)]
        dry_run: bool,
    },
    /// Perform forensic analysis of drift period
    Drift {
        /// Enable forensic mode
        #[arg(long)]
        forensic: bool,
        /// Time since (e.g., "2 days ago")
        #[arg(long)]
        since: Option<String>,
    },
    /// Auto-generate fix commands for conflicts
    Fix,
    /// Generate .env file for docker-compose
    GenerateEnv,
}

#[derive(Subcommand)]
pub enum HooksCommands {
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
pub enum HookDebugActions {
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
pub enum DeckgenCommands {
    /// Emit the canonical seeded deckgen trace fixture
    Trace {
        /// Optional file path to write the trace JSON; stdout if omitted
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum RecoverCommands {
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
pub enum HealthCommands {
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

    /// [DEPRECATED] Use 'health substrate' instead
    /// Run federation substrate health substrates
    Check {
        /// Auto-remediate critical issues
        #[arg(long)]
        auto_remediate: bool,
        /// Only show FSM state changes (don't run checks)
        #[arg(long)]
        fsm_only: bool,
    },
    /// Show health substrate status and recent reports
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
    /// Start HTTP API server for remote monitoring
    Api {
        /// Port to run API server on
        #[arg(default_value = "8000")]
        port: u16,
        /// Host to bind to
        #[arg(default_value = "127.0.0.1")]
        host: String,
        /// Run in debug mode
        #[arg(long)]
        debug: bool,
    },
}

#[derive(Subcommand)]
pub enum MigrateCommands {
    /// Migrate directories from state to data
    ///
    /// Migrates specified directories from XDG_STATE_HOME to XDG_DATA_HOME.
    /// Supports dry-run mode, conflict detection, and selective migration.
    Run {
        /// Directory name in state to migrate (can be specified multiple times)
        #[arg(short, long, value_name = "DIR")]
        dir: Vec<String>,
        /// Dry run mode (preview changes without executing)
        #[arg(long)]
        dry_run: bool,
        /// Force overwrite on conflicts
        #[arg(short, long)]
        force: bool,
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Verify completed migrations
    ///
    /// Checks that migrations completed successfully by verifying:
    /// - Source directories are removed or empty
    /// - Destination directories exist and contain expected files
    Verify {
        /// Directory name to verify (can be specified multiple times)
        #[arg(short, long, value_name = "DIR")]
        dir: Vec<String>,
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
pub enum ToolCommands {
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
}

#[derive(Subcommand)]
pub enum RegisterCommands {
    /// Register a tool manifest (alias for `nabi tool register`)
    Tool(ToolRegisterArgs),
}

#[derive(Clone, Debug, Args)]
pub struct ToolRegisterArgs {
    /// Path to the tool executable, script, or module entry point
    #[arg(value_name = "PATH")]
    pub path: String,
    /// Human-friendly tool name (defaults to inferred name)
    #[arg(long)]
    pub name: Option<String>,
    /// CLI command name to expose (defaults to slug)
    #[arg(long)]
    pub command: Option<String>,
    /// Explicit runtime selection (auto-detected if omitted)
    #[arg(long, value_enum)]
    pub runtime: Option<RuntimeKind>,
    /// Runtime version hint (defaults to inferred baseline)
    #[arg(long)]
    pub runtime_version: Option<String>,
    /// Version string to record in manifest
    #[arg(long)]
    pub version: Option<String>,
    /// Description to include in manifest metadata
    #[arg(long)]
    pub description: Option<String>,
    /// Comma separated tags for discovery and coordination
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,
    /// Repository URL or reference source (optional)
    #[arg(long)]
    pub repository: Option<String>,
    /// Entry point identifier (module/function) if different from command
    #[arg(long)]
    pub entry_point: Option<String>,
    /// Execution string (e.g., "python -m foo") if custom
    #[arg(long)]
    pub execution: Option<String>,
    /// Preferred virtual environment location
    #[arg(long)]
    pub venv: Option<String>,
    /// Installer hint for provisioning the runtime environment
    #[arg(long)]
    pub installer: Option<String>,
    /// Override manifest schema version
    #[arg(long)]
    pub schema_version: Option<String>,
    /// Override manifest status (defaults to "active")
    #[arg(long)]
    pub status: Option<String>,
    /// Overwrite existing manifest file if it already exists
    #[arg(long)]
    pub force: bool,
    /// Mark capabilities: federation-aware tool
    #[arg(long)]
    pub federation_aware: bool,
    /// Mark capabilities: aura compatible
    #[arg(long)]
    pub aura_compatible: bool,
    /// Mark capabilities: XDG compliant
    #[arg(long)]
    pub xdg_compliant: bool,
    /// Mark capabilities: hook integrated
    #[arg(long)]
    pub hook_integrated: bool,
    /// Mark capabilities: cross-platform support
    #[arg(long)]
    pub cross_platform: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum RuntimeKind {
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
    pub fn as_language(self) -> &'static str {
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

    pub fn default_version_hint(self) -> &'static str {
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

    pub fn default_execution(self, path: &Path) -> String {
        use crate::utils::shell_quote;
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
