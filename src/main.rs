use anyhow::{Context, Result};
use clap::{Arg, Args, Command, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell as CompletionShell};
use colored::*;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;
use chrono::Utc;
use std::fmt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod forge;
mod paths;
mod repo;
mod commands;
use paths::NabiPaths;
use commands::port;
use commands::tmux;
use commands::list;

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
    /// Filesystem scanning and metadata generation
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
    },
    /// File watching and real-time classification
    Watch {
        /// Path to watch
        #[arg(value_name = "PATH")]
        path: Option<String>,
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
    /// Runtime introspection and discovery (sessions, windows, panes, agents)
    ///
    /// Query the runtime for current state to enable:
    /// - Dynamic shell completion (nabi list sessions powers tmux completion)
    /// - System introspection (what agents/sessions/workers are active?)
    /// - Integration with external tools (kubectl, docker, etc.)
    ///
    /// All output is plain text (one item per line) for piping to shell completion systems.
    List {
        #[command(subcommand)]
        command: list::ListCommands,
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
    /// Generate shell completion scripts
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: CompletionShell,
    },
    /// Health check (alias for 'self doctor')
    #[command(visible_alias = "doc")]
    Doctor,
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
}

#[derive(Subcommand)]
enum DocsCommands {
    /// Manifest management (SHA256 tracking)
    Manifest {
        #[command(subcommand)]
        action: ManifestActions,
    },
}

#[derive(Subcommand)]
enum ManifestActions {
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
enum RepoCommands {
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
    Analyze {
        /// Path to repository to analyze
        #[arg(value_name = "PATH")]
        repo_path: String,

        /// Language hint (auto-detect if not provided: rust, python, go, typescript)
        #[arg(short, long)]
        lang: Option<String>,

        /// Force re-indexing (skip cache check)
        #[arg(short, long)]
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

#[derive(Subcommand)]
enum AuraCommands {
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
}

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
enum ServerActions {
    /// Start the tvmux server
    Start,
    /// Stop the tvmux server
    Stop,
    /// Check server status
    Status,
}

#[derive(Subcommand)]
enum ConfigActions {
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
enum HooksCommands {
    /// Transform hooks from schema to derived state (or use stable hooks)
    Transform {
        /// Use stable hooks from ~/.nabi/src/hooks instead of transforming
        #[arg(short, long)]
        stable: bool,
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
enum ToolCommands {
    /// Register a tool manifest for federation routing
    Register(ToolRegisterArgs),
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
    /// Overwrite existing manifest file if it already exists
    #[arg(long)]
    force: bool,
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
        Commands::Tool { command } => handle_tool(command),
        Commands::Register { command } => handle_register(command),
        Commands::Scan { path, tags, confidence } => handle_scan(path, tags, confidence),
        Commands::Watch { path } => handle_watch(path),
        Commands::Aura { command } => handle_aura(command),
        Commands::Configure { command } => handle_configure(command),
        Commands::Db { command } => handle_db(command),
        Commands::Record { command } => handle_record(command),
        Commands::Agent { command } => handle_agent(command),
        Commands::Port { command } => handle_port(command),
        Commands::List { command } => list::handle_list_commands(command),
        Commands::Tmux { command } => tmux::handle_tmux_commands(command),
        Commands::Hooks { command } => handle_hooks(command),
        Commands::Mode { mode } => handle_mode(mode),
        Commands::Riff { args } => handle_riff(args),
        Commands::Recover { command } => handle_recover(command),
        Commands::Completions { shell } => handle_completions(shell),
        Commands::Doctor => handle_self(SelfCommands::Doctor),
    }
}

fn handle_claude(command: ClaudeCommands) -> Result<()> {
    match command {
        ClaudeCommands::Session { action } => match action {
            SessionActions::List { limit } => {
                println!("{}", "📋 Listing Claude sessions...".cyan().bold());
                route_to_commander("claude", &["session", "list", "--limit", &limit.to_string()])
            }
            SessionActions::Recover { uuid } => {
                println!("{}", format!("🔄 Recovering session {}...", uuid).cyan().bold());
                route_to_commander("claude", &["session", "recover", &uuid])
            }
            SessionActions::View { uuid } => {
                println!("{}", format!("👁  Viewing session {}...", uuid).cyan().bold());
                route_to_commander("claude", &["session", "view", &uuid])
            }
        },
        ClaudeCommands::Project { action } => match action {
            ProjectActions::List => {
                println!("{}", "📂 Listing Claude projects...".cyan().bold());
                route_to_commander("claude", &["project", "list"])
            }
            ProjectActions::Migrate { path } => {
                println!("{}", format!("📦 Migrating project to {}...", path).cyan().bold());
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
                println!("{}", format!("🔧 Repairing {} -> {}...", file, output_file).yellow().bold());
                route_to_commander("data", &["jsonl", "repair", &file, "--output", &output_file])
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
                println!("{}", format!("🚀 Spawning {} agent...", role).magenta().bold());
                route_to_commander("federation", &["agent", "spawn", &role])
            }
        },
        FederationCommands::Sync { action } => match action {
            SyncActions::List => {
                println!("{}", "📂 Listing Syncthing folders...".cyan().bold());
                route_to_commander("federation", &["sync", "list"])
            }
            SyncActions::Pause { folder } => {
                println!("{}", format!("⏸️  Pausing folder {}...", folder).yellow().bold());
                route_to_commander("federation", &["sync", "pause", &folder])
            }
            SyncActions::Resume { folder } => {
                println!("{}", format!("▶️  Resuming folder {}...", folder).green().bold());
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
                println!("{}", format!("➕ Adding {} ({})...", name, service_type).green().bold());
                route_to_commander("federation", &["registry", "add", &name, "--type", &service_type])
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
                println!("{}", format!("🔍 Validating manifest for {}...", repo_path).cyan().bold());
                route_to_commander("docs", &["manifest", "validate", &repo_path])
            }
            ManifestActions::Generate { repo_path } => {
                println!("{}", format!("✨ Generating manifest for {}...", repo_path).cyan().bold());
                route_to_commander("docs", &["manifest", "generate", &repo_path])
            }
        },
    }
}

fn handle_repo(command: RepoCommands) -> Result<()> {
    match command {
        RepoCommands::Check { path, format, strict } => {
            let repo_path = path.unwrap_or_else(|| ".".to_string());
            repo::check(&repo_path, &format, strict)
        }
        RepoCommands::Analyze { repo_path, lang, force, format } => {
            repo::analyze(&repo_path, lang.as_deref(), force, &format)
        }
        RepoCommands::Graph { action } => {
            handle_graph(action)
        }
    }
}

fn handle_tool(command: ToolCommands) -> Result<()> {
    match command {
        ToolCommands::Register(args) => register_tool(args),
    }
}

fn handle_register(command: RegisterCommands) -> Result<()> {
    match command {
        RegisterCommands::Tool(args) => register_tool(args),
    }
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
    let tool_id = if slug.is_empty() { "tool".to_string() } else { slug };

    let command_name = args
        .command
        .clone()
        .unwrap_or_else(|| tool_id.clone());

    let runtime = args
        .runtime
        .or_else(|| infer_runtime(&canonical_path))
        .unwrap_or(RuntimeKind::Other);

    if args.runtime.is_none() && matches!(runtime, RuntimeKind::Other) {
        println!(
            "{}",
            "⚠️  Could not infer runtime automatically; recorded as 'other'."
                .yellow()
        );
    }

    let runtime_version_hint = args
        .runtime_version
        .clone()
        .unwrap_or_else(|| runtime.default_version_hint().to_string());

    let tool_version = args
        .version
        .clone()
        .unwrap_or_else(|| "0.1.0".to_string());

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
        let venv_path = NabiPaths::venv_dir()?
            .join(tool_id.replace('-', "_"));
        Some(path_to_tilde(&venv_path)?)
    } else {
        None
    };

    // Validate and setup dependencies
    let (validated_deps, dep_messages) = validate_tool_dependencies(
        &source_path,
        &venv_location,
        runtime
    )?;

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

    let status = args
        .status
        .clone()
        .unwrap_or_else(|| "active".to_string());

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

    let rendered = toml::to_string_pretty(&manifest)
        .context("Failed to serialize tool manifest to TOML")?;

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
    fs::create_dir_all(&tools_dir)
        .with_context(|| format!("Failed to create tools directory at {}", tools_dir.display()))?;

    let manifest_path = tools_dir.join(format!("{}.toml", tool_id));

    if manifest_path.exists() && !args.force {
        anyhow::bail!(
            "Manifest already exists at {} (use --force to overwrite)",
            manifest_path.display()
        );
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
    let mut possible_locations = vec![
        Some(source_path.join("requirements.txt")),
    ];

    // Add TOML path if source file name is available
    if let Some(file_name) = source_path.file_name() {
        if let Some(parent) = source_path.parent().and_then(|p| p.parent()) {
            possible_locations.push(Some(parent.join("tools")
                .join(file_name)
                .with_extension("toml")));
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
                messages.push(format!("📦 Found {} dependencies in {}",
                    dependencies.len(),
                    toml_path.display()));
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
                        messages.push(format!("📦 Found {} dependencies in {}",
                            dependencies.len(),
                            toml_path.display()));
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
            let status = process::Command::new("uv")
                .args(&["venv", venv_path.to_str().unwrap()])
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
            messages.push(format!("📥 Installing {} dependencies...", dependencies.len()));

            // Create temporary requirements file
            let temp_req = std::env::temp_dir().join("nabi_temp_requirements.txt");
            fs::write(&temp_req, dependencies.join("\n"))?;

            let status = process::Command::new("uv")
                .args(&[
                    "pip", "install",
                    "-r", temp_req.to_str().unwrap(),
                    "--python", python_bin.to_str().unwrap()
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
    let needs_quotes = raw
        .chars()
        .any(|c| matches!(c, ' ' | '"' | '\'' | '(' | ')' | '$' | '`' | '!' | '&' | ';' | '<' | '>' | '|'));
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
        let read = file.read(&mut buffer)
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
        GraphActions::Search { symbol, repo, format } => {
            let repo_path = repo.unwrap_or_else(|| ".".to_string());
            repo::graph_search(&repo_path, &symbol, &format)
        }
        GraphActions::References { symbol, repo, format } => {
            let repo_path = repo.unwrap_or_else(|| ".".to_string());
            repo::graph_references(&repo_path, &symbol, &format)
        }
        GraphActions::Related { symbol, repo, depth, format } => {
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

fn handle_completions(shell: CompletionShell) -> Result<()> {
    let mut command = Cli::command();
    generate(shell, &mut command, "nabi", &mut io::stdout());
    Ok(())
}

fn route_to_commander(commander: &str, args: &[&str]) -> Result<()> {
    // Use XDG Base Directory spec across all platforms
    let nabi_config = NabiPaths::config_dir()?;

    let commander_path = nabi_config
        .join("commanders")
        .join(commander);

    // Check if a native Rust commander binary exists
    let commander_binary = commander_path.join(commander);
    if commander_binary.exists() {
        println!(
            "{}",
            format!("→ Route to {} commander (native)", commander)
                .dimmed()
        );

        let mut cmd = process::Command::new(&commander_binary);
        cmd.args(args);
        let status = cmd.status()
            .context(format!("Failed to execute commander at {}", commander_binary.display()))?;

        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        return Ok(());
    }

    // Fallback: Route to Python CLI for commands not yet migrated to Rust
    // This enables gradual migration: Python → Rust
    println!(
        "{}",
        format!("→ Route to Python CLI: {}", commander)
            .dimmed()
    );

    let bin_dir = NabiPaths::bin_dir()?;
    let python_cli = bin_dir.join("nabi-python");

    if python_cli.exists() {
        let mut cmd = process::Command::new(&python_cli);
        cmd.arg(commander);
        cmd.args(args);
        let status = cmd.status()
            .context(format!("Failed to execute Python CLI at {}", python_cli.display()))?;

        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    } else {
        eprintln!(
            "{}",
            format!("❌ Commander '{}' not found and no Python CLI fallback available", commander)
                .red()
                .bold()
        );
        eprintln!("{}", format!("Expected Python CLI: {}", python_cli.display()).yellow());
        eprintln!("{}", "Run 'nabi self doctor' to diagnose issues.".yellow());
        process::exit(1);
    }
}

fn check_commander(commander: &str) -> Result<()> {
    let nabi_config = NabiPaths::config_dir()?;

    let commander_path = nabi_config
        .join("commanders")
        .join(commander);

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
            format!("Broken .venv in config directory: {}", config_venv.display()),
            format!("rm -rf {}", config_venv_path.display())
        ));
    }

    if nabi_venv.exists() {
        let nabi_venv_path = NabiPaths::config_dir()?.join(".nabi").join(".venv");
        violations.push((
            format!("Broken .venv in nested config: {}", nabi_venv.display()),
            format!("rm -rf {}", nabi_venv_path.display())
        ));
    }

    // Check 2: Verify venv directory exists and is properly structured
    let venv_base = NabiPaths::venv_dir()?;

    if !venv_base.exists() {
        violations.push((
            format!("Venv directory missing: {}", venv_base.display()),
            format!("mkdir -p {}", venv_base.display())
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
                        if line.contains("/Users/") || (line.contains("/home/") && !line.contains("${")) {
                            // Allow comments and specific patterns
                            if !line.trim().starts_with("#") {
                                violations.push((
                                    format!("Hardcoded path in {}: line {}", path.display(), line_num + 1),
                                    "Replace absolute paths with ~ or ${XDG_*} variables".to_string()
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
        ForgeCommands::Enable { feature } => {
            forge::handle_enable(feature)
        }
        ForgeCommands::Disable { feature } => {
            forge::handle_disable(feature)
        }
        ForgeCommands::Status => {
            forge::handle_status()
        }
        ForgeCommands::List => {
            forge::handle_list()
        }
    }
}

fn handle_scan(path: Option<String>, tags: Option<String>, confidence: Option<f32>) -> Result<()> {
    println!("{}", "🔍 Scanning filesystem...".cyan().bold());
    let mut args = vec!["scan".to_string()];
    if let Some(p) = path {
        args.push(p);
    }
    if let Some(t) = tags {
        args.push("--tags".to_string());
        args.push(t);
    }
    if let Some(c) = confidence {
        args.push("--confidence".to_string());
        args.push(c.to_string());
    }

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    route_to_python_cli(&arg_refs)
}

fn handle_watch(path: Option<String>) -> Result<()> {
    println!("{}", "👁  Watching filesystem...".cyan().bold());
    let mut args = vec!["watch".to_string()];
    if let Some(p) = path {
        args.push(p);
    }

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    route_to_python_cli(&arg_refs)
}

fn handle_aura(command: AuraCommands) -> Result<()> {
    match command {
        AuraCommands::List => {
            println!("{}", "📋 Listing AURAs...".cyan().bold());
            route_to_python_cli(&["aura", "list"])
        }
        AuraCommands::Show { name } => {
            println!("{}", format!("👁  Viewing AURA: {}...", name).cyan().bold());
            route_to_python_cli(&["aura", "show", &name])
        }
        AuraCommands::Create { name } => {
            println!("{}", format!("✨ Creating AURA: {}...", name).cyan().bold());
            route_to_python_cli(&["aura", "create", &name])
        }
    }
}

fn handle_configure(command: ConfigureCommands) -> Result<()> {
    match command {
        ConfigureCommands::Show => {
            println!("{}", "⚙️  Configuration".cyan().bold());
            route_to_python_cli(&["configure", "show"])
        }
        ConfigureCommands::Set { key, value } => {
            println!("{}", format!("✏️  Setting {} = {}...", key, value).cyan().bold());
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
            println!("{}", format!("💾 Exporting database to {}...", path).blue().bold());
            route_to_python_cli(&["db", "export", &path])
        }
        DbCommands::Import { path } => {
            println!("{}", format!("📥 Importing database from {}...", path).blue().bold());
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
                println!("{}", format!("✏️  Setting {} = {}...", key, value).cyan().bold());
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
    daemon_script: String,
    version: String,
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
        eprintln!("{}", format!("Expected at: {}", config_path.display()).yellow());
        anyhow::bail!("Missing kernel.json configuration");
    }

    let json_content = std::fs::read_to_string(&config_path)
        .context("Failed to read kernel.json")?;

    let config: KernelConfig = serde_json::from_str(&json_content)
        .context("Failed to parse kernel.json")?;

    Ok(config)
}

fn handle_agent(command: AgentKernelCommands) -> Result<()> {
    // Load kernel configuration (decoupled from venv name)
    let kernel_cfg = load_kernel_config()?;

    // Resolve Python executable from configured venv
    let kernel_venv = expand_home(&kernel_cfg.kernel_venv)?;
    let python_exe = kernel_venv.join("bin").join("python3");

    if !python_exe.exists() {
        eprintln!("{}", "❌ Python executable not found in kernel venv".red().bold());
        eprintln!("{}", format!("Expected at: {}", python_exe.display()).yellow());
        let kernel_config_path = NabiPaths::config_dir()?.join("commanders/kernel.json");
        eprintln!("{}", format!("   Configured in: {}", kernel_config_path.display()).yellow());
        eprintln!("{}", "   Try: nabi self doctor".yellow());
        process::exit(1);
    }

    // Get agent commander path
    let nabi_config = NabiPaths::config_dir()?
        .join("commanders")
        .join("agent");

    match command {
        AgentKernelCommands::Daemon { action } => {
            let daemon_script = nabi_config.join("daemon");
            if !daemon_script.exists() {
                eprintln!("{}", "❌ Agent daemon script not found".red().bold());
                eprintln!("{}", format!("Expected at: {}", daemon_script.display()).yellow());
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
                    let status = cmd.status()
                        .context(format!("Failed to execute daemon script at {}", daemon_script.display()))?;
                    if !status.success() {
                        process::exit(status.code().unwrap_or(1));
                    }
                    Ok(())
                }
                DaemonActions::Stop => {
                    println!("{}", "⏹️  Stopping NABIKernel daemon...".yellow().bold());
                    let mut cmd = process::Command::new(&python_exe);
                    cmd.arg(&daemon_script).arg("stop");
                    let status = cmd.status()
                        .context(format!("Failed to execute daemon script at {}", daemon_script.display()))?;
                    if !status.success() {
                        process::exit(status.code().unwrap_or(1));
                    }
                    Ok(())
                }
                DaemonActions::Restart => {
                    println!("{}", "🔄 Restarting NABIKernel daemon...".cyan().bold());
                    let mut cmd = process::Command::new(&python_exe);
                    cmd.arg(&daemon_script).arg("restart");
                    let status = cmd.status()
                        .context(format!("Failed to execute daemon script at {}", daemon_script.display()))?;
                    if !status.success() {
                        process::exit(status.code().unwrap_or(1));
                    }
                    Ok(())
                }
                DaemonActions::Status => {
                    println!("{}", "📊 Checking NABIKernel daemon status...".cyan().bold());
                    let mut cmd = process::Command::new(&python_exe);
                    cmd.arg(&daemon_script).arg("status");
                    let status = cmd.status()
                        .context(format!("Failed to execute daemon script at {}", daemon_script.display()))?;
                    if !status.success() {
                        process::exit(status.code().unwrap_or(1));
                    }
                    Ok(())
                }
            }
        }
        AgentKernelCommands::Spawn { agent_type, task, priority } => {
            println!("{}", format!("🤖 Spawning {} agent...", agent_type).magenta().bold());
            let spawn_script = nabi_config.join("spawn");
            let mut cmd = process::Command::new(&python_exe);
            cmd.arg(&spawn_script);
            cmd.arg(&agent_type);
            cmd.arg("--priority").arg(&priority);
            if let Some(t) = task {
                cmd.arg("--task").arg(&t);
            }
            let status = cmd.status()
                .context(format!("Failed to execute spawn script at {}", spawn_script.display()))?;
            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
            Ok(())
        }
        AgentKernelCommands::Status { agent_id } => {
            println!("{}", format!("📊 Checking status for {}...", agent_id).cyan().bold());
            let status_script = nabi_config.join("status");
            let mut cmd = process::Command::new(&python_exe);
            cmd.arg(&status_script).arg(&agent_id);
            let status = cmd.status()
                .context(format!("Failed to execute status script at {}", status_script.display()))?;
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
            let status = cmd.status()
                .context(format!("Failed to execute list script at {}", list_script.display()))?;
            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
            Ok(())
        }
        AgentKernelCommands::Kill { agent_id } => {
            println!("{}", format!("⚔️  Killing agent {}...", agent_id).red().bold());
            let kill_script = nabi_config.join("kill");
            let mut cmd = process::Command::new(&python_exe);
            cmd.arg(&kill_script).arg(&agent_id);
            let status = cmd.status()
                .context(format!("Failed to execute kill script at {}", kill_script.display()))?;
            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
            Ok(())
        }
        AgentKernelCommands::Wait { agent_id } => {
            println!("{}", format!("⏳ Waiting for agent {}...", agent_id).yellow().bold());
            let wait_script = nabi_config.join("wait");
            let mut cmd = process::Command::new(&python_exe);
            cmd.arg(&wait_script).arg(&agent_id);
            let status = cmd.status()
                .context(format!("Failed to execute wait script at {}", wait_script.display()))?;
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
            println!("{}", "🌐 Checking cross-platform conflicts...".cyan().bold());
            port::cmd_cross_platform()
        }
        PortCommands::Shift { service, old_port, new_port, dry_run } => {
            println!("{}", format!("🔄 Migrating {} from {} to {}...", service, old_port, new_port).yellow().bold());
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

fn handle_hooks(command: HooksCommands) -> Result<()> {
    match command {
        HooksCommands::Transform { stable } => {
            if stable {
                println!("{}", "🔗 Using stable hooks from ~/.nabi/src/hooks...".cyan().bold());

                // Use stable hooks: copy from ~/.nabi/src/hooks/src/ to deployment location
                let home = dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
                let stable_hooks_src = home.join(".nabi/src/hooks/src");
                let hooks_deploy = NabiPaths::data_dir()?.join("bin").join("hooks");

                // Ensure deployment directory exists
                fs::create_dir_all(&hooks_deploy)
                    .context(format!("Failed to create hooks directory at {}", hooks_deploy.display()))?;

                if !stable_hooks_src.exists() {
                    eprintln!(
                        "{}",
                        format!("❌ Stable hooks not found at: {}", stable_hooks_src.display())
                            .red()
                            .bold()
                    );
                    eprintln!(
                        "{}",
                        "Expected stable hooks at ~/.nabi/src/hooks/src/".yellow()
                    );
                    process::exit(1);
                }

                // Copy hook files from stable location
                let mut copied = 0;
                if let Ok(entries) = fs::read_dir(&stable_hooks_src) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |e| e == "py") && path.is_file() {
                            let filename = path.file_name().unwrap();
                            let dest = hooks_deploy.join(filename);
                            fs::copy(&path, &dest)
                                .context(format!("Failed to copy {} to {}", path.display(), dest.display()))?;
                            copied += 1;
                        }
                    }
                }

                println!(
                    "{}",
                    format!("✓ Copied {} hook files to {}", copied, hooks_deploy.display()).green()
                );
                Ok(())
            } else {
                println!("{}", "🔄 Transforming hooks from schema to derived state...".cyan().bold());

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
                    .ok_or_else(|| anyhow::anyhow!("Python not found. Set NABI_PYTHON or ensure python3/python is in PATH"))?;

                // Find and execute all transform_*.py scripts
                let mut executed = 0;
                if let Ok(entries) = fs::read_dir(&transform_scripts_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.file_name()
                                .and_then(|n| n.to_str())
                                .map_or(false, |n| n.starts_with("transform_") && n.ends_with(".py"))
                        {
                            println!(
                                "{}",
                                format!("  Running {}...", path.file_name().unwrap().to_string_lossy())
                                    .dimmed()
                            );

                            let status = process::Command::new(&python_exe)
                                .arg(&path)
                                .status()
                                .context(format!("Failed to execute transform script: {}", path.display()))?;

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
                        format!("⚠️  No transformation scripts found at: {}", transform_scripts_dir.display())
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

    let status = cmd.status()
        .context(format!("Failed to execute bash CLI at {}", bash_cli.display()))?;

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn handle_riff(args: Vec<String>) -> Result<()> {
    // Layer 1 → Layer 2 handoff for riff-cli
    // Route all riff commands to Python CLI layer
    println!(
        "{}",
        "🔍 Routing to riff-cli...".cyan().bold()
    );

    let mut python_args = vec!["riff"];
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    python_args.extend_from_slice(&arg_refs);

    route_to_python_cli(&python_args)
}

fn handle_recover(command: RecoverCommands) -> Result<()> {
    match command {
        RecoverCommands::Sessions { hours, detailed, export } => {
            println!(
                "{}",
                format!("🔄 Recovering sessions from last {} hours...", hours).cyan().bold()
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

/// Route commands to Python CLI for filesystem operations
fn route_to_python_cli(args: &[&str]) -> Result<()> {
    let bin_dir = NabiPaths::bin_dir()?;
    let python_cli = bin_dir.join("nabi-python");

    if python_cli.exists() {
        let mut cmd = process::Command::new(&python_cli);
        cmd.args(args);
        let status = cmd.status()
            .context(format!("Failed to execute Python CLI at {}", python_cli.display()))?;

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
