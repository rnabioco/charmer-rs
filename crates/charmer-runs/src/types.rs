use camino::Utf8PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a pipeline run.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
    #[default]
    Unknown,
}

/// Information about a single pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInfo {
    /// Snakemake run UUID (from SLURM job name).
    pub run_uuid: String,

    /// Absolute path to working directory.
    pub working_dir: Utf8PathBuf,

    /// When the run was first detected.
    pub first_seen: DateTime<Utc>,

    /// When the run was last updated (last job activity).
    pub last_updated: DateTime<Utc>,

    /// Run status.
    pub status: RunStatus,

    /// Total jobs (from snakemake log if known).
    pub total_jobs: Option<usize>,

    /// Completed job count.
    pub completed_jobs: usize,

    /// Failed job count.
    pub failed_jobs: usize,

    /// Whether pipeline finished (100% or error).
    pub finished: bool,

    /// Host machine name (from log).
    pub host: Option<String>,
}

impl RunInfo {
    /// Create a new run with the given UUID and working directory.
    pub fn new(run_uuid: String, working_dir: Utf8PathBuf) -> Self {
        let now = Utc::now();
        Self {
            run_uuid,
            working_dir,
            first_seen: now,
            last_updated: now,
            status: RunStatus::Running,
            total_jobs: None,
            completed_jobs: 0,
            failed_jobs: 0,
            finished: false,
            host: None,
        }
    }

    /// Update job counts and derive status.
    pub fn update_counts(&mut self, completed: usize, failed: usize, total: Option<usize>) {
        self.completed_jobs = completed;
        self.failed_jobs = failed;
        self.total_jobs = total;
        self.last_updated = Utc::now();

        // Derive status from counts
        if self.finished {
            self.status = if failed > 0 {
                RunStatus::Failed
            } else {
                RunStatus::Completed
            };
        } else {
            self.status = RunStatus::Running;
        }
    }

    /// Mark the run as finished.
    pub fn mark_finished(&mut self, success: bool) {
        self.finished = true;
        self.last_updated = Utc::now();
        self.status = if success {
            RunStatus::Completed
        } else {
            RunStatus::Failed
        };
    }
}

/// Collection of pipeline runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunsState {
    pub runs: Vec<RunInfo>,
}

impl RunsState {
    /// Maximum number of runs to keep.
    const MAX_RUNS: usize = 50;

    /// Get the most recent running run, or the most recently completed run.
    pub fn current_run(&self) -> Option<&RunInfo> {
        // First look for a running run
        if let Some(run) = self.runs.iter().find(|r| r.status == RunStatus::Running) {
            return Some(run);
        }
        // Otherwise return most recently updated run
        self.runs.iter().max_by_key(|r| r.last_updated)
    }

    /// Get run by UUID.
    pub fn get_run(&self, uuid: &str) -> Option<&RunInfo> {
        self.runs.iter().find(|r| r.run_uuid == uuid)
    }

    /// Get mutable run by UUID.
    pub fn get_run_mut(&mut self, uuid: &str) -> Option<&mut RunInfo> {
        self.runs.iter_mut().find(|r| r.run_uuid == uuid)
    }

    /// Register or update a run.
    pub fn upsert_run(&mut self, run: RunInfo) {
        if let Some(existing) = self.runs.iter_mut().find(|r| r.run_uuid == run.run_uuid) {
            *existing = run;
        } else {
            self.runs.push(run);
        }
        // Sort by last_updated descending
        self.runs
            .sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
        // Keep only last N runs
        self.runs.truncate(Self::MAX_RUNS);
    }

    /// Get all run UUIDs.
    pub fn run_uuids(&self) -> Vec<&str> {
        self.runs.iter().map(|r| r.run_uuid.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_info_new() {
        let run = RunInfo::new("test-uuid".to_string(), "/tmp/test".into());
        assert_eq!(run.run_uuid, "test-uuid");
        assert_eq!(run.status, RunStatus::Running);
        assert!(!run.finished);
    }

    #[test]
    fn test_runs_state_current_run_prefers_running() {
        let mut state = RunsState::default();

        // Add a completed run
        let mut completed = RunInfo::new("completed".to_string(), "/tmp/a".into());
        completed.mark_finished(true);
        state.upsert_run(completed);

        // Add a running run
        let running = RunInfo::new("running".to_string(), "/tmp/b".into());
        state.upsert_run(running);

        // Current run should be the running one
        let current = state.current_run().unwrap();
        assert_eq!(current.run_uuid, "running");
    }

    #[test]
    fn test_runs_state_truncates() {
        let mut state = RunsState::default();
        for i in 0..60 {
            let run = RunInfo::new(format!("run-{i}"), "/tmp/test".into());
            state.upsert_run(run);
        }
        assert_eq!(state.runs.len(), RunsState::MAX_RUNS);
    }
}
