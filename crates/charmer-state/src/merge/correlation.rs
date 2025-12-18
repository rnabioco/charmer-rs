//! Job correlation between different data sources.

use crate::types::PipelineState;

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
