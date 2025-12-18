//! Unified pipeline state for charmer.
//!
//! Merges data from SLURM and snakemake sources.

pub mod types;
pub mod merge;

pub use types::{Job, JobStatus, JobTiming, JobResources, PipelineState};
