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

/// Extract wildcards from output path.
/// For paths like "results/aligned/sample1.bam" with rule "align_sample",
/// tries to extract sample=sample1 based on common patterns.
fn extract_wildcards(output_path: &str, _rule: &str) -> Option<String> {
    let parts: Vec<&str> = output_path.split('/').collect();
    if parts.len() < 2 {
        return None;
    }

    let mut wildcards = Vec::new();

    // Get the filename without extension
    if let Some(filename) = parts.last() {
        let name_parts: Vec<&str> = filename.split('.').collect();
        if let Some(base_name) = name_parts.first() {
            // Check for patterns like "sample1_chr1" -> sample=sample1, chrom=chr1
            if base_name.contains('_') {
                let segments: Vec<&str> = base_name.split('_').collect();
                if segments.len() == 2 {
                    // Heuristic: first part is sample, second is something else (chrom, etc.)
                    if segments[1].starts_with("chr") {
                        wildcards.push(format!("sample={}", segments[0]));
                        wildcards.push(format!("chrom={}", segments[1]));
                    } else {
                        wildcards.push(format!("sample={}", segments[0]));
                        wildcards.push(format!("var={}", segments[1]));
                    }
                } else if segments.len() == 1 {
                    wildcards.push(format!("sample={}", segments[0]));
                }
            } else {
                // Simple case: just a sample name
                wildcards.push(format!("sample={}", base_name));
            }
        }
    }

    if wildcards.is_empty() {
        None
    } else {
        Some(wildcards.join(", "))
    }
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
            started_at: meta.starttime.map(timestamp_to_datetime),
            completed_at: meta.endtime.map(timestamp_to_datetime),
        };

        // Extract wildcards from output path
        let wildcards = extract_wildcards(&snakemake_job.output_path, &meta.rule);

        // Check if job already exists (from SLURM data)
        if let Some(existing) = state.jobs.get_mut(&job_id) {
            // Update with snakemake-specific data
            existing.shellcmd = meta.shellcmd.clone();
            existing.inputs = meta.input.clone();
            existing.log_files = meta.log.clone();
            existing.conda_env = meta.conda_env.clone();
            existing.container_img_url = meta.container_img_url.clone();
            if existing.wildcards.is_none() {
                existing.wildcards = wildcards;
            }
            if existing.timing.started_at.is_none() {
                existing.timing.started_at = timing.started_at;
            }
            if existing.timing.completed_at.is_none() {
                existing.timing.completed_at = timing.completed_at;
            }
            existing.data_sources.has_snakemake_metadata = true;
            existing.is_snakemake_job = true; // Mark as snakemake job when metadata is found
        } else {
            // Create new job entry
            let job = Job {
                id: job_id.clone(),
                rule: meta.rule.clone(),
                wildcards,
                outputs: vec![snakemake_job.output_path.clone()],
                inputs: meta.input.clone(),
                status,
                scheduler_job_id: None,
                shellcmd: meta.shellcmd.clone(),
                timing,
                resources: JobResources::default(),
                usage: None,
                log_files: meta.log.clone(),
                error: None,
                conda_env: meta.conda_env.clone(),
                container_img_url: meta.container_img_url.clone(),
                data_sources: DataSources {
                    has_snakemake_metadata: true,
                    has_slurm_squeue: false,
                    has_slurm_sacct: false,
                    has_lsf_bjobs: false,
                    has_lsf_bhist: false,
                },
                is_target: false,
                is_snakemake_job: true, // Jobs from snakemake metadata are always snakemake jobs
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
