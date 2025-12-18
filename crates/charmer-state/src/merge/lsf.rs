//! LSF job merging into unified state.

use super::comment::{make_job_id, parse_lsf_description};
use crate::types::{DataSources, Job, JobResources, JobTiming, PipelineState, ToJobStatus};
use charmer_lsf::LsfJob;
use chrono::Utc;

/// Merge LSF jobs into pipeline state.
pub fn merge_lsf_jobs(state: &mut PipelineState, jobs: Vec<LsfJob>, from_bhist: bool) {
    for lsf_job in jobs {
        // Try to parse rule info from description
        let (rule, wildcards) = lsf_job
            .description
            .as_ref()
            .and_then(|d| parse_lsf_description(d))
            .unwrap_or_else(|| (lsf_job.name.clone(), None));

        let job_id = make_job_id(&rule, wildcards.as_deref());

        // Update run_uuid if this is the first job
        if state.run_uuid.is_none() {
            state.run_uuid = Some(lsf_job.name.clone());
        }

        // Convert LSF state using the trait
        let status = lsf_job.state.to_job_status();
        let error = lsf_job.state.to_job_error();

        // Build timing
        let timing = JobTiming {
            queued_at: lsf_job.submit_time,
            started_at: lsf_job.start_time,
            completed_at: lsf_job.end_time,
        };

        // Build resources
        let resources = JobResources {
            cpus: lsf_job.nprocs,
            memory_mb: lsf_job.mem_limit_mb,
            time_limit: lsf_job.run_limit,
            partition: lsf_job.queue.clone(), // LSF queue = SLURM partition
            node: lsf_job.exec_host.clone(),
        };

        // Check if job already exists
        if let Some(existing) = state.jobs.get_mut(&job_id) {
            // Update with LSF data
            existing.slurm_job_id = Some(lsf_job.job_id.clone()); // Reuse field for LSF job ID
            existing.status = status;
            existing.resources = resources;
            existing.error = error;
            if existing.timing.queued_at.is_none() {
                existing.timing.queued_at = timing.queued_at;
            }
            if from_bhist {
                existing.data_sources.has_lsf_bhist = true;
            } else {
                existing.data_sources.has_lsf_bjobs = true;
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
                slurm_job_id: Some(lsf_job.job_id.clone()), // Reuse for LSF job ID
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
                    has_slurm_squeue: false,
                    has_slurm_sacct: false,
                    has_lsf_bjobs: !from_bhist,
                    has_lsf_bhist: from_bhist,
                },
                is_target: false,
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
