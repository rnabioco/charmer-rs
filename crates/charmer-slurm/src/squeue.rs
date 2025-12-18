//! Query active SLURM jobs via squeue.

use crate::types::{SlurmJob, SlurmJobState};
use charmer_parsers::{
    non_empty_string, parse_duration, parse_memory_mb, parse_slurm_timestamp, run_command,
    split_delimited, MemoryFormat,
};
use std::time::Duration;
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum SqueueError {
    #[error("Failed to execute squeue: {0}")]
    ExecutionError(String),
    #[error("Failed to parse squeue output: {0}")]
    ParseError(String),
}

/// squeue output format:
/// %A - Job ID
/// %j - Job name
/// %T - State (extended)
/// %P - Partition
/// %V - Submit time
/// %S - Start time
/// %e - End time (estimated)
/// %N - Nodelist
/// %C - CPUs
/// %m - Memory
/// %l - Time limit
/// %k - Comment
const SQUEUE_FORMAT: &str = "%A|%j|%T|%P|%V|%S|%e|%N|%C|%m|%l|%k";

/// Parse SLURM state string.
fn parse_state(s: &str) -> SlurmJobState {
    match s.to_uppercase().as_str() {
        "PENDING" | "PD" => SlurmJobState::Pending,
        "RUNNING" | "R" => SlurmJobState::Running,
        "COMPLETED" | "CD" => SlurmJobState::Completed {
            exit_code: 0,
            runtime: Duration::ZERO,
        },
        "FAILED" | "F" => SlurmJobState::Failed {
            exit_code: 1,
            error: String::new(),
        },
        "CANCELLED" | "CA" => SlurmJobState::Cancelled,
        "TIMEOUT" | "TO" => SlurmJobState::Timeout,
        "OUT_OF_MEMORY" | "OOM" => SlurmJobState::OutOfMemory,
        other => SlurmJobState::Unknown(other.to_string()),
    }
}

/// Parse a single line of squeue output.
fn parse_squeue_line(line: &str) -> Result<SlurmJob, SqueueError> {
    let fields = split_delimited(line, 12).map_err(SqueueError::ParseError)?;

    Ok(SlurmJob {
        job_id: fields[0].to_string(),
        name: fields[1].to_string(),
        state: parse_state(fields[2]),
        partition: non_empty_string(fields[3]),
        submit_time: parse_slurm_timestamp(fields[4]),
        start_time: parse_slurm_timestamp(fields[5]),
        end_time: parse_slurm_timestamp(fields[6]),
        nodelist: non_empty_string(fields[7]),
        cpus: fields[8].parse().ok(),
        mem_mb: parse_memory_mb(fields[9], MemoryFormat::Slurm),
        time_limit: parse_duration(fields[10]),
        comment: non_empty_string(fields[11]),
    })
}

/// Query active jobs with squeue.
pub async fn query_squeue(run_uuid: Option<&str>) -> Result<Vec<SlurmJob>, SqueueError> {
    let user = std::env::var("USER").unwrap_or_default();

    let mut cmd = Command::new("squeue");
    cmd.args(["-u", &user, "-h", "-o", SQUEUE_FORMAT]);

    // If run_uuid specified, filter by job name
    if let Some(uuid) = run_uuid {
        cmd.args(["--name", uuid]);
    }

    let stdout = run_command(&mut cmd, "squeue")
        .await
        .map_err(|e| SqueueError::ExecutionError(e.to_string()))?;

    let mut jobs = Vec::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_squeue_line(line) {
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
    fn test_parse_slurm_time() {
        let dt = parse_slurm_timestamp("2024-01-15T10:30:00").unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");

        assert!(parse_slurm_timestamp("N/A").is_none());
        assert!(parse_slurm_timestamp("").is_none());
    }

    #[test]
    fn test_parse_time_limit() {
        assert_eq!(parse_duration("1:00:00"), Some(Duration::from_secs(3600)));
        assert_eq!(
            parse_duration("1-00:00:00"),
            Some(Duration::from_secs(86400))
        );
        assert_eq!(parse_duration("30:00"), Some(Duration::from_secs(1800)));
        assert!(parse_duration("UNLIMITED").is_none());
    }

    #[test]
    fn test_parse_memory() {
        assert_eq!(parse_memory_mb("4G", MemoryFormat::Slurm), Some(4096));
        assert_eq!(parse_memory_mb("1000M", MemoryFormat::Slurm), Some(1000));
        assert_eq!(parse_memory_mb("4096", MemoryFormat::Slurm), Some(4096));
    }

    #[test]
    fn test_parse_state() {
        assert_eq!(parse_state("RUNNING"), SlurmJobState::Running);
        assert_eq!(parse_state("R"), SlurmJobState::Running);
        assert_eq!(parse_state("PENDING"), SlurmJobState::Pending);
        assert_eq!(parse_state("PD"), SlurmJobState::Pending);
    }

    #[test]
    fn test_parse_squeue_line() {
        let line = "12345|test_job|RUNNING|short|2024-01-15T10:00:00|2024-01-15T10:05:00|N/A|node01|4|4G|1:00:00|rule_align_wildcards_sample=S1";
        let job = parse_squeue_line(line).unwrap();
        assert_eq!(job.job_id, "12345");
        assert_eq!(job.name, "test_job");
        assert_eq!(job.state, SlurmJobState::Running);
        assert_eq!(job.cpus, Some(4));
        assert_eq!(
            job.comment,
            Some("rule_align_wildcards_sample=S1".to_string())
        );
    }
}
