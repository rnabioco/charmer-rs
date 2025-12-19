//! Snakemake metadata types and parsing.

use base64::prelude::*;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

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

#[derive(Error, Debug)]
pub enum MetadataError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("UTF-8 decode error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("Metadata directory not found: {0}")]
    NotFound(Utf8PathBuf),
}

/// Decode a base64-encoded filename to get the output path.
pub fn decode_metadata_filename(filename: &str) -> Result<String, MetadataError> {
    let bytes = BASE64_STANDARD.decode(filename)?;
    let path = String::from_utf8(bytes)?;
    Ok(path)
}

/// Parse a single metadata file.
pub fn parse_metadata_file(path: &Utf8Path) -> Result<SnakemakeJob, MetadataError> {
    let content = std::fs::read_to_string(path)?;
    let metadata: SnakemakeMetadata = serde_json::from_str(&content)?;

    // Decode the output path from the filename
    let filename = path
        .file_name()
        .ok_or_else(|| MetadataError::NotFound(path.to_owned()))?;
    let output_path = decode_metadata_filename(filename)?;

    Ok(SnakemakeJob {
        output_path,
        metadata,
    })
}

/// Scan the .snakemake/metadata directory and parse all metadata files.
pub fn scan_metadata_dir(working_dir: &Utf8Path) -> Result<Vec<SnakemakeJob>, MetadataError> {
    let metadata_dir = working_dir.join(".snakemake").join("metadata");

    if !metadata_dir.exists() {
        return Ok(vec![]);
    }

    let mut jobs = Vec::new();

    for entry in std::fs::read_dir(&metadata_dir)? {
        let entry = entry?;
        let path = Utf8PathBuf::try_from(entry.path()).map_err(|e| {
            MetadataError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))
        })?;

        // Skip directories and non-files
        if !path.is_file() {
            continue;
        }

        // Skip hidden files
        if path
            .file_name()
            .map(|n| n.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }

        match parse_metadata_file(&path) {
            Ok(job) => jobs.push(job),
            Err(e) => {
                // Log but continue on parse errors
                tracing::warn!("Failed to parse metadata file {}: {}", path, e);
            }
        }
    }

    Ok(jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_metadata_filename() {
        // "results/test.txt" in base64
        let encoded = BASE64_STANDARD.encode("results/test.txt");
        let decoded = decode_metadata_filename(&encoded).unwrap();
        assert_eq!(decoded, "results/test.txt");
    }

    #[test]
    fn test_parse_metadata() {
        let json = r#"{
            "rule": "test_rule",
            "input": ["input.txt"],
            "log": ["log.txt"],
            "params": [],
            "shellcmd": "echo hello",
            "incomplete": false,
            "starttime": 1700000000.0,
            "endtime": 1700000100.0,
            "job_hash": 12345
        }"#;

        let meta: SnakemakeMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.rule, "test_rule");
        assert!(!meta.incomplete);
        assert_eq!(meta.endtime, Some(1700000100.0));
    }
}
