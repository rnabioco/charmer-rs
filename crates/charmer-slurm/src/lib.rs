//! SLURM integration for charmer.
//!
//! Query job status via squeue and sacct.

pub mod failure;
pub mod sacct;
pub mod squeue;
pub mod types;

pub use failure::{FailureAnalysis, FailureError, FailureMode, analyze_failure};
pub use sacct::{SacctError, SlurmResourceUsage, query_resource_usage, query_sacct};
pub use squeue::{SqueueError, query_squeue};
pub use types::{SlurmJob, SlurmJobState};
