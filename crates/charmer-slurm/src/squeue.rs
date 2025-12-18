//! Query active SLURM jobs via squeue.

use crate::types::{SlurmJob, SlurmJobState};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
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

/// Parse a SLURM timestamp (YYYY-MM-DDTHH:MM:SS or "N/A").
fn parse_slurm_time(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() || s == "N/A" || s == "Unknown" {
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

/// Parse memory string (e.g., "4G", "1000M", "4096").
fn parse_memory(s: &str) -> Option<u64> {
    if s.is_empty() {
        return None;
    }

    let s = s.trim();
    if let Some(stripped) = s.strip_suffix('G') {
        stripped.parse::<u64>().ok().map(|v| v * 1024)
    } else if let Some(stripped) = s.strip_suffix('M') {
        stripped.parse::<u64>().ok()
    } else if let Some(stripped) = s.strip_suffix('K') {
        stripped.parse::<u64>().ok().map(|v| v / 1024)
    } else {
        // Assume MB if no suffix
        s.parse::<u64>().ok()
    }
}

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
    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() < 12 {
        return Err(SqueueError::ParseError(format!(
            "Expected 12 fields, got {}: {}",
            fields.len(),
            line
        )));
    }

    Ok(SlurmJob {
        job_id: fields[0].to_string(),
        name: fields[1].to_string(),
        state: parse_state(fields[2]),
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

/// Query active jobs with squeue.
pub async fn query_squeue(run_uuid: Option<&str>) -> Result<Vec<SlurmJob>, SqueueError> {
    let user = std::env::var("USER").unwrap_or_else(|_| "".to_string());

    let mut cmd = Command::new("squeue");
    cmd.args(["-u", &user, "-h", "-o", SQUEUE_FORMAT]);

    // If run_uuid specified, filter by job name
    if let Some(uuid) = run_uuid {
        cmd.args(["--name", uuid]);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| SqueueError::ExecutionError(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SqueueError::ExecutionError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
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
        let dt = parse_slurm_time("2024-01-15T10:30:00").unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");

        assert!(parse_slurm_time("N/A").is_none());
        assert!(parse_slurm_time("").is_none());
    }

    #[test]
    fn test_parse_time_limit() {
        assert_eq!(parse_time_limit("1:00:00"), Some(Duration::from_secs(3600)));
        assert_eq!(
            parse_time_limit("1-00:00:00"),
            Some(Duration::from_secs(86400))
        );
        assert_eq!(parse_time_limit("30:00"), Some(Duration::from_secs(1800)));
        assert!(parse_time_limit("UNLIMITED").is_none());
    }

    #[test]
    fn test_parse_memory() {
        assert_eq!(parse_memory("4G"), Some(4096));
        assert_eq!(parse_memory("1000M"), Some(1000));
        assert_eq!(parse_memory("4096"), Some(4096));
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
