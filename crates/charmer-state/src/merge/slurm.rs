//! SLURM job merging into unified state.

use super::comment::{make_job_id, parse_slurm_comment};
use crate::types::{DataSources, Job, JobResources, JobTiming, PipelineState, ToJobStatus};
use charmer_slurm::SlurmJob;
use chrono::Utc;

/// Merge SLURM jobs into pipeline state.
pub fn merge_slurm_jobs(state: &mut PipelineState, jobs: Vec<SlurmJob>, from_sacct: bool) {
    for slurm_job in jobs {
        // Try to parse rule info from comment
        let parsed = slurm_job
            .comment
            .as_ref()
            .and_then(|c| parse_slurm_comment(c));

        // Job is a snakemake job if comment parsing succeeded (has rule_ prefix)
        let is_snakemake_job = parsed.is_some();

        let (rule, wildcards) = parsed.unwrap_or_else(|| (slurm_job.name.clone(), None));

        let job_id = make_job_id(&rule, wildcards.as_deref());

        // Update run_uuid if this is the first job
        if state.run_uuid.is_none() {
            state.run_uuid = Some(slurm_job.name.clone());
        }

        // Convert SLURM state using the trait
        let status = slurm_job.state.to_job_status();
        let error = slurm_job.state.to_job_error();

        // Build timing
        let timing = JobTiming {
            queued_at: slurm_job.submit_time,
            started_at: slurm_job.start_time,
            completed_at: slurm_job.end_time,
        };

        // Build resources
        let resources = JobResources {
            cpus: slurm_job.cpus,
            memory_mb: slurm_job.mem_mb,
            time_limit: slurm_job.time_limit,
            partition: slurm_job.partition.clone(),
            node: slurm_job.nodelist.clone(),
        };

        // Check if job already exists
        if let Some(existing) = state.jobs.get_mut(&job_id) {
            // Update with SLURM data
            existing.scheduler_job_id = Some(slurm_job.job_id.clone());
            existing.status = status;
            existing.resources = resources;
            existing.error = error;
            if existing.timing.queued_at.is_none() {
                existing.timing.queued_at = timing.queued_at;
            }
            if from_sacct {
                existing.data_sources.has_slurm_sacct = true;
            } else {
                existing.data_sources.has_slurm_squeue = true;
            }
        } else {
            // Create new job entry
            let job = Job {
                id: job_id.clone(),
                rule,
                wildcards,
                outputs: vec![],
                inputs: vec![],
                status,
                scheduler_job_id: Some(slurm_job.job_id.clone()),
                shellcmd: String::new(),
                timing,
                resources,
                usage: None,
                log_files: vec![],
                error,
                conda_env: None,
                container_img_url: None,
                data_sources: DataSources {
                    has_snakemake_metadata: false,
                    has_slurm_squeue: !from_sacct,
                    has_slurm_sacct: from_sacct,
                    has_lsf_bjobs: false,
                    has_lsf_bhist: false,
                },
                is_target: false,
                is_snakemake_job,
            };

            let rule_name = job.rule.clone();
            state.jobs.insert(job_id.clone(), job);

            // Update jobs_by_rule index
            state
                .jobs_by_rule
                .entry(rule_name)
                .or_default()
                .push(job_id);
        }
    }

    state.last_updated = Utc::now();
}
