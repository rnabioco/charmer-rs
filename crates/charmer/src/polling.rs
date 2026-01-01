//! Background polling service for SLURM and LSF schedulers.

use charmer_lsf::{query_bhist, query_bjobs};
use charmer_slurm::{query_resource_usage, query_sacct, query_squeue};
use charmer_state::{
    FailureAnalysis, FailureMode, JobStatus, PipelineState, ResourceUsage, merge_lsf_jobs,
    merge_slurm_jobs,
};
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::interval;

/// Scheduler type detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerType {
    Slurm,
    Lsf,
}

/// Detect which scheduler is available.
pub async fn detect_scheduler() -> Option<SchedulerType> {
    // Try SLURM first
    if tokio::process::Command::new("squeue")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Some(SchedulerType::Slurm);
    }

    // Try LSF
    if tokio::process::Command::new("bjobs")
        .arg("-V")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Some(SchedulerType::Lsf);
    }

    None
}

/// Configuration for the polling service.
#[derive(Debug, Clone)]
pub struct PollingConfig {
    /// Interval for polling active jobs (squeue/bjobs).
    pub active_poll_interval: Duration,
    /// Interval for polling historical jobs (sacct/bhist).
    pub history_poll_interval: Duration,
    /// Run UUID filter (optional).
    pub run_uuid: Option<String>,
    /// Hours of history to fetch.
    pub history_hours: u64,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            active_poll_interval: Duration::from_secs(5),
            history_poll_interval: Duration::from_secs(30),
            run_uuid: None,
            history_hours: 24,
        }
    }
}

/// Polling service that runs in the background.
pub struct PollingService {
    state: Arc<Mutex<PipelineState>>,
    config: PollingConfig,
    scheduler: SchedulerType,
}

impl PollingService {
    pub fn new(
        state: Arc<Mutex<PipelineState>>,
        config: PollingConfig,
        scheduler: SchedulerType,
    ) -> Self {
        Self {
            state,
            config,
            scheduler,
        }
    }

    /// Start the polling service in the background.
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    /// Main polling loop.
    async fn run(self) {
        let mut active_ticker = interval(self.config.active_poll_interval);
        let mut history_ticker = interval(self.config.history_poll_interval);

        // Skip the first tick (fires immediately)
        active_ticker.tick().await;
        history_ticker.tick().await;

        loop {
            tokio::select! {
                _ = active_ticker.tick() => {
                    self.poll_active_jobs().await;
                }
                _ = history_ticker.tick() => {
                    self.poll_historical_jobs().await;
                }
            }
        }
    }

    /// Poll active jobs (squeue or bjobs).
    async fn poll_active_jobs(&self) {
        match self.scheduler {
            SchedulerType::Slurm => {
                if let Err(e) = self.poll_squeue().await {
                    tracing::error!("Error polling squeue: {}", e);
                }
            }
            SchedulerType::Lsf => {
                if let Err(e) = self.poll_bjobs().await {
                    tracing::error!("Error polling bjobs: {}", e);
                }
            }
        }
    }

    /// Poll historical jobs (sacct or bhist).
    async fn poll_historical_jobs(&self) {
        match self.scheduler {
            SchedulerType::Slurm => {
                if let Err(e) = self.poll_sacct().await {
                    tracing::error!("Error polling sacct: {}", e);
                }
            }
            SchedulerType::Lsf => {
                if let Err(e) = self.poll_bhist().await {
                    tracing::error!("Error polling bhist: {}", e);
                }
            }
        }
    }

    /// Poll SLURM squeue.
    async fn poll_squeue(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let run_uuid = self.config.run_uuid.as_deref();
        let jobs = query_squeue(run_uuid).await?;

        let mut state = self.state.lock().await;
        merge_slurm_jobs(&mut state, jobs, false);

        Ok(())
    }

    /// Poll SLURM sacct.
    async fn poll_sacct(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let run_uuid = self.config.run_uuid.as_deref();
        let since = Some(Utc::now() - chrono::Duration::hours(self.config.history_hours as i64));
        let jobs = query_sacct(run_uuid, since).await?;

        let mut state = self.state.lock().await;
        merge_slurm_jobs(&mut state, jobs, true);

        // Enrich failed jobs with failure analysis
        self.enrich_failed_jobs_slurm(&mut state).await;

        // Enrich completed jobs with resource usage
        self.enrich_completed_jobs_slurm(&mut state).await;

        Ok(())
    }

    /// Poll LSF bjobs.
    async fn poll_bjobs(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let job_name_filter = self.config.run_uuid.as_deref();
        let jobs = query_bjobs(job_name_filter).await?;

        let mut state = self.state.lock().await;
        merge_lsf_jobs(&mut state, jobs, false);

        Ok(())
    }

    /// Poll LSF bhist.
    async fn poll_bhist(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let job_name_filter = self.config.run_uuid.as_deref();
        let since = Some(Utc::now() - chrono::Duration::hours(self.config.history_hours as i64));
        let jobs = query_bhist(job_name_filter, since).await?;

        let mut state = self.state.lock().await;
        merge_lsf_jobs(&mut state, jobs, true);

        // Enrich failed jobs with failure analysis
        self.enrich_failed_jobs_lsf(&mut state).await;

        Ok(())
    }

    /// Enrich failed SLURM jobs with detailed failure analysis.
    async fn enrich_failed_jobs_slurm(&self, state: &mut PipelineState) {
        // Collect job IDs that need failure analysis
        let jobs_needing_analysis: Vec<(String, String)> = state
            .jobs
            .iter()
            .filter(|(_, job)| {
                job.status == JobStatus::Failed
                    && job.scheduler_job_id.is_some()
                    && job
                        .error
                        .as_ref()
                        .map(|e| e.analysis.is_none())
                        .unwrap_or(true)
            })
            .map(|(id, job)| (id.clone(), job.scheduler_job_id.clone().unwrap()))
            .take(5) // Limit to avoid too many queries
            .collect();

        // Analyze each failed job
        for (job_id, scheduler_job_id) in jobs_needing_analysis {
            if let Ok(analysis) = charmer_slurm::analyze_failure(&scheduler_job_id).await
                && let Some(job) = state.jobs.get_mut(&job_id)
            {
                // Convert SLURM analysis to unified format
                let unified_analysis = convert_slurm_analysis(&analysis);

                if let Some(ref mut error) = job.error {
                    error.analysis = Some(unified_analysis);
                } else {
                    // Create error with analysis
                    job.error = Some(charmer_state::JobError {
                        exit_code: match &analysis.mode {
                            charmer_slurm::FailureMode::ExitCode { code, .. } => *code,
                            _ => -1,
                        },
                        message: analysis.explanation.clone(),
                        analysis: Some(unified_analysis),
                    });
                }
            }
        }
    }

    /// Enrich completed SLURM jobs with resource usage data.
    async fn enrich_completed_jobs_slurm(&self, state: &mut PipelineState) {
        // Collect job IDs that need resource usage data
        let jobs_needing_usage: Vec<(String, String)> = state
            .jobs
            .iter()
            .filter(|(_, job)| {
                // Get usage for completed and failed jobs that don't have it yet
                matches!(job.status, JobStatus::Completed | JobStatus::Failed)
                    && job.scheduler_job_id.is_some()
                    && job.usage.is_none()
            })
            .map(|(id, job)| (id.clone(), job.scheduler_job_id.clone().unwrap()))
            .take(10) // Limit to avoid too many queries per poll
            .collect();

        // Query resource usage for each job
        for (job_id, scheduler_job_id) in jobs_needing_usage {
            if let Ok(Some(usage)) = query_resource_usage(&scheduler_job_id).await
                && let Some(job) = state.jobs.get_mut(&job_id)
            {
                job.usage = Some(ResourceUsage {
                    max_rss_mb: usage.max_rss_mb,
                    elapsed_seconds: usage.elapsed_seconds,
                    cpu_time_seconds: usage.cpu_time_seconds,
                });
            }
        }
    }

    /// Enrich failed LSF jobs with detailed failure analysis.
    async fn enrich_failed_jobs_lsf(&self, state: &mut PipelineState) {
        // Collect job IDs that need failure analysis
        let jobs_needing_analysis: Vec<(String, String)> = state
            .jobs
            .iter()
            .filter(|(_, job)| {
                job.status == JobStatus::Failed
                    && job.scheduler_job_id.is_some()
                    && job
                        .error
                        .as_ref()
                        .map(|e| e.analysis.is_none())
                        .unwrap_or(true)
            })
            .map(|(id, job)| (id.clone(), job.scheduler_job_id.clone().unwrap()))
            .take(5) // Limit to avoid too many queries
            .collect();

        // Analyze each failed job
        for (job_id, lsf_job_id) in jobs_needing_analysis {
            if let Ok(analysis) = charmer_lsf::analyze_failure(&lsf_job_id).await
                && let Some(job) = state.jobs.get_mut(&job_id)
            {
                // Convert LSF analysis to unified format
                let unified_analysis = convert_lsf_analysis(&analysis);

                if let Some(ref mut error) = job.error {
                    error.analysis = Some(unified_analysis);
                } else {
                    // Create error with analysis
                    job.error = Some(charmer_state::JobError {
                        exit_code: match &analysis.mode {
                            charmer_lsf::FailureMode::ExitCode { code, .. } => *code,
                            _ => -1,
                        },
                        message: analysis.explanation.clone(),
                        analysis: Some(unified_analysis),
                    });
                }
            }
        }
    }
}

/// Convert SLURM failure analysis to unified format.
fn convert_slurm_analysis(analysis: &charmer_slurm::FailureAnalysis) -> FailureAnalysis {
    let mode = match &analysis.mode {
        charmer_slurm::FailureMode::OutOfMemory { .. } => FailureMode::OutOfMemory,
        charmer_slurm::FailureMode::Timeout { .. } => FailureMode::Timeout,
        charmer_slurm::FailureMode::ExitCode { .. } => FailureMode::ExitCode,
        charmer_slurm::FailureMode::Cancelled { .. } => FailureMode::Cancelled,
        charmer_slurm::FailureMode::NodeFailure { .. } => FailureMode::NodeFailure,
        charmer_slurm::FailureMode::Unknown { .. } => FailureMode::Unknown,
    };

    let (memory_used_mb, memory_limit_mb) = match &analysis.mode {
        charmer_slurm::FailureMode::OutOfMemory {
            used_mb,
            requested_mb,
            ..
        } => (Some(*used_mb), Some(*requested_mb)),
        _ => (analysis.max_rss_mb, analysis.req_mem_mb),
    };

    let (runtime_seconds, time_limit_seconds) = match &analysis.mode {
        charmer_slurm::FailureMode::Timeout {
            elapsed_seconds,
            limit_seconds,
            ..
        } => (Some(*elapsed_seconds), Some(*limit_seconds)),
        _ => (analysis.elapsed_seconds, analysis.time_limit_seconds),
    };

    FailureAnalysis {
        mode,
        explanation: analysis.explanation.clone(),
        suggestion: analysis.suggestion.clone(),
        memory_used_mb,
        memory_limit_mb,
        runtime_seconds,
        time_limit_seconds,
    }
}

/// Convert LSF failure analysis to unified format.
fn convert_lsf_analysis(analysis: &charmer_lsf::FailureAnalysis) -> FailureAnalysis {
    let mode = match &analysis.mode {
        charmer_lsf::FailureMode::OutOfMemory { .. } => FailureMode::OutOfMemory,
        charmer_lsf::FailureMode::Timeout { .. } => FailureMode::Timeout,
        charmer_lsf::FailureMode::ExitCode { .. } => FailureMode::ExitCode,
        charmer_lsf::FailureMode::Killed { .. } => FailureMode::Cancelled,
        charmer_lsf::FailureMode::HostFailure { .. } => FailureMode::NodeFailure,
        charmer_lsf::FailureMode::Unknown { .. } => FailureMode::Unknown,
    };

    let (memory_used_mb, memory_limit_mb) = match &analysis.mode {
        charmer_lsf::FailureMode::OutOfMemory {
            used_mb, limit_mb, ..
        } => (Some(*used_mb), Some(*limit_mb)),
        _ => (analysis.max_mem_mb, analysis.mem_limit_mb),
    };

    let (runtime_seconds, time_limit_seconds) = match &analysis.mode {
        charmer_lsf::FailureMode::Timeout {
            elapsed_seconds,
            limit_seconds,
            ..
        } => (Some(*elapsed_seconds), Some(*limit_seconds)),
        _ => (analysis.run_time_seconds, analysis.run_limit_seconds),
    };

    FailureAnalysis {
        mode,
        explanation: analysis.explanation.clone(),
        suggestion: analysis.suggestion.clone(),
        memory_used_mb,
        memory_limit_mb,
        runtime_seconds,
        time_limit_seconds,
    }
}

/// Initialize polling service and return a handle to it.
pub async fn init_polling(
    state: Arc<Mutex<PipelineState>>,
    config: PollingConfig,
) -> Option<tokio::task::JoinHandle<()>> {
    // Detect scheduler
    let scheduler = detect_scheduler().await?;

    tracing::info!(
        "Detected scheduler: {:?}, polling every {} seconds",
        scheduler,
        config.active_poll_interval.as_secs()
    );

    // Create and start the polling service
    let service = PollingService::new(state, config, scheduler);
    Some(service.start())
}
