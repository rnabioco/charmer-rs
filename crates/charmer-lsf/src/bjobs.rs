//! Query active LSF jobs via bjobs.

use crate::types::{LsfJob, LsfJobState};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use std::time::Duration;
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum BjobsError {
    #[error("Failed to execute bjobs: {0}")]
    ExecutionError(String),
    #[error("Failed to parse bjobs output: {0}")]
    ParseError(String),
}

/// bjobs output format (using -o with delimiter)
/// JOBID STAT QUEUE SUBMIT_TIME START_TIME FINISH_TIME EXEC_HOST NPROCS MEMLIMIT JOB_DESCRIPTION
const BJOBS_FORMAT: &str = "jobid stat queue submit_time start_time finish_time exec_host nprocs memlimit job_description delimiter='|'";

/// Parse LSF timestamp format (Mon DD HH:MM or Mon DD HH:MM YYYY)
fn parse_lsf_time(s: &str) -> Option<DateTime<Utc>> {
    use chrono::Datelike;

    if s.is_empty() || s == "-" {
        return None;
    }

    // LSF uses formats like "Dec 18 10:30" or "Dec 18 10:30 2024"
    let current_year = Utc::now().year();

    // Try with year first
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%b %d %H:%M %Y") {
        return Utc.from_local_datetime(&dt).single();
    }

    // Try without year (assume current year)
    if let Ok(dt) = NaiveDateTime::parse_from_str(&format!("{} {}", s, current_year), "%b %d %H:%M %Y") {
        return Utc.from_local_datetime(&dt).single();
    }

    None
}

/// Parse LSF memory string (e.g., "4 GB", "1000 MB").
fn parse_memory(s: &str) -> Option<u64> {
    if s.is_empty() || s == "-" {
        return None;
    }

    let parts: Vec<&str> = s.trim().split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let value: f64 = parts[0].parse().ok()?;
    let unit = parts.get(1).map(|s| s.to_uppercase()).unwrap_or_default();

    match unit.as_str() {
        "GB" | "G" => Some((value * 1024.0) as u64),
        "MB" | "M" | "" => Some(value as u64),
        "KB" | "K" => Some((value / 1024.0) as u64),
        _ => Some(value as u64),
    }
}

/// Parse LSF state string.
fn parse_state(s: &str) -> LsfJobState {
    match s.to_uppercase().as_str() {
        "PEND" => LsfJobState::Pending,
        "RUN" => LsfJobState::Running,
        "DONE" => LsfJobState::Done {
            exit_code: 0,
            runtime: Duration::ZERO,
        },
        "EXIT" => LsfJobState::Exit {
            exit_code: 1,
            error: String::new(),
        },
        "PSUSP" => LsfJobState::UserSuspendedPending,
        "USUSP" => LsfJobState::UserSuspended,
        "SSUSP" => LsfJobState::SystemSuspended,
        "ZOMBI" => LsfJobState::Zombie,
        other => LsfJobState::Unknown(other.to_string()),
    }
}

/// Parse a single line of bjobs output.
fn parse_bjobs_line(line: &str) -> Result<LsfJob, BjobsError> {
    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() < 10 {
        return Err(BjobsError::ParseError(format!(
            "Expected 10 fields, got {}: {}",
            fields.len(),
            line
        )));
    }

    Ok(LsfJob {
        job_id: fields[0].trim().to_string(),
        name: String::new(), // bjobs doesn't include name in this format
        state: parse_state(fields[1].trim()),
        queue: Some(fields[2].trim().to_string()).filter(|s| !s.is_empty() && s != "-"),
        submit_time: parse_lsf_time(fields[3].trim()),
        start_time: parse_lsf_time(fields[4].trim()),
        end_time: parse_lsf_time(fields[5].trim()),
        exec_host: Some(fields[6].trim().to_string()).filter(|s| !s.is_empty() && s != "-"),
        nprocs: fields[7].trim().parse().ok(),
        mem_limit_mb: parse_memory(fields[8].trim()),
        mem_used_mb: None,
        run_limit: None,
        description: Some(fields[9].trim().to_string()).filter(|s| !s.is_empty() && s != "-"),
    })
}

/// Query active jobs with bjobs.
pub async fn query_bjobs(job_name_filter: Option<&str>) -> Result<Vec<LsfJob>, BjobsError> {
    let mut cmd = Command::new("bjobs");
    cmd.args(["-o", BJOBS_FORMAT, "-noheader"]);

    // Filter by job name if specified
    if let Some(name) = job_name_filter {
        cmd.args(["-J", name]);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| BjobsError::ExecutionError(e.to_string()))?;

    // bjobs returns non-zero if no jobs found, which is OK
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut jobs = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("No ") {
            continue;
        }
        match parse_bjobs_line(line) {
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
    fn test_parse_state() {
        assert_eq!(parse_state("PEND"), LsfJobState::Pending);
        assert_eq!(parse_state("RUN"), LsfJobState::Running);
        assert!(matches!(parse_state("DONE"), LsfJobState::Done { .. }));
        assert!(matches!(parse_state("EXIT"), LsfJobState::Exit { .. }));
    }

    #[test]
    fn test_parse_memory() {
        assert_eq!(parse_memory("4 GB"), Some(4096));
        assert_eq!(parse_memory("1000 MB"), Some(1000));
        assert_eq!(parse_memory("1000"), Some(1000));
        assert!(parse_memory("-").is_none());
    }
}
