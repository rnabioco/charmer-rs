//! Background polling service for SLURM and LSF schedulers.

use charmer_lsf::{query_bhist, query_bjobs};
use charmer_slurm::{query_sacct, query_squeue};
use charmer_state::{merge_lsf_jobs, merge_slurm_jobs, PipelineState};
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
                    eprintln!("Error polling squeue: {}", e);
                }
            }
            SchedulerType::Lsf => {
                if let Err(e) = self.poll_bjobs().await {
                    eprintln!("Error polling bjobs: {}", e);
                }
            }
        }
    }

    /// Poll historical jobs (sacct or bhist).
    async fn poll_historical_jobs(&self) {
        match self.scheduler {
            SchedulerType::Slurm => {
                if let Err(e) = self.poll_sacct().await {
                    eprintln!("Error polling sacct: {}", e);
                }
            }
            SchedulerType::Lsf => {
                if let Err(e) = self.poll_bhist().await {
                    eprintln!("Error polling bhist: {}", e);
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

        Ok(())
    }
}

/// Initialize polling service and return a handle to it.
pub async fn init_polling(
    state: Arc<Mutex<PipelineState>>,
    config: PollingConfig,
) -> Option<tokio::task::JoinHandle<()>> {
    // Detect scheduler
    let scheduler = detect_scheduler().await?;

    eprintln!(
        "Detected scheduler: {:?}, polling every {} seconds",
        scheduler,
        config.active_poll_interval.as_secs()
    );

    // Create and start the polling service
    let service = PollingService::new(state, config, scheduler);
    Some(service.start())
}
