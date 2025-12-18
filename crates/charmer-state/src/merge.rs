//! Merge SLURM and snakemake data into unified state.

use crate::types::{DataSources, Job, JobError, JobResources, JobStatus, JobTiming, PipelineState};
use charmer_core::SnakemakeJob;
use charmer_slurm::{SlurmJob, SlurmJobState};
use chrono::{DateTime, TimeZone, Utc};

/// Parse snakemake SLURM comment field: "rule_{rulename}_wildcards_{wildcards}"
pub fn parse_slurm_comment(comment: &str) -> Option<(String, Option<String>)> {
    // Format: "rule_RULENAME_wildcards_WILDCARDS" or just "rule_RULENAME"
    if !comment.starts_with("rule_") {
        return None;
    }

    let rest = &comment[5..]; // Skip "rule_"

    if let Some(wc_pos) = rest.find("_wildcards_") {
        let rule = &rest[..wc_pos];
        let wildcards = &rest[wc_pos + 11..]; // Skip "_wildcards_"
        Some((
            rule.to_string(),
            if wildcards.is_empty() {
                None
            } else {
                Some(wildcards.to_string())
            },
        ))
    } else {
        Some((rest.to_string(), None))
    }
}

/// Generate a job ID from rule and wildcards.
fn make_job_id(rule: &str, wildcards: Option<&str>) -> String {
    match wildcards {
        Some(wc) => format!("{}[{}]", rule, wc),
        None => rule.to_string(),
    }
}

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

/// Merge SLURM jobs into pipeline state.
pub fn merge_slurm_jobs(state: &mut PipelineState, jobs: Vec<SlurmJob>, from_sacct: bool) {
    for slurm_job in jobs {
        // Try to parse rule info from comment
        let (rule, wildcards) = slurm_job
            .comment
            .as_ref()
            .and_then(|c| parse_slurm_comment(c))
            .unwrap_or_else(|| (slurm_job.name.clone(), None));

        let job_id = make_job_id(&rule, wildcards.as_deref());

        // Update run_uuid if this is the first job
        if state.run_uuid.is_none() {
            state.run_uuid = Some(slurm_job.name.clone());
        }

        // Convert SLURM state to JobStatus
        let status = match &slurm_job.state {
            SlurmJobState::Pending => JobStatus::Queued,
            SlurmJobState::Running => JobStatus::Running,
            SlurmJobState::Completed { .. } => JobStatus::Completed,
            SlurmJobState::Failed { .. } => JobStatus::Failed,
            SlurmJobState::Cancelled => JobStatus::Cancelled,
            SlurmJobState::Timeout => JobStatus::Failed,
            SlurmJobState::OutOfMemory => JobStatus::Failed,
            SlurmJobState::Unknown(_) => JobStatus::Unknown,
        };

        // Build error info
        let error = match &slurm_job.state {
            SlurmJobState::Failed { exit_code, error } => Some(JobError {
                exit_code: *exit_code,
                message: error.clone(),
            }),
            SlurmJobState::Timeout => Some(JobError {
                exit_code: -1,
                message: "Job exceeded time limit".to_string(),
            }),
            SlurmJobState::OutOfMemory => Some(JobError {
                exit_code: -1,
                message: "Job exceeded memory limit".to_string(),
            }),
            _ => None,
        };

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
            existing.slurm_job_id = Some(slurm_job.job_id.clone());
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
                slurm_job_id: Some(slurm_job.job_id.clone()),
                shellcmd: String::new(),
                timing,
                resources,
                log_files: vec![],
                error,
                data_sources: DataSources {
                    has_snakemake_metadata: false,
                    has_slurm_squeue: !from_sacct,
                    has_slurm_sacct: from_sacct,
                },
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

/// Attempt to correlate jobs that couldn't be matched by comment field.
/// Uses timing windows and rule name matching.
pub fn correlate_jobs(state: &mut PipelineState) {
    // Find jobs that have SLURM data but no snakemake metadata
    let slurm_only: Vec<_> = state
        .jobs
        .values()
        .filter(|j| j.data_sources.has_slurm_squeue && !j.data_sources.has_snakemake_metadata)
        .map(|j| j.id.clone())
        .collect();

    // Find jobs that have snakemake metadata but no SLURM data
    let snakemake_only: Vec<_> = state
        .jobs
        .values()
        .filter(|j| j.data_sources.has_snakemake_metadata && !j.data_sources.has_slurm_squeue)
        .map(|j| j.id.clone())
        .collect();

    // Try to match by rule name and timing
    for slurm_id in slurm_only {
        if let Some(slurm_job) = state.jobs.get(&slurm_id) {
            let slurm_start = slurm_job.timing.started_at;

            for snakemake_id in &snakemake_only {
                if let Some(sm_job) = state.jobs.get(snakemake_id) {
                    // Match by rule name
                    if slurm_job.rule != sm_job.rule {
                        continue;
                    }

                    // Match by timing (within 60 second window)
                    if let (Some(slurm_t), Some(sm_t)) = (slurm_start, sm_job.timing.started_at) {
                        let diff = (slurm_t - sm_t).num_seconds().abs();
                        if diff <= 60 {
                            // Found a match - merge the data
                            // In a real implementation, we'd merge these entries
                            // For now, just log the correlation
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slurm_comment() {
        // Basic rule only
        let (rule, wc) = parse_slurm_comment("rule_align_reads").unwrap();
        assert_eq!(rule, "align_reads");
        assert!(wc.is_none());

        // Rule with wildcards
        let (rule, wc) = parse_slurm_comment("rule_align_reads_wildcards_sample=S1").unwrap();
        assert_eq!(rule, "align_reads");
        assert_eq!(wc.unwrap(), "sample=S1");

        // Invalid format
        assert!(parse_slurm_comment("not_a_rule").is_none());
    }

    #[test]
    fn test_make_job_id() {
        assert_eq!(make_job_id("align", None), "align");
        assert_eq!(make_job_id("align", Some("sample=S1")), "align[sample=S1]");
    }
}
