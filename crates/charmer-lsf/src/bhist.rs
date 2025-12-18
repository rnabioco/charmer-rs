//! Query LSF job history via bhist.

use crate::types::{LsfJob, LsfJobState};
use chrono::{DateTime, Datelike, NaiveDateTime, TimeZone, Utc};
use std::time::Duration;
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum BhistError {
    #[error("Failed to execute bhist: {0}")]
    ExecutionError(String),
    #[error("Failed to parse bhist output: {0}")]
    ParseError(String),
}

/// Parse LSF timestamp format (Mon DD HH:MM or Mon DD HH:MM YYYY)
#[allow(dead_code)]
fn parse_lsf_time(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() || s == "-" {
        return None;
    }

    let current_year = Utc::now().year();

    // Try with year first
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%b %d %H:%M %Y") {
        return Utc.from_local_datetime(&dt).single();
    }

    // Try without year
    if let Ok(dt) =
        NaiveDateTime::parse_from_str(&format!("{} {}", s, current_year), "%b %d %H:%M %Y")
    {
        return Utc.from_local_datetime(&dt).single();
    }

    None
}

/// Parse LSF memory string.
fn parse_memory(s: &str) -> Option<u64> {
    if s.is_empty() || s == "-" {
        return None;
    }

    let parts: Vec<&str> = s.split_whitespace().collect();
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

/// Parse LSF run time string (HH:MM:SS or seconds).
#[allow(dead_code)]
fn parse_runtime(s: &str) -> Option<Duration> {
    if s.is_empty() || s == "-" {
        return None;
    }

    // Try HH:MM:SS format
    let parts: Vec<u64> = s.split(':').filter_map(|p| p.parse().ok()).collect();
    match parts.len() {
        3 => Some(Duration::from_secs(
            parts[0] * 3600 + parts[1] * 60 + parts[2],
        )),
        2 => Some(Duration::from_secs(parts[0] * 60 + parts[1])),
        1 => Some(Duration::from_secs(parts[0])),
        _ => s.parse::<u64>().ok().map(Duration::from_secs),
    }
}

/// Query job history with bhist.
/// Note: bhist output format varies by LSF version, this is a basic implementation.
pub async fn query_bhist(
    job_name_filter: Option<&str>,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<LsfJob>, BhistError> {
    let mut cmd = Command::new("bhist");
    cmd.args(["-a", "-l"]); // All jobs, long format

    // Filter by job name if specified
    if let Some(name) = job_name_filter {
        cmd.args(["-J", name]);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| BhistError::ExecutionError(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // bhist -l output is complex multi-line format, parse job blocks
    parse_bhist_long_output(&stdout, since)
}

/// Parse bhist -l (long format) output.
/// Jobs are separated by dashed lines and contain structured info.
fn parse_bhist_long_output(
    output: &str,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<LsfJob>, BhistError> {
    let mut jobs = Vec::new();
    let mut current_job: Option<LsfJob> = None;

    for line in output.lines() {
        let line = line.trim();

        // Job header line: "Job <12345>, ..."
        if line.starts_with("Job <") {
            // Save previous job if exists
            if let Some(job) = current_job.take() {
                // Filter by time if specified
                if let Some(since_time) = since {
                    if job.submit_time.map(|t| t >= since_time).unwrap_or(true) {
                        jobs.push(job);
                    }
                } else {
                    jobs.push(job);
                }
            }

            // Parse job ID
            if let Some(end) = line.find(">,") {
                let job_id = line[5..end].to_string();
                current_job = Some(LsfJob {
                    job_id,
                    name: String::new(),
                    state: LsfJobState::Unknown("UNKNOWN".to_string()),
                    queue: None,
                    submit_time: None,
                    start_time: None,
                    end_time: None,
                    exec_host: None,
                    nprocs: None,
                    mem_limit_mb: None,
                    mem_used_mb: None,
                    run_limit: None,
                    description: None,
                });
            }
        }

        // Parse job details from current job
        if let Some(ref mut job) = current_job {
            if line.contains("Job Name <") {
                if let (Some(start), Some(end)) = (line.find("Job Name <"), line.rfind(">")) {
                    job.name = line[start + 10..end].to_string();
                }
            }
            if line.contains("Queue <") {
                if let (Some(start), Some(end)) = (line.find("Queue <"), line.find(">,")) {
                    job.queue = Some(line[start + 7..end].to_string());
                }
            }
            if line.starts_with("Submitted from") || line.contains("submitted from") {
                // Parse submit time from context
            }
            if line.contains("Started on") {
                if let Some(host_start) = line.find("Started on <") {
                    if let Some(host_end) = line[host_start..].find(">,") {
                        job.exec_host =
                            Some(line[host_start + 12..host_start + host_end].to_string());
                    }
                }
            }
            if line.contains("Done successfully") {
                job.state = LsfJobState::Done {
                    exit_code: 0,
                    runtime: Duration::ZERO,
                };
            }
            if line.contains("Exited with exit code") {
                let exit_code = line
                    .split("exit code")
                    .nth(1)
                    .and_then(|s| s.trim().trim_end_matches('.').parse().ok())
                    .unwrap_or(1);
                job.state = LsfJobState::Exit {
                    exit_code,
                    error: String::new(),
                };
            }
            if line.contains("MAX MEM:") {
                if let Some(mem_str) = line.split("MAX MEM:").nth(1) {
                    job.mem_used_mb = parse_memory(mem_str.trim().split(';').next().unwrap_or(""));
                }
            }
        }
    }

    // Don't forget last job
    if let Some(job) = current_job {
        if let Some(since_time) = since {
            if job.submit_time.map(|t| t >= since_time).unwrap_or(true) {
                jobs.push(job);
            }
        } else {
            jobs.push(job);
        }
    }

    Ok(jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_runtime() {
        assert_eq!(parse_runtime("1:30:00"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_runtime("30:00"), Some(Duration::from_secs(1800)));
        assert_eq!(parse_runtime("3600"), Some(Duration::from_secs(3600)));
        assert!(parse_runtime("-").is_none());
    }
}
