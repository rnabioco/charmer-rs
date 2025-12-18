//! Unified job and pipeline state types.

use camino::Utf8PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Unified job status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Waiting for dependencies
    Pending,
    /// Queued in SLURM
    Queued,
    /// Currently running
    Running,
    /// Completed successfully
    Completed,
    /// Failed
    Failed,
    /// Cancelled
    Cancelled,
    /// Unknown state
    Unknown,
}

/// Trait for converting scheduler-specific states to unified JobStatus.
pub trait ToJobStatus {
    /// Convert to unified JobStatus.
    fn to_job_status(&self) -> JobStatus;

    /// Extract error information if the job failed.
    fn to_job_error(&self) -> Option<JobError>;
}

// Implementation for SLURM job states
impl ToJobStatus for charmer_slurm::SlurmJobState {
    fn to_job_status(&self) -> JobStatus {
        match self {
            Self::Pending => JobStatus::Queued,
            Self::Running => JobStatus::Running,
            Self::Completed { .. } => JobStatus::Completed,
            Self::Failed { .. } => JobStatus::Failed,
            Self::Cancelled => JobStatus::Cancelled,
            Self::Timeout => JobStatus::Failed,
            Self::OutOfMemory => JobStatus::Failed,
            Self::Unknown(_) => JobStatus::Unknown,
        }
    }

    fn to_job_error(&self) -> Option<JobError> {
        match self {
            Self::Failed { exit_code, error } => Some(JobError {
                exit_code: *exit_code,
                message: error.clone(),
            }),
            Self::Timeout => Some(JobError {
                exit_code: -1,
                message: "Job exceeded time limit".to_string(),
            }),
            Self::OutOfMemory => Some(JobError {
                exit_code: -1,
                message: "Job exceeded memory limit".to_string(),
            }),
            _ => None,
        }
    }
}

// Implementation for LSF job states
impl ToJobStatus for charmer_lsf::LsfJobState {
    fn to_job_status(&self) -> JobStatus {
        match self {
            Self::Pending => JobStatus::Queued,
            Self::Running => JobStatus::Running,
            Self::Done { .. } => JobStatus::Completed,
            Self::Exit { .. } => JobStatus::Failed,
            Self::UserSuspendedPending | Self::UserSuspended => JobStatus::Pending,
            Self::SystemSuspended => JobStatus::Pending,
            Self::Zombie => JobStatus::Unknown,
            Self::Unknown(_) => JobStatus::Unknown,
        }
    }

    fn to_job_error(&self) -> Option<JobError> {
        match self {
            Self::Exit { exit_code, error } => Some(JobError {
                exit_code: *exit_code,
                message: error.clone(),
            }),
            _ => None,
        }
    }
}

impl JobStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Pending => "○",
            Self::Queued => "◐",
            Self::Running => "●",
            Self::Completed => "✓",
            Self::Failed => "✗",
            Self::Cancelled => "⊘",
            Self::Unknown => "?",
        }
    }
}

/// Job timing information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobTiming {
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Job resource allocation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobResources {
    pub cpus: Option<u32>,
    pub memory_mb: Option<u64>,
    pub time_limit: Option<Duration>,
    pub partition: Option<String>,
    pub node: Option<String>,
}

/// Job error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobError {
    pub exit_code: i32,
    pub message: String,
}

/// Data source flags.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataSources {
    pub has_snakemake_metadata: bool,
    pub has_slurm_squeue: bool,
    pub has_slurm_sacct: bool,
    pub has_lsf_bjobs: bool,
    pub has_lsf_bhist: bool,
}

/// Unified job combining SLURM and snakemake data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Unique identifier
    pub id: String,

    /// Snakemake rule name
    pub rule: String,

    /// Wildcards as key=value string
    pub wildcards: Option<String>,

    /// Output file(s)
    pub outputs: Vec<String>,

    /// Input files
    pub inputs: Vec<String>,

    /// Current status
    pub status: JobStatus,

    /// SLURM job ID (if submitted)
    pub slurm_job_id: Option<String>,

    /// Shell command
    pub shellcmd: String,

    /// Timing information
    pub timing: JobTiming,

    /// Resource allocation
    pub resources: JobResources,

    /// Log file paths
    pub log_files: Vec<String>,

    /// Error details (if failed)
    pub error: Option<JobError>,

    /// Data sources
    pub data_sources: DataSources,
}

/// Pipeline-level state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineState {
    /// Snakemake run UUID
    pub run_uuid: Option<String>,

    /// Pipeline working directory
    pub working_dir: Utf8PathBuf,

    /// All jobs indexed by ID
    pub jobs: HashMap<String, Job>,

    /// Jobs grouped by rule
    pub jobs_by_rule: HashMap<String, Vec<String>>,

    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

impl PipelineState {
    pub fn new(working_dir: Utf8PathBuf) -> Self {
        Self {
            run_uuid: None,
            working_dir,
            jobs: HashMap::new(),
            jobs_by_rule: HashMap::new(),
            last_updated: Utc::now(),
        }
    }

    pub fn job_counts(&self) -> JobCounts {
        let mut counts = JobCounts::default();
        for job in self.jobs.values() {
            match job.status {
                JobStatus::Pending => counts.pending += 1,
                JobStatus::Queued => counts.queued += 1,
                JobStatus::Running => counts.running += 1,
                JobStatus::Completed => counts.completed += 1,
                JobStatus::Failed => counts.failed += 1,
                JobStatus::Cancelled => counts.cancelled += 1,
                JobStatus::Unknown => counts.unknown += 1,
            }
        }
        counts.total = self.jobs.len();
        counts
    }
}

#[derive(Debug, Clone, Default)]
pub struct JobCounts {
    pub total: usize,
    pub pending: usize,
    pub queued: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub unknown: usize,
}
