//! Snakemake metadata types and parsing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Snakemake job metadata from .snakemake/metadata/{base64_output_file}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SnakemakeMetadata {
    /// Rule name
    pub rule: String,

    /// Input files
    #[serde(default)]
    pub input: Vec<String>,

    /// Log files
    #[serde(default)]
    pub log: Vec<String>,

    /// Rule parameters
    #[serde(default)]
    pub params: Vec<String>,

    /// Actual shell command executed
    #[serde(default)]
    pub shellcmd: String,

    /// Whether the job is incomplete (still running)
    #[serde(default)]
    pub incomplete: bool,

    /// Start timestamp (Unix epoch)
    pub starttime: f64,

    /// End timestamp (Unix epoch) - None if still running
    pub endtime: Option<f64>,

    /// Job hash for reproducibility
    #[serde(default)]
    pub job_hash: u64,

    /// Conda environment used (if any)
    pub conda_env: Option<String>,

    /// Container image URL (if any)
    pub container_img_url: Option<String>,

    /// Input file checksums for cache invalidation
    #[serde(default)]
    pub input_checksums: HashMap<String, String>,
}

/// Parsed snakemake job with decoded output path.
#[derive(Debug, Clone)]
pub struct SnakemakeJob {
    /// Output file path (decoded from metadata filename)
    pub output_path: String,

    /// Metadata from JSON
    pub metadata: SnakemakeMetadata,
}
