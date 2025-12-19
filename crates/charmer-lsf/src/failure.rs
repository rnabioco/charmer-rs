//! LSF job failure analysis.
//!
//! Query detailed failure information and provide actionable suggestions.

use charmer_parsers::{
    format_duration, format_duration_lsf, parse_duration_secs, parse_memory_mb,
    run_command_allow_failure, MemoryFormat,
};
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum FailureError {
    #[error("Failed to execute bhist: {0}")]
    ExecutionError(String),
    #[error("Job not found: {0}")]
    NotFound(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Failure mode classification for LSF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureMode {
    /// Job ran out of memory
    OutOfMemory {
        used_mb: u64,
        limit_mb: u64,
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
    /// Job was killed by user or admin
    Killed { by_user: Option<String> },
    /// Host/node failure
    HostFailure { host: Option<String> },
    /// Unknown failure mode
    Unknown { term_reason: String },
}

/// Detailed failure analysis result for LSF.
#[derive(Debug, Clone)]
pub struct FailureAnalysis {
    /// LSF job ID
    pub job_id: String,
    /// Classified failure mode
    pub mode: FailureMode,
    /// Human-readable explanation
    pub explanation: String,
    /// Suggested fix
    pub suggestion: String,
    /// Raw termination reason
    pub term_reason: String,
    /// Actual memory used (MB)
    pub max_mem_mb: Option<u64>,
    /// Memory limit (MB)
    pub mem_limit_mb: Option<u64>,
    /// Actual runtime (seconds)
    pub run_time_seconds: Option<u64>,
    /// Time limit (seconds)
    pub run_limit_seconds: Option<u64>,
}

impl FailureAnalysis {
    /// Generate explanation and suggestion based on failure mode.
    fn generate_messages(mode: &FailureMode) -> (String, String) {
        match mode {
            FailureMode::OutOfMemory {
                used_mb,
                limit_mb,
                suggested_mb,
            } => {
                let explanation = format!(
                    "Job exceeded memory limit. Used {:.1} GB but limit was {:.1} GB.",
                    *used_mb as f64 / 1024.0,
                    *limit_mb as f64 / 1024.0
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
                    format_duration_lsf(*suggested_seconds)
                );
                (explanation, suggestion)
            }
            FailureMode::ExitCode { code, signal } => {
                let explanation = if let Some(sig) = signal {
                    format!("Job exited with code {} and signal {}", code, sig)
                } else {
                    match code {
                        1 => "Job failed with exit code 1 (general error)".to_string(),
                        137 => {
                            "Job killed (likely OOM). Exit code 137 = 128 + 9 (SIGKILL)".to_string()
                        }
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
            FailureMode::Killed { by_user } => {
                let explanation = if let Some(user) = by_user {
                    format!("Job was killed by {}", user)
                } else {
                    "Job was killed".to_string()
                };
                (
                    "Consider if this was intentional or due to dependency failure.".to_string(),
                    explanation,
                )
            }
            FailureMode::HostFailure { host } => {
                let explanation = if let Some(h) = host {
                    format!("Job failed due to host {} failure", h)
                } else {
                    "Job failed due to host failure".to_string()
                };
                (
                    "Re-run the job. If persistent, contact cluster admin.".to_string(),
                    explanation,
                )
            }
            FailureMode::Unknown { term_reason } => (
                format!("Job failed: {}", term_reason),
                "Check LSF logs for details.".to_string(),
            ),
        }
    }
}

/// Query detailed failure information for an LSF job.
pub async fn analyze_failure(job_id: &str) -> Result<FailureAnalysis, FailureError> {
    // Use bhist -l to get detailed job history including termination info
    let mut cmd = Command::new("bhist");
    cmd.args(["-l", job_id]);

    let stdout = run_command_allow_failure(&mut cmd, "bhist")
        .await
        .map_err(|e| FailureError::ExecutionError(e.to_string()))?;

    if stdout.contains("No matching job found") || stdout.is_empty() {
        return Err(FailureError::NotFound(job_id.to_string()));
    }

    parse_bhist_output(job_id, &stdout)
}

/// Parse bhist -l output for failure analysis.
fn parse_bhist_output(job_id: &str, output: &str) -> Result<FailureAnalysis, FailureError> {
    let mut term_reason = String::new();
    let mut max_mem_mb: Option<u64> = None;
    let mut mem_limit_mb: Option<u64> = None;
    let mut run_time_seconds: Option<u64> = None;
    let mut run_limit_seconds: Option<u64> = None;
    let mut exit_code: Option<i32> = None;

    for line in output.lines() {
        let line = line.trim();

        // Look for termination reason
        if line.contains("Exited with exit code") {
            if let Some(code) = extract_number(line, "exit code") {
                exit_code = Some(code as i32);
            }
            term_reason = line.to_string();
        } else if line.contains("TERM_") {
            term_reason = line.to_string();
        }

        // Look for memory info
        if line.contains("MAX MEM:") {
            max_mem_mb = parse_lsf_memory_from_line(line, "MAX MEM:");
        }
        if line.contains("MEMLIMIT") || line.contains("MEM LIMIT:") {
            mem_limit_mb = parse_lsf_memory_from_line(line, "MEMLIMIT")
                .or_else(|| parse_lsf_memory_from_line(line, "MEM LIMIT:"));
        }

        // Look for runtime info
        if line.contains("Run time:") || line.contains("RUN_TIME:") {
            run_time_seconds = parse_lsf_time_from_line(line);
        }
        if line.contains("RUNLIMIT") || line.contains("RUN LIMIT:") {
            run_limit_seconds = parse_lsf_time_from_line(line);
        }
    }

    // Determine failure mode
    let mode = if term_reason.contains("TERM_MEMLIMIT") {
        let used = max_mem_mb.unwrap_or(0);
        let limit = mem_limit_mb.unwrap_or(0);
        let suggested = ((used as f64 * 1.5) / 1024.0).ceil() as u64 * 1024;
        FailureMode::OutOfMemory {
            used_mb: used,
            limit_mb: limit,
            suggested_mb: suggested.max(limit + 1024),
        }
    } else if term_reason.contains("TERM_RUNLIMIT") {
        let elapsed = run_time_seconds.unwrap_or(0);
        let limit = run_limit_seconds.unwrap_or(0);
        let suggested = (elapsed as f64 * 1.5) as u64;
        FailureMode::Timeout {
            elapsed_seconds: elapsed,
            limit_seconds: limit,
            suggested_seconds: suggested.max(limit + 3600),
        }
    } else if term_reason.contains("TERM_OWNER") || term_reason.contains("TERM_ADMIN") {
        FailureMode::Killed { by_user: None }
    } else if term_reason.contains("TERM_HOST") || term_reason.contains("TERM_LOAD") {
        FailureMode::HostFailure { host: None }
    } else if let Some(code) = exit_code {
        if code == 137 {
            let used = max_mem_mb.unwrap_or(0);
            let limit = mem_limit_mb.unwrap_or(0);
            let suggested = ((used as f64 * 1.5) / 1024.0).ceil() as u64 * 1024;
            FailureMode::OutOfMemory {
                used_mb: used,
                limit_mb: limit,
                suggested_mb: suggested.max(limit + 1024),
            }
        } else {
            FailureMode::ExitCode { code, signal: None }
        }
    } else if !term_reason.is_empty() {
        FailureMode::Unknown {
            term_reason: term_reason.clone(),
        }
    } else {
        FailureMode::Unknown {
            term_reason: "Unknown failure".to_string(),
        }
    };

    let (explanation, suggestion) = FailureAnalysis::generate_messages(&mode);

    Ok(FailureAnalysis {
        job_id: job_id.to_string(),
        mode,
        explanation,
        suggestion,
        term_reason,
        max_mem_mb,
        mem_limit_mb,
        run_time_seconds,
        run_limit_seconds,
    })
}

/// Extract a number after a given prefix.
fn extract_number(s: &str, prefix: &str) -> Option<u64> {
    if let Some(idx) = s.find(prefix) {
        let after = &s[idx + prefix.len()..];
        let num_str: String = after
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        num_str.parse().ok()
    } else {
        None
    }
}

/// Parse LSF memory value from a line with a prefix (e.g., "MAX MEM: 4.5 Gbytes").
fn parse_lsf_memory_from_line(line: &str, prefix: &str) -> Option<u64> {
    if let Some(idx) = line.find(prefix) {
        let after = &line[idx + prefix.len()..].trim();
        // Extract just "4.5 Gbytes" part for the shared parser
        let mem_str: String = after
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        parse_memory_mb(&mem_str, MemoryFormat::Lsf)
    } else {
        None
    }
}

/// Parse LSF time value from a line (e.g., "Run time: 01:30:00").
fn parse_lsf_time_from_line(line: &str) -> Option<u64> {
    // Look for HH:MM:SS pattern in the line
    for word in line.split_whitespace() {
        if word.contains(':') && word.chars().filter(|c| *c == ':').count() == 2 {
            return parse_duration_secs(word);
        }
    }
    // Look for "N seconds" pattern
    if let Some(idx) = line.find("seconds") {
        let before = line[..idx].trim();
        if let Some(num_str) = before.split_whitespace().last() {
            if let Ok(secs) = num_str.parse::<u64>() {
                return Some(secs);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lsf_memory_from_line() {
        assert_eq!(
            parse_lsf_memory_from_line("MAX MEM: 4.5 GB", "MAX MEM:"),
            Some(4608)
        );
        assert_eq!(
            parse_lsf_memory_from_line("MEMLIMIT 8192 MB", "MEMLIMIT"),
            Some(8192)
        );
    }

    #[test]
    fn test_parse_lsf_time_from_line() {
        assert_eq!(parse_lsf_time_from_line("Run time: 01:30:00"), Some(5400));
        assert_eq!(parse_lsf_time_from_line("1800 seconds"), Some(1800));
    }
}
