//! SLURM job failure analysis.
//!
//! Query detailed failure information and provide actionable suggestions.

use charmer_parsers::{
    MemoryFormat, format_duration, format_duration_slurm, parse_duration_secs, parse_memory_mb,
    run_command_allow_failure,
};
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum FailureError {
    #[error("Failed to execute sacct: {0}")]
    ExecutionError(String),
    #[error("Job not found: {0}")]
    NotFound(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Failure mode classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureMode {
    /// Job ran out of memory
    OutOfMemory {
        used_mb: u64,
        requested_mb: u64,
        suggested_mb: u64,
    },
    /// Job exceeded time limit
    Timeout {
        elapsed_seconds: u64,
        limit_seconds: u64,
        suggested_seconds: u64,
    },
    /// Job failed with non-zero exit code
    ExitCode { code: i32, signal: Option<i32> },
    /// Job was cancelled by user or admin
    Cancelled { by_user: Option<String> },
    /// Node failure
    NodeFailure { node: Option<String> },
    /// Unknown failure mode
    Unknown { state: String },
}

/// Detailed failure analysis result.
#[derive(Debug, Clone)]
pub struct FailureAnalysis {
    /// SLURM job ID
    pub job_id: String,
    /// Classified failure mode
    pub mode: FailureMode,
    /// Human-readable explanation
    pub explanation: String,
    /// Suggested fix
    pub suggestion: String,
    /// Raw SLURM state string
    pub raw_state: String,
    /// Actual memory used (MB)
    pub max_rss_mb: Option<u64>,
    /// Requested memory (MB)
    pub req_mem_mb: Option<u64>,
    /// Actual runtime (seconds)
    pub elapsed_seconds: Option<u64>,
    /// Time limit (seconds)
    pub time_limit_seconds: Option<u64>,
}

impl FailureAnalysis {
    /// Generate explanation and suggestion based on failure mode.
    fn generate_messages(mode: &FailureMode) -> (String, String) {
        match mode {
            FailureMode::OutOfMemory {
                used_mb,
                requested_mb,
                suggested_mb,
            } => {
                let explanation = format!(
                    "Job exceeded memory limit. Used {:.1} GB but only {:.1} GB was allocated.",
                    *used_mb as f64 / 1024.0,
                    *requested_mb as f64 / 1024.0
                );
                let suggestion = format!(
                    "Increase memory to at least {:.1} GB. In your Snakefile, add:\n  resources: mem_mb={}",
                    *suggested_mb as f64 / 1024.0,
                    suggested_mb
                );
                (explanation, suggestion)
            }
            FailureMode::Timeout {
                elapsed_seconds,
                limit_seconds,
                suggested_seconds,
            } => {
                let explanation = format!(
                    "Job exceeded time limit. Ran for {} but limit was {}.",
                    format_duration(*elapsed_seconds),
                    format_duration(*limit_seconds)
                );
                let suggestion = format!(
                    "Increase time limit to at least {}. In your Snakefile, add:\n  resources: runtime=\"{}\"",
                    format_duration(*suggested_seconds),
                    format_duration_slurm(*suggested_seconds)
                );
                (explanation, suggestion)
            }
            FailureMode::ExitCode { code, signal } => {
                let explanation = if let Some(sig) = signal {
                    match sig {
                        9 => format!(
                            "Job killed with signal {} (SIGKILL). Exit code: {}",
                            sig, code
                        ),
                        11 => format!(
                            "Job crashed with signal {} (SIGSEGV - segmentation fault). Exit code: {}",
                            sig, code
                        ),
                        15 => format!(
                            "Job terminated with signal {} (SIGTERM). Exit code: {}",
                            sig, code
                        ),
                        _ => format!("Job exited with code {} and signal {}", code, sig),
                    }
                } else {
                    match code {
                        1 => "Job failed with exit code 1 (general error)".to_string(),
                        2 => "Job failed with exit code 2 (misuse of shell command)".to_string(),
                        126 => "Job failed with exit code 126 (command not executable)".to_string(),
                        127 => "Job failed with exit code 127 (command not found)".to_string(),
                        137 => "Job killed (likely OOM killer). Exit code 137 = 128 + 9 (SIGKILL)"
                            .to_string(),
                        _ => format!("Job failed with exit code {}", code),
                    }
                };
                let suggestion = if *code == 137 {
                    "This is likely an out-of-memory error. Try increasing memory allocation."
                        .to_string()
                } else {
                    "Check the job's stderr log for error details.".to_string()
                };
                (explanation, suggestion)
            }
            FailureMode::Cancelled { by_user } => {
                let explanation = if let Some(user) = by_user {
                    format!("Job was cancelled by {}", user)
                } else {
                    "Job was cancelled".to_string()
                };
                (
                    "Consider if this was intentional or due to dependency failure.".to_string(),
                    explanation,
                )
            }
            FailureMode::NodeFailure { node } => {
                let explanation = if let Some(n) = node {
                    format!("Job failed due to node {} failure", n)
                } else {
                    "Job failed due to node failure".to_string()
                };
                (
                    "Re-run the job. If persistent, contact cluster admin.".to_string(),
                    explanation,
                )
            }
            FailureMode::Unknown { state } => (
                format!("Job failed with unknown state: {}", state),
                "Check SLURM logs for details.".to_string(),
            ),
        }
    }
}

/// Query detailed failure information for a SLURM job.
pub async fn analyze_failure(job_id: &str) -> Result<FailureAnalysis, FailureError> {
    // Query sacct with detailed memory and time info
    // Format: State, ExitCode, MaxRSS, ReqMem, Elapsed, Timelimit, NodeList
    let mut cmd = Command::new("sacct");
    cmd.args([
        "-j",
        job_id,
        "-X",
        "--parsable2",
        "--noheader",
        "--format",
        "State,ExitCode,MaxRSS,ReqMem,Elapsed,Timelimit,NodeList",
    ]);

    let stdout = run_command_allow_failure(&mut cmd, "sacct")
        .await
        .map_err(|e| FailureError::ExecutionError(e.to_string()))?;

    let line = stdout
        .lines()
        .next()
        .ok_or_else(|| FailureError::NotFound(job_id.to_string()))?;

    parse_failure_line(job_id, line)
}

/// Parse sacct output line for failure analysis.
fn parse_failure_line(job_id: &str, line: &str) -> Result<FailureAnalysis, FailureError> {
    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() < 7 {
        return Err(FailureError::ParseError(format!(
            "Expected 7 fields, got {}: {}",
            fields.len(),
            line
        )));
    }

    let raw_state = fields[0].to_string();
    let exit_code_str = fields[1];
    let max_rss_str = fields[2];
    let req_mem_str = fields[3];
    let elapsed_str = fields[4];
    let time_limit_str = fields[5];
    let node = if fields[6].is_empty() || fields[6] == "None" {
        None
    } else {
        Some(fields[6].to_string())
    };

    // Parse exit code (format: "exit_code:signal")
    let (exit_code, signal) = parse_exit_code_signal(exit_code_str);

    // Parse memory values
    let max_rss_mb = parse_memory_mb(max_rss_str, MemoryFormat::SlurmSacct);
    let req_mem_mb = parse_memory_mb(req_mem_str, MemoryFormat::SlurmSacct);

    // Parse time values
    let elapsed_seconds = parse_duration_secs(elapsed_str);
    let time_limit_seconds = parse_duration_secs(time_limit_str);

    // Determine failure mode
    let base_state = raw_state.split_whitespace().next().unwrap_or(&raw_state);
    let mode = match base_state.to_uppercase().as_str() {
        "OUT_OF_MEMORY" => {
            let used = max_rss_mb.unwrap_or(0);
            let requested = req_mem_mb.unwrap_or(0);
            // Suggest 50% more than used, rounded up to nearest GB
            let suggested = ((used as f64 * 1.5) / 1024.0).ceil() as u64 * 1024;
            FailureMode::OutOfMemory {
                used_mb: used,
                requested_mb: requested,
                suggested_mb: suggested.max(requested + 1024),
            }
        }
        "TIMEOUT" => {
            let elapsed = elapsed_seconds.unwrap_or(0);
            let limit = time_limit_seconds.unwrap_or(0);
            // Suggest 50% more time
            let suggested = (elapsed as f64 * 1.5) as u64;
            FailureMode::Timeout {
                elapsed_seconds: elapsed,
                limit_seconds: limit,
                suggested_seconds: suggested.max(limit + 3600),
            }
        }
        "CANCELLED" => {
            // Check if cancelled by someone
            let by_user = if raw_state.contains("by ") {
                raw_state.split("by ").nth(1).map(|s| s.trim().to_string())
            } else {
                None
            };
            FailureMode::Cancelled { by_user }
        }
        "NODE_FAIL" => FailureMode::NodeFailure { node },
        "FAILED" | "BOOT_FAIL" | "DEADLINE" => {
            // Check for common exit codes that indicate OOM
            if exit_code == 137 || (signal == Some(9) && max_rss_mb.is_some()) {
                let used = max_rss_mb.unwrap_or(0);
                let requested = req_mem_mb.unwrap_or(0);
                let suggested = ((used as f64 * 1.5) / 1024.0).ceil() as u64 * 1024;
                FailureMode::OutOfMemory {
                    used_mb: used,
                    requested_mb: requested,
                    suggested_mb: suggested.max(requested + 1024),
                }
            } else {
                FailureMode::ExitCode {
                    code: exit_code,
                    signal,
                }
            }
        }
        other => FailureMode::Unknown {
            state: other.to_string(),
        },
    };

    let (explanation, suggestion) = FailureAnalysis::generate_messages(&mode);

    Ok(FailureAnalysis {
        job_id: job_id.to_string(),
        mode,
        explanation,
        suggestion,
        raw_state,
        max_rss_mb,
        req_mem_mb,
        elapsed_seconds,
        time_limit_seconds,
    })
}

/// Parse exit code string "code:signal" into (code, signal).
fn parse_exit_code_signal(s: &str) -> (i32, Option<i32>) {
    let parts: Vec<&str> = s.split(':').collect();
    let code = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let signal = parts
        .get(1)
        .and_then(|p| p.parse().ok())
        .filter(|&s| s != 0);
    (code, signal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exit_code_signal() {
        assert_eq!(parse_exit_code_signal("0:0"), (0, None));
        assert_eq!(parse_exit_code_signal("1:0"), (1, None));
        assert_eq!(parse_exit_code_signal("137:9"), (137, Some(9)));
    }
}
