//! SLURM integration for charmer.
//!
//! Query job status via squeue and sacct.

pub mod types;
pub mod squeue;
pub mod sacct;

pub use types::{SlurmJob, SlurmJobState};
