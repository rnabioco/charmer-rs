//! Merge SLURM, LSF, and snakemake data into unified state.

mod comment;
mod correlation;
mod lsf;
mod slurm;
mod snakemake;

pub use comment::{make_job_id, parse_lsf_description, parse_slurm_comment};
pub use correlation::correlate_jobs;
pub use lsf::merge_lsf_jobs;
pub use slurm::merge_slurm_jobs;
pub use snakemake::merge_snakemake_jobs;
