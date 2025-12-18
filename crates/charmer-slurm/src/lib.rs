//! SLURM integration for charmer.
//!
//! Query job status via squeue and sacct.

pub mod failure;
pub mod sacct;
pub mod squeue;
pub mod types;

pub use failure::{analyze_failure, FailureAnalysis, FailureError, FailureMode};
pub use sacct::{query_resource_usage, query_sacct, SacctError, SlurmResourceUsage};
pub use squeue::{query_squeue, SqueueError};
pub use types::{SlurmJob, SlurmJobState};
