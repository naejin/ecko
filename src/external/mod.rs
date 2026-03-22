//! External tool adapters -- subprocess-based deep analysis tools.
//!
//! Each adapter runs an external binary, parses its output, and returns
//! `HashMap<String, Vec<Echo>>` (file path -> echoes). All adapters handle
//! missing binaries, timeouts, and parse errors gracefully -- returning empty
//! on any failure and emitting warnings via `crate::echo::emit()`.

pub mod clippy;
pub mod golangci;
pub mod pyright;
pub mod tsc;

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Run a command with a timeout. Returns `None` if the process times out,
/// cannot be spawned, or encounters a wait error. On timeout the child is killed.
///
/// Emits user-facing messages via `echo::emit()` to distinguish spawn failures
/// (tool not installed) from timeouts, so callers don't need to guess.
///
/// Spawns reader threads for stdout/stderr to prevent pipe buffer deadlocks
/// when subprocess output exceeds the OS pipe buffer (~64KB).
pub fn run_with_timeout(mut cmd: Command, timeout: Duration, tool_name: &str) -> Option<Output> {
    let mut child = match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => {
            crate::debug::debug(&format!("{tool_name} spawn failed: {e}"));
            crate::echo::emit(&format!("~~ ecko ~~ note: {tool_name} not found"));
            return None;
        }
    };

    let start = Instant::now();

    // Read output in separate threads to avoid pipe buffer deadlock.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut r) = stdout {
            use std::io::Read;
            let _ = r.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut r) = stderr {
            use std::io::Read;
            let _ = r.read_to_end(&mut buf);
        }
        buf
    });

    // Poll for exit with timeout.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout_data = stdout_handle.join().unwrap_or_default();
                let stderr_data = stderr_handle.join().unwrap_or_default();
                return Some(Output {
                    status,
                    stdout: stdout_data,
                    stderr: stderr_data,
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    // Join reader threads so they don't leak (pipe closes unblock them)
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    let secs = timeout.as_secs();
                    crate::debug::debug(&format!("{tool_name} timed out ({secs}s)"));
                    crate::echo::emit(&format!(
                        "~~ ecko ~~ warning: {tool_name} timed out ({secs}s limit)"
                    ));
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                crate::debug::debug(&format!("{tool_name} wait error: {e}"));
                return None;
            }
        }
    }
}
