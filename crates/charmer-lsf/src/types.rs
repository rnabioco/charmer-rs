//! LSF job types.

use chrono::{DateTime, Utc};
use std::time::Duration;

/// LSF job status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LsfJobState {
    /// PEND - Job is pending
    Pending,
    /// RUN - Job is running
    Running,
    /// DONE - Job completed successfully
    Done { exit_code: i32, runtime: Duration },
    /// EXIT - Job exited with non-zero status
    Exit { exit_code: i32, error: String },
    /// PSUSP - Job suspended by user while pending
    UserSuspendedPending,
    /// USUSP - Job suspended by user while running
    UserSuspended,
    /// SSUSP - Job suspended by system
    SystemSuspended,
    /// ZOMBI - Job is zombie (finished but info not available)
    Zombie,
    /// Unknown state
    Unknown(String),
}

/// LSF job information from bjobs/bhist.
#[derive(Debug, Clone)]
pub struct LsfJob {
    /// LSF job ID
    pub job_id: String,

    /// Job name
    pub name: String,

    /// Job state
    pub state: LsfJobState,

    /// Queue name
    pub queue: Option<String>,

    /// Submit time
    pub submit_time: Option<DateTime<Utc>>,

    /// Start time
    pub start_time: Option<DateTime<Utc>>,

    /// End time
    pub end_time: Option<DateTime<Utc>>,

    /// Execution host(s)
    pub exec_host: Option<String>,

    /// Number of processors
    pub nprocs: Option<u32>,

    /// Memory limit (MB)
    pub mem_limit_mb: Option<u64>,

    /// Actual memory used (MB)
    pub mem_used_mb: Option<u64>,

    /// Run limit (wall clock time)
    pub run_limit: Option<Duration>,

    /// Job description (used by snakemake for rule info)
    pub description: Option<String>,
}
