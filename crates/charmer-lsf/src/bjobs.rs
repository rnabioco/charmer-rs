//! Query active LSF jobs via bjobs.

use crate::types::{LsfJob, LsfJobState};
use charmer_parsers::{
    non_empty_string, parse_lsf_timestamp, parse_memory_mb, run_command_allow_failure,
    split_delimited, MemoryFormat,
};
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
    let fields = split_delimited(line, 10).map_err(BjobsError::ParseError)?;

    Ok(LsfJob {
        job_id: fields[0].trim().to_string(),
        name: String::new(), // bjobs doesn't include name in this format
        state: parse_state(fields[1].trim()),
        queue: non_empty_string(fields[2]),
        submit_time: parse_lsf_timestamp(fields[3].trim()),
        start_time: parse_lsf_timestamp(fields[4].trim()),
        end_time: parse_lsf_timestamp(fields[5].trim()),
        exec_host: non_empty_string(fields[6]),
        nprocs: fields[7].trim().parse().ok(),
        mem_limit_mb: parse_memory_mb(fields[8].trim(), MemoryFormat::Lsf),
        mem_used_mb: None,
        run_limit: None,
        description: non_empty_string(fields[9]),
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

    // bjobs returns non-zero if no jobs found, which is OK
    let stdout = run_command_allow_failure(&mut cmd, "bjobs")
        .await
        .map_err(|e| BjobsError::ExecutionError(e.to_string()))?;

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
    use charmer_parsers::parse_lsf_timestamp;

    #[test]
    fn test_parse_state() {
        assert_eq!(parse_state("PEND"), LsfJobState::Pending);
        assert_eq!(parse_state("RUN"), LsfJobState::Running);
        assert!(matches!(parse_state("DONE"), LsfJobState::Done { .. }));
        assert!(matches!(parse_state("EXIT"), LsfJobState::Exit { .. }));
    }

    #[test]
    fn test_parse_memory() {
        assert_eq!(parse_memory_mb("4 GB", MemoryFormat::Lsf), Some(4096));
        assert_eq!(parse_memory_mb("1000 MB", MemoryFormat::Lsf), Some(1000));
        assert_eq!(parse_memory_mb("1000", MemoryFormat::Lsf), Some(1000));
        assert!(parse_memory_mb("-", MemoryFormat::Lsf).is_none());
    }

    #[test]
    fn test_parse_lsf_time() {
        // With year
        let dt = parse_lsf_timestamp("Dec 18 10:30 2024").unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-12-18");

        assert!(parse_lsf_timestamp("-").is_none());
    }
}
