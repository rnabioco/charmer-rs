//! SLURM job types.

use chrono::{DateTime, Utc};
use std::time::Duration;

/// SLURM job status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlurmJobState {
    Pending,
    Running,
    Completed { exit_code: i32, runtime: Duration },
    Failed { exit_code: i32, error: String },
    Cancelled,
    Timeout,
    OutOfMemory,
    Unknown(String),
}

/// SLURM job information from squeue/sacct.
#[derive(Debug, Clone)]
pub struct SlurmJob {
    /// SLURM job ID
    pub job_id: String,

    /// Job name (run_uuid for snakemake SLURM plugin)
    pub name: String,

    /// Job state
    pub state: SlurmJobState,

    /// Partition
    pub partition: Option<String>,

    /// Submit time
    pub submit_time: Option<DateTime<Utc>>,

    /// Start time
    pub start_time: Option<DateTime<Utc>>,

    /// End time
    pub end_time: Option<DateTime<Utc>>,

    /// Node list
    pub nodelist: Option<String>,

    /// Allocated CPUs
    pub cpus: Option<u32>,

    /// Memory (in MB)
    pub mem_mb: Option<u64>,

    /// Time limit
    pub time_limit: Option<Duration>,

    /// Comment field (contains rule info for snakemake)
    pub comment: Option<String>,
}
