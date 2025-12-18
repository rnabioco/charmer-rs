//! Snakemake metadata merging into unified state.

use crate::types::{DataSources, Job, JobResources, JobStatus, JobTiming, PipelineState};
use charmer_core::SnakemakeJob;
use chrono::{DateTime, TimeZone, Utc};

/// Convert Unix timestamp to DateTime.
fn timestamp_to_datetime(ts: f64) -> DateTime<Utc> {
    Utc.timestamp_opt(ts as i64, ((ts.fract()) * 1_000_000_000.0) as u32)
        .single()
        .unwrap_or_else(Utc::now)
}

/// Merge snakemake metadata into pipeline state.
pub fn merge_snakemake_jobs(state: &mut PipelineState, jobs: Vec<SnakemakeJob>) {
    for snakemake_job in jobs {
        let meta = &snakemake_job.metadata;

        // Generate job ID from output path or rule
        let job_id = snakemake_job.output_path.clone();

        // Determine status from metadata
        let status = if meta.incomplete {
            JobStatus::Running
        } else if meta.endtime.is_some() {
            JobStatus::Completed
        } else {
            JobStatus::Pending
        };

        // Build timing
        let timing = JobTiming {
            queued_at: None,
            started_at: Some(timestamp_to_datetime(meta.starttime)),
            completed_at: meta.endtime.map(timestamp_to_datetime),
        };

        // Check if job already exists (from SLURM data)
        if let Some(existing) = state.jobs.get_mut(&job_id) {
            // Update with snakemake-specific data
            existing.shellcmd = meta.shellcmd.clone();
            existing.inputs = meta.input.clone();
            existing.log_files = meta.log.clone();
            if existing.timing.started_at.is_none() {
                existing.timing.started_at = timing.started_at;
            }
            if existing.timing.completed_at.is_none() {
                existing.timing.completed_at = timing.completed_at;
            }
            existing.data_sources.has_snakemake_metadata = true;
        } else {
            // Create new job entry
            let job = Job {
                id: job_id.clone(),
                rule: meta.rule.clone(),
                wildcards: None, // Will be parsed from output path pattern
                outputs: vec![snakemake_job.output_path.clone()],
                inputs: meta.input.clone(),
                status,
                slurm_job_id: None,
                shellcmd: meta.shellcmd.clone(),
                timing,
                resources: JobResources::default(),
                log_files: meta.log.clone(),
                error: None,
                data_sources: DataSources {
                    has_snakemake_metadata: true,
                    has_slurm_squeue: false,
                    has_slurm_sacct: false,
                    has_lsf_bjobs: false,
                    has_lsf_bhist: false,
                },
            };
            state.jobs.insert(job_id.clone(), job);

            // Update jobs_by_rule index
            state
                .jobs_by_rule
                .entry(meta.rule.clone())
                .or_default()
                .push(job_id);
        }
    }

    state.last_updated = Utc::now();
}
