/// Microkernel monitoring commands (kernel + agent memory overview)
use anyhow::{Context, Result};
use clap::{Subcommand, ValueEnum};
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

pub fn handle_kernel_commands(cmd: KernelCommands) -> Result<()> {
    match cmd {
        KernelCommands::Mem {
            format,
            max_agents,
            estimate_per_pane_mb,
        } => handle_kernel_mem(format, max_agents, estimate_per_pane_mb),
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
