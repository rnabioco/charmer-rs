//! Query SLURM job history via sacct.

use crate::types::{SlurmJob, SlurmJobState};
use charmer_parsers::{
    MemoryFormat, non_empty_string, parse_duration, parse_duration_secs, parse_exit_code,
    parse_memory_mb, parse_slurm_timestamp, run_command, split_delimited,
};
use chrono::{DateTime, Utc};
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
const SACCT_FORMAT: &str = "JobIDRaw,JobName,State,Partition,Submit,Start,End,NodeList,AllocCPUS,ReqMem,Timelimit,Comment,ExitCode";

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
    let fields = split_delimited(line, 13).map_err(SacctError::ParseError)?;

    let state = parse_state(fields[2], fields[12]);

    Ok(SlurmJob {
        job_id: fields[0].to_string(),
        name: fields[1].to_string(),
        state,
        partition: non_empty_string(fields[3]),
        submit_time: parse_slurm_timestamp(fields[4]),
        start_time: parse_slurm_timestamp(fields[5]),
        end_time: parse_slurm_timestamp(fields[6]),
        nodelist: non_empty_string(fields[7]),
        cpus: fields[8].parse().ok(),
        mem_mb: parse_memory_mb(fields[9], MemoryFormat::SlurmSacct),
        time_limit: parse_duration(fields[10]),
        comment: non_empty_string(fields[11]),
    })
}

/// Resource usage data from sacct.
#[derive(Debug, Clone)]
pub struct SlurmResourceUsage {
    pub job_id: String,
    pub max_rss_mb: Option<u64>,
    pub elapsed_seconds: Option<u64>,
    pub cpu_time_seconds: Option<u64>,
}

/// Query resource usage for a specific job.
pub async fn query_resource_usage(job_id: &str) -> Result<Option<SlurmResourceUsage>, SacctError> {
    let mut cmd = Command::new("sacct");
    cmd.args([
        "-j",
        job_id,
        "-X",
        "--parsable2",
        "--noheader",
        "--format",
        "JobIDRaw,MaxRSS,Elapsed,TotalCPU",
    ]);

    let stdout = run_command(&mut cmd, "sacct")
        .await
        .map_err(|e| SacctError::ExecutionError(e.to_string()))?;

    let line = match stdout.lines().next() {
        Some(l) if !l.trim().is_empty() => l,
        _ => return Ok(None),
    };

    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() < 4 {
        return Ok(None);
    }

    Ok(Some(SlurmResourceUsage {
        job_id: fields[0].to_string(),
        max_rss_mb: parse_memory_mb(fields[1], MemoryFormat::SlurmSacct),
        elapsed_seconds: parse_elapsed_time(fields[2]),
        cpu_time_seconds: parse_elapsed_time(fields[3]),
    }))
}

/// Parse elapsed time string, stripping any milliseconds before parsing.
fn parse_elapsed_time(s: &str) -> Option<u64> {
    if s.is_empty() || s == "Unknown" {
        return None;
    }
    // Strip milliseconds (e.g., "01:30:00.123" -> "01:30:00")
    let clean = s.split('.').next().unwrap_or(s);
    parse_duration_secs(clean)
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

    let stdout = run_command(&mut cmd, "sacct")
        .await
        .map_err(|e| SacctError::ExecutionError(e.to_string()))?;

    let mut jobs = Vec::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_sacct_line(line) {
            Ok(job) => jobs.push(job),
            Err(e) => tracing::warn!("Failed to parse sacct line: {}", e),
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
