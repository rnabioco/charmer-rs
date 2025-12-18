//! Unified pipeline state for charmer.
//!
//! Merges data from SLURM and snakemake sources.

pub mod merge;
pub mod types;

pub use merge::{
    correlate_jobs, merge_lsf_jobs, merge_slurm_jobs, merge_snakemake_jobs,
    parse_lsf_description, parse_slurm_comment,
};
pub use types::{
    DataSources, Job, JobCounts, JobError, JobResources, JobStatus, JobTiming, PipelineState,
};
