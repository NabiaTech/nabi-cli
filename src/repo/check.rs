use super::validators::{ComplianceReport, Validator, Severity};
use super::validators::xdg::XdgValidator;
use super::validators::paths::PathValidator;
use super::validators::symlinks::SymlinkValidator;
use anyhow::Result;
use std::path::Path;
use std::collections::HashMap;
use colored::*;

pub fn check(repo_path: &str, format: &str, strict: bool) -> Result<()> {
    let path = Path::new(repo_path).canonicalize()
        .unwrap_or_else(|_| Path::new(repo_path).to_path_buf());

    println!("{}", format!("🔍 Scanning repository: {}", path.display()).cyan().bold());
    println!();

    // Run validators
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(XdgValidator),
        Box::new(PathValidator::new()),
        Box::new(SymlinkValidator::new()),
    ];

    let mut all_violations = Vec::new();

    for validator in validators {
        print!("  {} validator... ", validator.name().cyan());
        match validator.validate(&path) {
            Ok(violations) => {
                println!("{}", format!("✓ ({} issues)", violations.len()).dimmed());
                all_violations.extend(violations);
            }
            Err(e) => {
                println!("{}", format!("⚠ warning: {}", e).yellow());
            }
        }
    }

    // Count files scanned (simplified for MVP)
    let total_files = walkdir::WalkDir::new(&path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count();

    // Generate report
    let report = ComplianceReport {
        repo_path: path.clone(),
        violations: all_violations,
        total_files_scanned: total_files,
    };

    // Output based on format
    match format {
        "json" => output_json(&report)?,
        _ => output_text(&report)?,
    }

    // Exit code based on severity
    let exit_code = calculate_exit_code(&report, strict);
    std::process::exit(exit_code);
}

fn output_text(report: &ComplianceReport) -> Result<()> {
    println!();
    println!("{}", "═".repeat(70).cyan());
    println!("{}", "  Compliance Report".bold().cyan());
    println!("{}", "═".repeat(70).cyan());
    println!();
    println!("  Repository: {}", report.repo_path.display().to_string().white().bold());
    println!("  Files scanned: {}", report.total_files_scanned.to_string().white());
    println!("  Total violations: {}", report.violations.len().to_string().white());
    println!();

    if report.violations.is_empty() {
        println!("{}", "  ✓ All compliance checks passed!".green().bold());
        println!();
        return Ok(());
    }

    // Group by severity
    let mut by_severity: HashMap<_, Vec<_>> = HashMap::new();
    for v in &report.violations {
        by_severity.entry(&v.severity).or_default().push(v);
    }

    // Print violations by severity
    for severity in [Severity::Critical, Severity::Error, Severity::Warning, Severity::Info] {
        if let Some(violations) = by_severity.get(&severity) {
            let severity_str = match severity {
                Severity::Critical => "CRITICAL".red().bold(),
                Severity::Error => "ERROR".red(),
                Severity::Warning => "WARNING".yellow(),
                Severity::Info => "INFO".blue(),
            };

            println!("{} {} {}", "─".repeat(3).dimmed(), severity_str, "─".repeat(60).dimmed());
            println!();

            for v in violations {
                let file_display = v.file_path
                    .strip_prefix(&report.repo_path)
                    .unwrap_or(&v.file_path)
                    .display();

                let location = if let Some(line) = v.line_number {
                    format!("{}:{}", file_display, line).white()
                } else {
                    format!("{}", file_display).white()
                };

                println!("  {} {}", "▸".dimmed(), location);
                println!("    {} {}", format!("[{}]", v.rule_id).yellow(), v.message.dimmed());

                if let Some(suggestion) = &v.suggestion {
                    println!("    {} {}", "💡".dimmed(), suggestion.dimmed().italic());
                }
                println!();
            }
        }
    }

    println!("{}", "═".repeat(70).cyan());
    println!();

    Ok(())
}

fn output_json(report: &ComplianceReport) -> Result<()> {
    use serde_json::json;

    let violations_json: Vec<_> = report.violations.iter().map(|v| {
        json!({
            "file": v.file_path.display().to_string(),
            "line": v.line_number,
            "rule_id": v.rule_id,
            "severity": format!("{:?}", v.severity),
            "message": v.message,
            "suggestion": v.suggestion,
        })
    }).collect();

    let output = json!({
        "repo_path": report.repo_path.display().to_string(),
        "files_scanned": report.total_files_scanned,
        "violations_count": report.violations.len(),
        "violations": violations_json,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn calculate_exit_code(report: &ComplianceReport, strict: bool) -> i32 {
    if report.violations.is_empty() {
        return 0; // Success
    }

    let has_critical = report.violations.iter().any(|v| matches!(v.severity, Severity::Critical));
    let has_errors = report.violations.iter().any(|v| matches!(v.severity, Severity::Error));
    let has_warnings = report.violations.iter().any(|v| matches!(v.severity, Severity::Warning));

    if has_critical {
        3 // Critical violations
    } else if has_errors {
        2 // Errors found
    } else if has_warnings && strict {
        1 // Warnings in strict mode
    } else {
        0 // Success (warnings ignored in non-strict)
    }
}
