/// Microkernel monitoring and daemon control commands
use anyhow::{Context, Result};
use clap::{Subcommand, ValueEnum};
use colored::*;
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::process::Command;

use crate::paths::NabiPaths;

#[derive(Subcommand)]
pub enum KernelCommands {
    /// Summarize microkernel + agent memory usage and compare to pane baseline
    Mem {
        /// Output format (text, json)
        #[arg(long, value_enum)]
        format: Option<KernelMemOutputFormat>,

        /// Maximum agent rows to display (text output only)
        #[arg(long)]
        max_agents: Option<usize>,

        /// Override estimated MB per isolated pane when computing efficiency
        #[arg(long)]
        estimate_per_pane_mb: Option<f64>,
    },

    /// Daemon control (start, stop, restart, status)
    Daemon {
        #[command(subcommand)]
        action: DaemonActions,
    },

    /// Check NABIKernel daemon health and service status
    Health {
        /// Show detailed service information
        #[arg(long)]
        detailed: bool,

        /// Output format (text or json)
        #[arg(long, value_enum)]
        format: Option<HealthOutputFormat>,
    },

    /// Check NABIKernel daemon status
    Status {
        /// Show detailed diagnostic information
        #[arg(long)]
        detailed: bool,
    },
}

#[derive(Subcommand)]
pub enum DaemonActions {
    /// Start the NABIKernel daemon
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the NABIKernel daemon
    Stop,
    /// Restart the NABIKernel daemon
    Restart,
    /// Check daemon status
    Status,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum KernelMemOutputFormat {
    #[value(alias = "text")]
    Text,
    #[value(alias = "json")]
    Json,
}

impl Default for KernelMemOutputFormat {
    fn default() -> Self {
        KernelMemOutputFormat::Text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum HealthOutputFormat {
    #[value(alias = "text")]
    Text,
    #[value(alias = "json")]
    Json,
}

impl Default for HealthOutputFormat {
    fn default() -> Self {
        HealthOutputFormat::Text
    }
}

pub fn handle_kernel_commands(cmd: KernelCommands) -> Result<()> {
    match cmd {
        KernelCommands::Mem {
            format,
            max_agents,
            estimate_per_pane_mb,
        } => handle_kernel_mem(format, max_agents, estimate_per_pane_mb),
        KernelCommands::Daemon { action } => handle_kernel_daemon(action),
        KernelCommands::Health { detailed, format } => handle_kernel_health(detailed, format),
        KernelCommands::Status { detailed } => handle_kernel_status(detailed),
    }
}

fn handle_kernel_mem(
    format: Option<KernelMemOutputFormat>,
    max_agents_override: Option<usize>,
    estimate_per_pane_override: Option<f64>,
) -> Result<()> {
    let config = KernelMemConfig::load();
    let resolved_format = format
        .or(config.output.default_format)
        .unwrap_or(KernelMemOutputFormat::Text);

    let mut detection = config.detection.clone();
    detection.normalize();

    let processes = collect_processes().context("Failed to read process table via ps")?;

    let kernel_records = build_kernel_records(&processes, &detection);
    let kernel_total_kb: u64 = kernel_records.iter().map(|r| r.memory_kb).sum();

    let agent_records = build_agent_records(&processes, &detection);
    let agent_total_kb: u64 = agent_records.iter().map(|r| r.memory_kb).sum();
    let agent_count = agent_records.len();

    let pane_count = count_tmux_panes();
    let estimate_per_pane = estimate_per_pane_override
        .unwrap_or(config.defaults.estimate_per_pane_mb)
        .max(0.0);

    let architecture = ArchitectureSummary::from_totals(
        pane_count,
        estimate_per_pane,
        kernel_total_kb,
        agent_total_kb,
    );

    let report = KernelMemReport {
        header: config.defaults.header_title.clone(),
        kernel_processes: kernel_records,
        kernel_total_kb,
        agents: agent_records,
        agent_total_kb,
        agent_count,
        architecture,
    };

    match resolved_format {
        KernelMemOutputFormat::Text => {
            let max_agents = max_agents_override.unwrap_or(config.defaults.max_agent_rows);
            print_text_report(&report, max_agents, config.output.show_legend);
        }
        KernelMemOutputFormat::Json => print_json_report(&report)?,
    }

    Ok(())
}

fn print_text_report(report: &KernelMemReport, max_agent_rows: usize, show_legend: bool) {
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("{}", report.header);
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!();

    println!("📍 MICROKERNEL CORE PROCESSES");
    println!("────────────────────────────────────────────────────────────────────────────");
    if report.kernel_processes.is_empty() {
        println!("❌ No memchain kernel processes found");
    } else {
        println!(
            "{:<10} {:<8} {:<12} {:<8} {:<12} {}",
            "TYPE", "PID", "MEMORY", "GB", "UPTIME", "NAME"
        );
        println!("────────────────────────────────────────────────────────────────────────");
        for record in &report.kernel_processes {
            println!(
                "{:<10} {:<8} {:<12} {:<8} {:<12} {}",
                record.process_type,
                record.pid,
                format!("{:.1} MB", record.memory_mb),
                format!("{:.2}", record.memory_gb),
                record.uptime,
                record.command
            );
        }
        println!();
        println!(
            "KERNEL TOTAL: {:.1} MB ({:.2} GB)",
            report.kernel_total_mb(),
            report.kernel_total_gb()
        );
    }

    println!();
    println!("🤖 AGENT PROCESSES (Connected to Kernel)");
    println!("────────────────────────────────────────────────────────────────────────────");
    if report.agents.is_empty() {
        println!("⚠️  No active agents detected");
    } else {
        println!(
            "{:<12} {:<8} {:<12} {:<8} {:<12} {}",
            "TYPE", "PID", "MEMORY", "GB", "UPTIME", "NAME"
        );
        println!("────────────────────────────────────────────────────────────────────────");
        let rows_to_show = if max_agent_rows == 0 {
            report.agents.len()
        } else {
            max_agent_rows
        };
        for record in report.agents.iter().take(rows_to_show) {
            println!(
                "{:<12} {:<8} {:<12} {:<8} {:<12} {}",
                record.agent_type,
                record.pid,
                format!("{:.1} MB", record.memory_mb),
                format!("{:.2}", record.memory_gb),
                record.uptime,
                record.command
            );
        }
        println!();
        println!(
            "AGENTS TOTAL ({} agents): {:.1} MB ({:.2} GB)",
            report.agent_count,
            report.agent_total_mb(),
            report.agent_total_gb()
        );
    }

    println!();
    println!("📊 ARCHITECTURE COMPARISON");
    println!("────────────────────────────────────────────────────────────────────────────");
    if let Some(count) = report.architecture.pane_count {
        println!("ISOLATED PANES ARCHITECTURE (Current Baseline):");
        println!("  • Panes: {}", count);
        println!(
            "  • Est. Memory per pane: ~{:.0} MB",
            report.architecture.estimate_per_pane_mb
        );
        println!(
            "  • Est. Total Memory: ~{:.0} MB",
            report.architecture.estimated_total_mb
        );
    } else {
        println!("ISOLATED PANES ARCHITECTURE (Current Baseline):");
        println!("  • tmux not detected (unable to count panes)");
        println!(
            "  • Est. Memory per pane: ~{:.0} MB",
            report.architecture.estimate_per_pane_mb
        );
    }
    println!();
    println!("MICROKERNEL FEDERATION ARCHITECTURE (Evolving):");
    println!("  • Kernel processes: {}", report.kernel_processes.len());
    println!("  • Agents: {}", report.agent_count);
    println!(
        "  • Total Memory: {:.0} MB",
        report.architecture.microkernel_total_mb
    );
    println!();
    println!("EFFICIENCY GAIN:");
    if let Some(ratio) = report.architecture.efficiency_ratio {
        println!(
            "  • {:.0} MB ÷ {:.0} MB = {:.1}×",
            report.architecture.estimated_total_mb, report.architecture.microkernel_total_mb, ratio
        );
        if let Some(saved) = report.architecture.saved_mb {
            println!("  • Memory saved: {:.0} MB", saved);
        }
    } else {
        println!("  • Microkernel not active or baseline unavailable");
    }

    println!();
    if show_legend {
        println!("═══════════════════════════════════════════════════════════════════════════════");
        println!("Legend:");
        println!("  • Kernel processes: Core memchain runtime");
        println!("  • Agents: Orchestrator workers (Igris, Beru, etc)");
        println!("  • Efficiency: (Isolated Total) ÷ (Microkernel Total)");
        println!("═══════════════════════════════════════════════════════════════════════════════");
    }
}

fn print_json_report(report: &KernelMemReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report)
        .context("Failed to serialize kernel memory report to JSON")?;
    println!("{json}");
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct KernelMemReport {
    header: String,
    kernel_processes: Vec<KernelProcessRecord>,
    kernel_total_kb: u64,
    agents: Vec<AgentProcessRecord>,
    agent_total_kb: u64,
    agent_count: usize,
    architecture: ArchitectureSummary,
}

impl KernelMemReport {
    fn kernel_total_mb(&self) -> f64 {
        kb_to_mb(self.kernel_total_kb)
    }

    fn kernel_total_gb(&self) -> f64 {
        kb_to_gb(self.kernel_total_kb)
    }

    fn agent_total_mb(&self) -> f64 {
        kb_to_mb(self.agent_total_kb)
    }

    fn agent_total_gb(&self) -> f64 {
        kb_to_gb(self.agent_total_kb)
    }
}

#[derive(Debug, Clone, Serialize)]
struct KernelProcessRecord {
    pid: i32,
    process_type: String,
    command: String,
    args: String,
    uptime: String,
    memory_kb: u64,
    memory_mb: f64,
    memory_gb: f64,
}

#[derive(Debug, Clone, Serialize)]
struct AgentProcessRecord {
    pid: i32,
    agent_type: String,
    command: String,
    args: String,
    uptime: String,
    memory_kb: u64,
    memory_mb: f64,
    memory_gb: f64,
}

#[derive(Debug, Clone, Serialize)]
struct ArchitectureSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pane_count: Option<usize>,
    estimate_per_pane_mb: f64,
    estimated_total_mb: f64,
    microkernel_total_mb: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    efficiency_ratio: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    saved_mb: Option<f64>,
}

impl ArchitectureSummary {
    fn from_totals(
        pane_count: Option<usize>,
        estimate_per_pane_mb: f64,
        kernel_total_kb: u64,
        agent_total_kb: u64,
    ) -> Self {
        let estimated_total_mb = pane_count
            .map(|count| (count as f64) * estimate_per_pane_mb)
            .unwrap_or(0.0);
        let microkernel_total_mb = kb_to_mb(kernel_total_kb + agent_total_kb);

        let efficiency_ratio = if microkernel_total_mb > 0.0 && estimated_total_mb > 0.0 {
            Some(estimated_total_mb / microkernel_total_mb)
        } else {
            None
        };
        let saved_mb = efficiency_ratio.map(|_| estimated_total_mb - microkernel_total_mb);

        ArchitectureSummary {
            pane_count,
            estimate_per_pane_mb,
            estimated_total_mb,
            microkernel_total_mb,
            efficiency_ratio,
            saved_mb,
        }
    }
}

#[derive(Debug, Clone)]
struct ProcessInfo {
    pid: i32,
    rss_kb: u64,
    command: String,
    command_lower: String,
    args: String,
    args_lower: String,
    uptime: String,
}

#[derive(Debug, Clone, Deserialize)]
struct KernelMemConfig {
    #[serde(default)]
    defaults: KernelMemDefaults,
    #[serde(default)]
    detection: KernelMemDetection,
    #[serde(default)]
    output: KernelMemOutput,
}

impl Default for KernelMemConfig {
    fn default() -> Self {
        KernelMemConfig {
            defaults: KernelMemDefaults::default(),
            detection: KernelMemDetection::default(),
            output: KernelMemOutput::default(),
        }
    }
}

impl KernelMemConfig {
    fn load() -> Self {
        let path = NabiPaths::config_dir()
            .map(|dir| dir.join("kernel-mem").join("config.toml"))
            .ok();

        if let Some(cfg_path) = path {
            if cfg_path.exists() {
                if let Ok(contents) = fs::read_to_string(&cfg_path) {
                    if let Ok(cfg) = toml::from_str::<KernelMemConfig>(&contents) {
                        return cfg;
                    } else {
                        eprintln!(
                            "⚠️  Failed to parse {} (falling back to defaults)",
                            cfg_path.display()
                        );
                    }
                } else {
                    eprintln!(
                        "⚠️  Unable to read {} (falling back to defaults)",
                        cfg_path.display()
                    );
                }
            }
        }

        KernelMemConfig::default()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct KernelMemDefaults {
    #[serde(default = "default_header_title")]
    header_title: String,
    #[serde(default = "default_estimate_per_pane_mb")]
    estimate_per_pane_mb: f64,
    #[serde(default = "default_max_agent_rows")]
    max_agent_rows: usize,
}

impl Default for KernelMemDefaults {
    fn default() -> Self {
        KernelMemDefaults {
            header_title: default_header_title(),
            estimate_per_pane_mb: default_estimate_per_pane_mb(),
            max_agent_rows: default_max_agent_rows(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct KernelMemDetection {
    #[serde(default = "default_kernel_patterns")]
    kernel_patterns: Vec<String>,
    #[serde(default = "default_kernel_api_patterns")]
    kernel_api_patterns: Vec<String>,
    #[serde(default = "default_agent_patterns")]
    agent_patterns: Vec<AgentPattern>,
    #[serde(default = "default_ignore_process_names")]
    ignore_process_names: Vec<String>,
}

impl Default for KernelMemDetection {
    fn default() -> Self {
        KernelMemDetection {
            kernel_patterns: default_kernel_patterns(),
            kernel_api_patterns: default_kernel_api_patterns(),
            agent_patterns: default_agent_patterns(),
            ignore_process_names: default_ignore_process_names(),
        }
    }
}

impl KernelMemDetection {
    fn normalize(&mut self) {
        self.kernel_patterns = self
            .kernel_patterns
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        self.kernel_api_patterns = self
            .kernel_api_patterns
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        for pattern in &mut self.agent_patterns {
            pattern.pattern = pattern.pattern.to_ascii_lowercase();
        }
        self.ignore_process_names = self
            .ignore_process_names
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
    }

    fn is_kernel(&self, info: &ProcessInfo) -> bool {
        if self.kernel_patterns.is_empty() {
            return false;
        }
        self.kernel_patterns
            .iter()
            .all(|pattern| info.args_lower.contains(pattern))
    }

    fn kernel_type(&self, info: &ProcessInfo) -> String {
        if self
            .kernel_api_patterns
            .iter()
            .any(|pattern| info.args_lower.contains(pattern))
        {
            "kernel_api".to_string()
        } else {
            "kernel".to_string()
        }
    }

    fn agent_label(&self, info: &ProcessInfo) -> Option<String> {
        if self
            .ignore_process_names
            .iter()
            .any(|pattern| info.command_lower == *pattern)
        {
            return None;
        }
        for pattern in &self.agent_patterns {
            if info.command_lower.contains(&pattern.pattern)
                || info.args_lower.contains(&pattern.pattern)
            {
                return Some(pattern.label.clone());
            }
        }
        None
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AgentPattern {
    label: String,
    pattern: String,
}

#[derive(Debug, Clone, Deserialize)]
struct KernelMemOutput {
    #[serde(default)]
    default_format: Option<KernelMemOutputFormat>,
    #[serde(default = "default_true")]
    show_legend: bool,
}

impl Default for KernelMemOutput {
    fn default() -> Self {
        KernelMemOutput {
            default_format: None,
            show_legend: true,
        }
    }
}

fn build_kernel_records(
    processes: &[ProcessInfo],
    detection: &KernelMemDetection,
) -> Vec<KernelProcessRecord> {
    let mut records = Vec::new();
    for info in processes {
        if detection.is_kernel(info) {
            records.push(KernelProcessRecord {
                pid: info.pid,
                process_type: detection.kernel_type(info),
                command: info.command.clone(),
                args: info.args.clone(),
                uptime: info.uptime.clone(),
                memory_kb: info.rss_kb,
                memory_mb: kb_to_mb(info.rss_kb),
                memory_gb: kb_to_gb(info.rss_kb),
            });
        }
    }
    records.sort_by(|a, b| b.memory_kb.cmp(&a.memory_kb));
    records
}

fn build_agent_records(
    processes: &[ProcessInfo],
    detection: &KernelMemDetection,
) -> Vec<AgentProcessRecord> {
    let mut records = Vec::new();
    for info in processes {
        if let Some(label) = detection.agent_label(info) {
            records.push(AgentProcessRecord {
                pid: info.pid,
                agent_type: label,
                command: info.command.clone(),
                args: info.args.clone(),
                uptime: info.uptime.clone(),
                memory_kb: info.rss_kb,
                memory_mb: kb_to_mb(info.rss_kb),
                memory_gb: kb_to_gb(info.rss_kb),
            });
        }
    }
    records.sort_by(|a, b| b.memory_kb.cmp(&a.memory_kb));
    records
}

fn collect_processes() -> Result<Vec<ProcessInfo>> {
    let output = Command::new("ps")
        .args(&["-axo", "pid=,rss=,comm=,etime=,args="])
        .output()
        .context("ps command failed")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "ps command failed: {}",
            stderr.trim().to_string()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut processes = Vec::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(pid_str) = parts.next() else {
            continue;
        };
        let Some(rss_str) = parts.next() else {
            continue;
        };
        let Some(command) = parts.next() else {
            continue;
        };
        let Some(uptime) = parts.next() else { continue };

        let pid = match pid_str.parse::<i32>() {
            Ok(val) => val,
            Err(_) => continue,
        };
        let rss_kb = match rss_str.parse::<u64>() {
            Ok(val) => val,
            Err(_) => 0,
        };

        let args = parts.collect::<Vec<&str>>().join(" ");
        let command_lower = command.to_ascii_lowercase();
        let args_lower = args.to_ascii_lowercase();

        processes.push(ProcessInfo {
            pid,
            rss_kb,
            command: command.to_string(),
            command_lower,
            args,
            args_lower,
            uptime: uptime.to_string(),
        });
    }

    Ok(processes)
}

fn count_tmux_panes() -> Option<usize> {
    let output = Command::new("tmux")
        .args(&["list-panes", "-a", "-F", "#{pane_id}"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let count = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    Some(count)
}

fn kb_to_mb(kb: u64) -> f64 {
    kb as f64 / 1024.0
}

fn kb_to_gb(kb: u64) -> f64 {
    kb as f64 / (1024.0 * 1024.0)
}

fn default_header_title() -> String {
    "MICROKERNEL FEDERATION ARCHITECTURE MONITORING".to_string()
}

fn default_estimate_per_pane_mb() -> f64 {
    350.0
}

fn default_max_agent_rows() -> usize {
    20
}

fn default_kernel_patterns() -> Vec<String> {
    vec!["memchain".to_string(), "kernel".to_string()]
}

fn default_kernel_api_patterns() -> Vec<String> {
    vec!["kernel_api".to_string()]
}

fn default_agent_patterns() -> Vec<AgentPattern> {
    vec![
        AgentPattern {
            label: "vigil".to_string(),
            pattern: "vigil".to_string(),
        },
        AgentPattern {
            label: "beru".to_string(),
            pattern: "beru".to_string(),
        },
        AgentPattern {
            label: "igris".to_string(),
            pattern: "igris".to_string(),
        },
        AgentPattern {
            label: "worker/orchestrator".to_string(),
            pattern: "worker".to_string(),
        },
        AgentPattern {
            label: "worker/orchestrator".to_string(),
            pattern: "orchestrator".to_string(),
        },
        AgentPattern {
            label: "coordinator".to_string(),
            pattern: "coordinator".to_string(),
        },
        AgentPattern {
            label: "agent".to_string(),
            pattern: "agent".to_string(),
        },
    ]
}

fn default_ignore_process_names() -> Vec<String> {
    vec!["tmux".to_string(), "grep".to_string(), "sh".to_string()]
}

fn default_true() -> bool {
    true
}

// ============================================================================
// NOTIFICATION HELPERS
// ============================================================================

#[derive(Debug, Clone)]
struct NotificationConfig {
    enabled: bool,
    min_level: NotificationLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum NotificationLevel {
    Info,
    Warning,
    Critical,
}

fn send_notification(event: &str, title: &str, message: &str, level: NotificationLevel) {
    // Try to read config, but don't fail if not available
    let config = load_notification_config().unwrap_or(NotificationConfig {
        enabled: true,
        min_level: NotificationLevel::Warning,
    });

    if !config.enabled || level < config.min_level {
        return;
    }

    // Use osascript for macOS notifications
    let sound = match level {
        NotificationLevel::Critical => "Basso",
        NotificationLevel::Warning => "Ping",
        NotificationLevel::Info => "Glass",
    };

    let script = format!(
        r#"display notification "{}" with title "{}" sound name "{}""#,
        message, title, sound
    );

    let _ = Command::new("/usr/bin/osascript")
        .args(&["-e", &script])
        .output();
}

fn load_notification_config() -> Result<NotificationConfig> {
    let config_path = NabiPaths::config_dir()
        .context("Failed to resolve config directory")?
        .join("kernel.toml");

    if !config_path.exists() {
        return Ok(NotificationConfig {
            enabled: true,
            min_level: NotificationLevel::Warning,
        });
    }

    let contents = fs::read_to_string(&config_path).context("Failed to read kernel.toml")?;

    // Parse TOML to extract notification settings
    let parsed: toml::Value = toml::from_str(&contents).context("Failed to parse kernel.toml")?;

    let enabled = parsed
        .get("kernel")
        .and_then(|k| k.get("notifications"))
        .and_then(|n| n.get("enabled"))
        .and_then(|e| e.as_bool())
        .unwrap_or(true);

    let min_level_str = parsed
        .get("kernel")
        .and_then(|k| k.get("notifications"))
        .and_then(|n| n.get("min_level"))
        .and_then(|l| l.as_str())
        .unwrap_or("warning");

    let min_level = match min_level_str {
        "info" => NotificationLevel::Info,
        "warning" => NotificationLevel::Warning,
        "critical" => NotificationLevel::Critical,
        _ => NotificationLevel::Warning,
    };

    Ok(NotificationConfig { enabled, min_level })
}

// ============================================================================
// DAEMON CONTROL HANDLERS
// ============================================================================

fn handle_kernel_daemon(action: DaemonActions) -> Result<()> {
    match action {
        DaemonActions::Start { foreground } => daemon_start(foreground),
        DaemonActions::Stop => daemon_stop(),
        DaemonActions::Restart => daemon_restart(),
        DaemonActions::Status => handle_kernel_status(false),
    }
}

fn daemon_start(foreground: bool) -> Result<()> {
    println!(
        "{}",
        "🚀 Starting NABIKernel daemon...".bright_cyan().bold()
    );
    println!();

    // Pre-flight check: port conflict detection
    if !check_port_available(5380)? {
        send_notification(
            "port_conflict",
            "❌ NABIKernel Port Conflict",
            "Port 5380 is already in use. Run 'nabi kernel status' for details.",
            NotificationLevel::Critical,
        );
        return Err(anyhow::anyhow!(
            "Cannot start NABIKernel - port 5380 is already in use"
        ));
    }

    let daemon_script = NabiPaths::config_dir()
        .context("Failed to resolve config directory")?
        .join("commanders")
        .join("agent")
        .join("daemon");

    if !daemon_script.exists() {
        return Err(anyhow::anyhow!(
            "Daemon script not found at {}",
            daemon_script.display()
        ));
    }

    let mut cmd = Command::new(&daemon_script);
    cmd.arg("start");

    if foreground {
        cmd.arg("--foreground");
    }

    let output = cmd.output().context("Failed to execute daemon script")?;

    if output.status.success() {
        println!("{}", "✅ NABIKernel daemon started successfully".green());
        println!();
        println!("   Health endpoint: http://localhost:5380/health");
        println!("   Check status: nabi kernel status");

        send_notification(
            "startup_success",
            "✅ NABIKernel Started",
            "Daemon running on port 5380",
            NotificationLevel::Info,
        );
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", "❌ Failed to start daemon:".red().bold());
        eprintln!("{}", stderr);

        send_notification(
            "startup_failure",
            "❌ NABIKernel Startup Failed",
            "Check logs: tail ~/.local/state/nabi/federation/nabikernel.error.log",
            NotificationLevel::Critical,
        );

        return Err(anyhow::anyhow!("Daemon start failed"));
    }

    Ok(())
}

fn daemon_stop() -> Result<()> {
    println!(
        "{}",
        "🛑 Stopping NABIKernel daemon...".bright_yellow().bold()
    );
    println!();

    let daemon_script = NabiPaths::config_dir()
        .context("Failed to resolve config directory")?
        .join("commanders")
        .join("agent")
        .join("daemon");

    if !daemon_script.exists() {
        return Err(anyhow::anyhow!(
            "Daemon script not found at {}",
            daemon_script.display()
        ));
    }

    let output = Command::new(&daemon_script)
        .arg("stop")
        .output()
        .context("Failed to execute daemon script")?;

    if output.status.success() {
        println!("{}", "✅ NABIKernel daemon stopped".green());

        send_notification(
            "shutdown_success",
            "🛑 NABIKernel Stopped",
            "Daemon shutdown complete",
            NotificationLevel::Info,
        );
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", "❌ Failed to stop daemon:".red().bold());
        eprintln!("{}", stderr);

        send_notification(
            "shutdown_failure",
            "⚠️ NABIKernel Shutdown Issue",
            "Shutdown may have been incomplete. Check 'nabi kernel status'",
            NotificationLevel::Warning,
        );

        return Err(anyhow::anyhow!("Daemon stop failed"));
    }

    Ok(())
}

fn daemon_restart() -> Result<()> {
    println!(
        "{}",
        "🔄 Restarting NABIKernel daemon...".bright_magenta().bold()
    );
    println!();

    daemon_stop()?;
    std::thread::sleep(std::time::Duration::from_secs(2));

    match daemon_start(false) {
        Ok(_) => {
            send_notification(
                "restart_success",
                "🔄 NABIKernel Restarted",
                "Daemon restart complete",
                NotificationLevel::Info,
            );
            Ok(())
        }
        Err(e) => {
            send_notification(
                "restart_failure",
                "❌ NABIKernel Restart Failed",
                "Restart failed. Check status with 'nabi kernel status --detailed'",
                NotificationLevel::Critical,
            );
            Err(e)
        }
    }
}

fn check_port_available(port: u16) -> Result<bool> {
    let output = Command::new("lsof")
        .args(&["-i", &format!(":{}", port)])
        .output()
        .context("Failed to execute lsof")?;

    if output.status.success() && !output.stdout.is_empty() {
        // Port is in use
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!(
            "{}",
            format!("❌ Port {} is already in use", port).red().bold()
        );
        eprintln!();
        eprintln!("{}", stdout);

        // Try to extract PID for helpful error message
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                let pid = parts[1];
                eprintln!("{}", format!("💡 To fix: kill {}", pid).yellow());
                eprintln!("   Or run: nabi kernel daemon stop");
                break;
            }
        }
        eprintln!();

        return Ok(false);
    }

    Ok(true)
}

// ============================================================================
// STATUS HANDLERS
// ============================================================================

fn handle_kernel_status(detailed: bool) -> Result<()> {
    println!("{}", "📊 NABIKernel Daemon Status".bright_cyan().bold());
    println!("{}", "━".repeat(60).bright_black());
    println!();

    let mut all_ok = true;

    // 1. Check LaunchAgent registration
    let launchctl_output = Command::new("launchctl")
        .args(&["list", "io.nabia.nabikernel"])
        .output();

    match launchctl_output {
        Ok(output) if output.status.success() => {
            let info = String::from_utf8_lossy(&output.stdout);
            println!("{}", "✅ LaunchAgent: Registered".green());

            if detailed {
                // Parse PID and LastExitStatus
                for line in info.lines() {
                    if line.contains("\"PID\"") {
                        println!("   {}", line.trim().bright_black());
                    } else if line.contains("\"LastExitStatus\"") {
                        println!("   {}", line.trim().bright_black());
                    }
                }
            }
        }
        _ => {
            println!("{}", "❌ LaunchAgent: Not registered".red());
            all_ok = false;
            println!();
            println!("   Run: nabi kernel daemon start");
            return Ok(());
        }
    }

    // 2. Check port
    let port_check = Command::new("lsof").args(&["-i", ":5380"]).output();

    match port_check {
        Ok(output) if output.status.success() && !output.stdout.is_empty() => {
            println!("{}", "✅ Port 5380: Listening".green());
            if detailed {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines().take(2) {
                    println!("   {}", line.bright_black());
                }
            }
        }
        _ => {
            println!(
                "{}",
                "⚠️  Port 5380: Not listening (daemon may be starting)".yellow()
            );
            all_ok = false;
        }
    }

    // 3. Check health endpoint
    match check_health_endpoint("http://localhost:5380/health") {
        Ok(Some(json)) => {
            println!("{}", "✅ Health Endpoint: Responding".green());
            if detailed {
                println!("   {}", serde_json::to_string_pretty(&json)?.bright_black());
            }
        }
        Ok(None) => {
            println!("{}", "❌ Health Endpoint: Not responding".red());
            all_ok = false;
            println!();
            println!("   Daemon may be in crash loop - check logs:");
            println!("   tail ~/.local/state/nabi/federation/nabikernel.error.log");
        }
        Err(e) => {
            println!("{}", format!("❌ Health Endpoint: Error ({})", e).red());
            all_ok = false;
        }
    }

    println!();
    if all_ok {
        println!("{}", "🎉 All checks passed!".green().bold());
    } else {
        println!("{}", "⚠️  Some checks failed".yellow().bold());
    }

    Ok(())
}

fn handle_kernel_health(detailed: bool, format: Option<HealthOutputFormat>) -> Result<()> {
    let format = format.unwrap_or_default();

    let mut health_status = HealthStatus {
        core_services: Vec::new(),
        docker_services: Vec::new(),
        summary: HealthSummary {
            healthy: 0,
            degraded: 0,
            down: 0,
        },
    };

    // Check kernel daemon
    health_status.core_services.push(check_service_health(
        "NABIKernel API",
        "http://localhost:5380/health",
    ));

    // Check key federation services
    health_status.core_services.push(check_service_health(
        "SurrealDB",
        "http://localhost:8284/health",
    ));
    health_status
        .core_services
        .push(check_service_health("Loki", "http://localhost:3100/ready"));
    health_status.core_services.push(check_service_health(
        "Grafana",
        "http://localhost:3002/api/health",
    ));
    health_status.core_services.push(check_service_health(
        "Vigil",
        "http://localhost:8100/health",
    ));

    // Calculate summary
    for service in &health_status.core_services {
        match service.status {
            ServiceStatus::Healthy => health_status.summary.healthy += 1,
            ServiceStatus::Degraded => health_status.summary.degraded += 1,
            ServiceStatus::Down => health_status.summary.down += 1,
        }
    }

    // Output
    match format {
        HealthOutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&health_status)?);
        }
        HealthOutputFormat::Text => {
            print_health_dashboard(&health_status, detailed);
        }
    }

    Ok(())
}

fn print_health_dashboard(status: &HealthStatus, _detailed: bool) {
    println!(
        "{}",
        "NABIKernel Federation Health Dashboard"
            .bright_cyan()
            .bold()
    );
    println!("{}", "━".repeat(60).bright_black());
    println!();

    println!("{}", "Core Services:".bright_white().bold());
    for service in &status.core_services {
        let status_str = match service.status {
            ServiceStatus::Healthy => "✅ Healthy".green(),
            ServiceStatus::Degraded => "⚠️  Degraded".yellow(),
            ServiceStatus::Down => "❌ Down".red(),
        };

        println!(
            "  {} {:20} {:30} {}",
            status_str,
            service.name,
            service.endpoint.bright_black(),
            service.message.bright_black()
        );
    }

    println!();
    println!("{}", "Summary:".bright_white().bold());
    println!("  {} {} healthy", "✅".green(), status.summary.healthy);
    println!("  {} {} degraded", "⚠️ ".yellow(), status.summary.degraded);
    println!("  {} {} down", "❌".red(), status.summary.down);
}

fn check_service_health(name: &str, endpoint: &str) -> ServiceHealth {
    match check_health_endpoint(endpoint) {
        Ok(Some(_)) => ServiceHealth {
            name: name.to_string(),
            endpoint: endpoint.to_string(),
            status: ServiceStatus::Healthy,
            message: String::new(),
        },
        Ok(None) => ServiceHealth {
            name: name.to_string(),
            endpoint: endpoint.to_string(),
            status: ServiceStatus::Down,
            message: "Connection refused".to_string(),
        },
        Err(e) => ServiceHealth {
            name: name.to_string(),
            endpoint: endpoint.to_string(),
            status: ServiceStatus::Degraded,
            message: format!("{}", e),
        },
    }
}

fn check_health_endpoint(url: &str) -> Result<Option<serde_json::Value>> {
    // Use curl for HTTP requests to avoid adding reqwest dependency
    let output = Command::new("curl")
        .args(&["-s", "-f", "--max-time", "2", url])
        .output()
        .context("Failed to execute curl")?;

    if output.status.success() {
        let body = String::from_utf8_lossy(&output.stdout);
        if let Ok(json) = serde_json::from_str(&body) {
            Ok(Some(json))
        } else {
            Ok(Some(serde_json::json!({"status": "ok"})))
        }
    } else {
        Ok(None)
    }
}

// ============================================================================
// HEALTH DATA STRUCTURES
// ============================================================================

#[derive(Debug, Serialize)]
struct HealthStatus {
    core_services: Vec<ServiceHealth>,
    docker_services: Vec<ServiceHealth>,
    summary: HealthSummary,
}

#[derive(Debug, Serialize)]
struct ServiceHealth {
    name: String,
    endpoint: String,
    status: ServiceStatus,
    message: String,
}

#[derive(Debug, Serialize)]
enum ServiceStatus {
    Healthy,
    Degraded,
    Down,
}

#[derive(Debug, Serialize)]
struct HealthSummary {
    healthy: usize,
    degraded: usize,
    down: usize,
}
