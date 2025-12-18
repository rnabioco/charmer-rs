//! SLURM integration for charmer.
//!
//! Query job status via squeue and sacct.

pub mod sacct;
pub mod squeue;
pub mod types;

pub use sacct::{query_sacct, SacctError};
pub use squeue::{query_squeue, SqueueError};
pub use types::{SlurmJob, SlurmJobState};
