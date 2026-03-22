//! Ecko v2 -- deterministic code quality checks for AI agents.
//!
//! CLI entry point. Dispatches to runner functions based on `--mode`.

use clap::{Parser, ValueEnum};
use std::io::Read as _;
use std::process;

mod config;
mod debug;
mod echo;

// Stub modules -- Phase 1 declares them so later phases just fill them in.
mod checks;
mod external;
mod fingerprint;
mod fix;
mod formatter;
mod git;
mod guard;
mod lang;
mod ledger;
mod mcp;
mod query_engine;
mod runner;
mod suppress;

/// Ecko -- deterministic code quality checks for AI agents.
#[derive(Parser, Debug)]
#[command(name = "ecko", version, about)]
struct Cli {
    /// Run mode.
    #[arg(long, value_enum)]
    mode: Mode,

    /// File to check (PostToolUse / DryRun mode).
    #[arg(long)]
    file: Option<String>,

    /// Comma-separated file list (Stop mode override).
    #[arg(long)]
    files: Option<String>,

    /// Working directory.
    #[arg(long, default_value = ".")]
    cwd: String,

    /// Plugin root directory.
    #[arg(long, default_value = ".")]
    plugin_root: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Mode {
    /// PostToolUse -- per-file checks after Write/Edit.
    PostToolUse,
    /// Stop -- deep analysis across modified files.
    Stop,
    /// PreToolUseBash -- guard dangerous bash commands.
    PreToolUseBash,
    /// DryRun -- list applicable checks without running tools.
    DryRun,
    /// McpServer -- expose checks as an MCP server.
    McpServer,
    /// SessionStats -- print session ledger summary to stdout.
    SessionStats,
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.mode {
        Mode::PostToolUse => {
            let file = match cli.file {
                Some(f) => f,
                None => {
                    // Read from stdin (hook pipes JSON input).
                    let mut input = String::new();
                    if std::io::stdin().read_to_string(&mut input).is_err() {
                        process::exit(0);
                    }
                    match serde_json::from_str::<serde_json::Value>(&input) {
                        Ok(val) => val
                            .get("file_path")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        Err(_) => String::new(),
                    }
                }
            };
            if file.is_empty() {
                process::exit(0);
            }
            // Resolve relative path.
            let file = if !std::path::Path::new(&file).is_absolute() {
                format!("{}/{}", cli.cwd, file)
            } else {
                file
            };
            runner::run_post_tool_use(&file, &cli.cwd, &cli.plugin_root)
        }
        Mode::Stop => {
            let files_override: Option<Vec<String>> = cli.files.map(|f| {
                f.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            });
            runner::run_stop(&cli.cwd, &cli.plugin_root, files_override)
        }
        Mode::PreToolUseBash => {
            // Read tool input JSON from stdin to extract the "command" field.
            let mut input = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut input) {
                debug::debug(&format!("failed to read stdin: {e}"));
                process::exit(0);
            }

            let command = match serde_json::from_str::<serde_json::Value>(&input) {
                Ok(val) => val
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                Err(_) => {
                    // Fallback: treat raw stdin as the command itself.
                    input.trim().to_string()
                }
            };

            if command.is_empty() {
                0
            } else {
                guard::run_pre_tool_use_bash(&command, &cli.cwd)
            }
        }
        Mode::DryRun => {
            let file = cli.file.unwrap_or_else(|| {
                echo::emit("ecko: --file is required for dry-run mode");
                process::exit(1);
            });
            // Resolve relative path.
            let file = if !std::path::Path::new(&file).is_absolute() {
                format!("{}/{}", cli.cwd, file)
            } else {
                file
            };
            runner::run_dry_run(&file, &cli.cwd, &cli.plugin_root)
        }
        Mode::McpServer => {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("ecko: failed to start async runtime: {e}");
                    process::exit(1);
                }
            };
            rt.block_on(async {
                if let Err(e) = mcp::run_mcp_server().await {
                    eprintln!("ecko MCP server error: {e}");
                    process::exit(1);
                }
            });
            0
        }
        Mode::SessionStats => {
            let cfg = config::load_config(&cli.cwd);
            let session_hours = cfg.session_hours;
            let entries = ledger::read_session(&cli.cwd, session_hours);
            let corrections = ledger::compute_self_corrections(&entries);
            let json_values = ledger::entries_to_json_values(&entries);
            let summary = echo::format_session_stats(&json_values, &corrections);
            println!("{summary}");
            0
        }
    };

    process::exit(exit_code);
}
