//! Merge SLURM and snakemake data into unified state.

use crate::types::{Job, PipelineState};
use charmer_core::SnakemakeJob;
use charmer_slurm::SlurmJob;

/// Merge snakemake metadata into pipeline state.
pub fn merge_snakemake_jobs(state: &mut PipelineState, jobs: Vec<SnakemakeJob>) {
    // TODO: Implement merging logic
    for _job in jobs {
        // Create or update job entries
    }
    state.last_updated = chrono::Utc::now();
}

/// Merge SLURM jobs into pipeline state.
pub fn merge_slurm_jobs(state: &mut PipelineState, jobs: Vec<SlurmJob>) {
    // TODO: Implement merging logic
    for _job in jobs {
        // Match to existing jobs or create new entries
    }
    state.last_updated = chrono::Utc::now();
}

/// Attempt to correlate uncorrelated jobs.
pub fn correlate_jobs(state: &mut PipelineState) {
    // TODO: Match SLURM jobs to snakemake metadata
    // Using comment field parsing and timing windows
}
