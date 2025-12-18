//! Query SLURM job history via sacct.

use crate::types::{SlurmJob, SlurmJobState};
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SacctError {
    #[error("Failed to execute sacct: {0}")]
    ExecutionError(String),
    #[error("Failed to parse sacct output: {0}")]
    ParseError(String),
}

/// Query job history with sacct.
pub async fn query_sacct(
    _run_uuid: Option<&str>,
    _since: Option<DateTime<Utc>>,
) -> Result<Vec<SlurmJob>, SacctError> {
    // TODO: Implement sacct parsing
    // sacct -X --parsable2 --noheader --format=JobIDRaw,JobName,State,...
    Ok(vec![])
}
