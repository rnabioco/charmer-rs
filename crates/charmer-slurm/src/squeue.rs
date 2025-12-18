//! Query active SLURM jobs via squeue.

use crate::types::{SlurmJob, SlurmJobState};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SqueueError {
    #[error("Failed to execute squeue: {0}")]
    ExecutionError(String),
    #[error("Failed to parse squeue output: {0}")]
    ParseError(String),
}

/// Query active jobs with squeue.
pub async fn query_squeue(_run_uuid: Option<&str>) -> Result<Vec<SlurmJob>, SqueueError> {
    // TODO: Implement squeue parsing
    // squeue -u $USER -h -o "%A|%j|%T|%P|%V|%S|%e|%N|%C|%m|%l|%k"
    Ok(vec![])
}
