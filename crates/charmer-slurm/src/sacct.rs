//! Query SLURM job history via sacct.

use crate::types::{SlurmJob, SlurmJobState};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use std::time::Duration;
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum SacctError {
    #[error("Failed to execute sacct: {0}")]
    ExecutionError(String),
    #[error("Failed to parse sacct output: {0}")]
    ParseError(String),
}

/// sacct output format (--parsable2 uses | delimiter)
/// JobIDRaw, JobName, State, Partition, Submit, Start, End, NodeList, AllocCPUS, ReqMem, Timelimit, Comment, ExitCode
const SACCT_FORMAT: &str =
    "JobIDRaw,JobName,State,Partition,Submit,Start,End,NodeList,AllocCPUS,ReqMem,Timelimit,Comment,ExitCode";

/// Parse a SLURM timestamp (YYYY-MM-DDTHH:MM:SS or "Unknown").
fn parse_slurm_time(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() || s == "Unknown" || s == "None" {
        return None;
    }
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .and_then(|dt| Utc.from_local_datetime(&dt).single())
}

/// Parse a SLURM time limit (D-HH:MM:SS or HH:MM:SS or MM:SS).
fn parse_time_limit(s: &str) -> Option<Duration> {
    if s.is_empty() || s == "UNLIMITED" {
        return None;
    }

    let parts: Vec<&str> = s.split('-').collect();
    let (days, time_part) = if parts.len() == 2 {
        (parts[0].parse::<u64>().unwrap_or(0), parts[1])
    } else {
        (0, parts[0])
    };

    let time_parts: Vec<u64> = time_part
        .split(':')
        .filter_map(|p| p.parse().ok())
        .collect();

    let seconds = match time_parts.len() {
        3 => time_parts[0] * 3600 + time_parts[1] * 60 + time_parts[2],
        2 => time_parts[0] * 60 + time_parts[1],
        1 => time_parts[0],
        _ => return None,
    };

    Some(Duration::from_secs(days * 86400 + seconds))
}

/// Parse memory string (e.g., "4Gn", "1000Mn", "4096").
fn parse_memory(s: &str) -> Option<u64> {
    if s.is_empty() {
        return None;
    }

    // sacct memory can have 'n' or 'c' suffix (per node/per core)
    let s = s.trim().trim_end_matches('n').trim_end_matches('c');

    if let Some(stripped) = s.strip_suffix('G') {
        stripped.parse::<u64>().ok().map(|v| v * 1024)
    } else if let Some(stripped) = s.strip_suffix('M') {
        stripped.parse::<u64>().ok()
    } else if let Some(stripped) = s.strip_suffix('K') {
        stripped.parse::<u64>().ok().map(|v| v / 1024)
    } else {
        s.parse::<u64>().ok()
    }
}

/// Parse exit code (format: "exit_code:signal").
fn parse_exit_code(s: &str) -> i32 {
    s.split(':')
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Parse sacct state string with exit code info.
fn parse_state(state_str: &str, exit_code_str: &str) -> SlurmJobState {
    let exit_code = parse_exit_code(exit_code_str);

    // sacct states can have suffixes like "CANCELLED by 12345"
    let base_state = state_str.split_whitespace().next().unwrap_or(state_str);

    match base_state.to_uppercase().as_str() {
        "PENDING" => SlurmJobState::Pending,
        "RUNNING" => SlurmJobState::Running,
        "COMPLETED" => SlurmJobState::Completed {
            exit_code,
            runtime: Duration::ZERO, // Would need to calculate from start/end
        },
        "FAILED" => SlurmJobState::Failed {
            exit_code,
            error: format!("Exit code: {}", exit_code),
        },
        "CANCELLED" => SlurmJobState::Cancelled,
        "TIMEOUT" => SlurmJobState::Timeout,
        "OUT_OF_MEMORY" => SlurmJobState::OutOfMemory,
        "NODE_FAIL" => SlurmJobState::Failed {
            exit_code: -1,
            error: "Node failure".to_string(),
        },
        other => SlurmJobState::Unknown(other.to_string()),
    }
}

/// Parse a single line of sacct output.
fn parse_sacct_line(line: &str) -> Result<SlurmJob, SacctError> {
    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() < 13 {
        return Err(SacctError::ParseError(format!(
            "Expected 13 fields, got {}: {}",
            fields.len(),
            line
        )));
    }

    let state = parse_state(fields[2], fields[12]);

    Ok(SlurmJob {
        job_id: fields[0].to_string(),
        name: fields[1].to_string(),
        state,
        partition: Some(fields[3].to_string()).filter(|s| !s.is_empty()),
        submit_time: parse_slurm_time(fields[4]),
        start_time: parse_slurm_time(fields[5]),
        end_time: parse_slurm_time(fields[6]),
        nodelist: Some(fields[7].to_string()).filter(|s| !s.is_empty()),
        cpus: fields[8].parse().ok(),
        mem_mb: parse_memory(fields[9]),
        time_limit: parse_time_limit(fields[10]),
        comment: Some(fields[11].to_string()).filter(|s| !s.is_empty()),
    })
}

/// Query job history with sacct.
pub async fn query_sacct(
    run_uuid: Option<&str>,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<SlurmJob>, SacctError> {
    let mut cmd = Command::new("sacct");
    cmd.args(["-X", "--parsable2", "--noheader", "--format", SACCT_FORMAT]);

    // Add time filter
    if let Some(since_time) = since {
        let time_str = since_time.format("%Y-%m-%dT%H:%M:%S").to_string();
        cmd.args(["--starttime", &time_str]);
    } else {
        // Default to last 24 hours
        cmd.args(["--starttime", "now-24hours"]);
    }

    // Filter by job name if run_uuid specified
    if let Some(uuid) = run_uuid {
        cmd.args(["--name", uuid]);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| SacctError::ExecutionError(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SacctError::ExecutionError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut jobs = Vec::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_sacct_line(line) {
            Ok(job) => jobs.push(job),
            Err(e) => eprintln!("Warning: {}", e),
        }
    }

    Ok(jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exit_code() {
        assert_eq!(parse_exit_code("0:0"), 0);
        assert_eq!(parse_exit_code("1:0"), 1);
        assert_eq!(parse_exit_code("137:9"), 137);
    }

    #[test]
    fn test_parse_state() {
        assert!(matches!(
            parse_state("COMPLETED", "0:0"),
            SlurmJobState::Completed { exit_code: 0, .. }
        ));
        assert!(matches!(
            parse_state("FAILED", "1:0"),
            SlurmJobState::Failed { exit_code: 1, .. }
        ));
        assert_eq!(
            parse_state("CANCELLED by 12345", "0:0"),
            SlurmJobState::Cancelled
        );
    }

    #[test]
    fn test_parse_sacct_line() {
        let line = "12345|test_job|COMPLETED|short|2024-01-15T10:00:00|2024-01-15T10:05:00|2024-01-15T10:10:00|node01|4|4Gn|1:00:00|rule_align_wildcards_sample=S1|0:0";
        let job = parse_sacct_line(line).unwrap();
        assert_eq!(job.job_id, "12345");
        assert_eq!(job.name, "test_job");
        assert!(matches!(job.state, SlurmJobState::Completed { .. }));
    }
}
