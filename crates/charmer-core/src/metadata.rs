//! Snakemake metadata types and parsing.

use base64::prelude::*;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Deserialize a string field that may be null, defaulting to empty string.
fn deserialize_nullable_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

/// Snakemake job metadata from .snakemake/metadata/{base64_output_file}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SnakemakeMetadata {
    /// Rule name
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
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
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub shellcmd: String,

    /// Whether the job is incomplete (still running)
    #[serde(default)]
    pub incomplete: bool,

    /// Start timestamp (Unix epoch) - None if not yet started
    pub starttime: Option<f64>,

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

use std::collections::HashSet;
use std::time::SystemTime;

/// Result of incremental scan - contains parsed jobs and tracking info.
#[derive(Debug)]
pub struct IncrementalScanResult {
    /// New or modified jobs that were parsed.
    pub jobs: Vec<SnakemakeJob>,
    /// Total files in metadata directory.
    pub total_files: usize,
    /// Files skipped due to unchanged mtime.
    pub files_skipped: usize,
}

/// Scan metadata directory incrementally, only parsing changed files.
///
/// # Arguments
/// * `working_dir` - The pipeline working directory
/// * `mtime_cache` - Mutable reference to the mtime cache (updated in place)
///
/// # Returns
/// * `IncrementalScanResult` with parsed jobs and metadata about the scan
pub fn scan_metadata_dir_incremental(
    working_dir: &Utf8Path,
    mtime_cache: &mut HashMap<String, SystemTime>,
) -> Result<IncrementalScanResult, MetadataError> {
    let metadata_dir = working_dir.join(".snakemake").join("metadata");

    if !metadata_dir.exists() {
        return Ok(IncrementalScanResult {
            jobs: vec![],
            total_files: 0,
            files_skipped: 0,
        });
    }

    let mut jobs = Vec::new();
    let mut current_files: HashSet<String> = HashSet::new();
    let mut files_skipped = 0;
    let mut total_files = 0;

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

        total_files += 1;
        let path_str = path.to_string();
        current_files.insert(path_str.clone());

        // Get file mtime
        let mtime = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("Failed to get mtime for {}: {}", path, e);
                // Parse anyway if we can't get mtime
                match parse_metadata_file(&path) {
                    Ok(job) => jobs.push(job),
                    Err(e) => tracing::warn!("Failed to parse metadata file {}: {}", path, e),
                }
                continue;
            }
        };

        // Check if file has changed
        let should_parse = match mtime_cache.get(&path_str) {
            Some(cached_mtime) => *cached_mtime != mtime,
            None => true, // New file
        };

        if should_parse {
            match parse_metadata_file(&path) {
                Ok(job) => {
                    jobs.push(job);
                    mtime_cache.insert(path_str, mtime);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse metadata file {}: {}", path, e);
                    // Don't cache failed parses - retry next time
                }
            }
        } else {
            files_skipped += 1;
        }
    }

    // Remove deleted files from cache
    let deleted_paths: Vec<String> = mtime_cache
        .keys()
        .filter(|path| !current_files.contains(*path))
        .cloned()
        .collect();
    for path in deleted_paths {
        mtime_cache.remove(&path);
    }

    Ok(IncrementalScanResult {
        jobs,
        total_files,
        files_skipped,
    })
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
        assert_eq!(meta.starttime, Some(1700000000.0));
        assert_eq!(meta.endtime, Some(1700000100.0));
    }

    #[test]
    fn test_parse_metadata_with_nulls() {
        // Test that null values are handled gracefully
        let json = r#"{
            "rule": null,
            "input": [],
            "log": [],
            "params": [],
            "shellcmd": "",
            "incomplete": false,
            "starttime": null,
            "endtime": null,
            "job_hash": 0
        }"#;

        let meta: SnakemakeMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.rule, "");
        assert_eq!(meta.starttime, None);
        assert_eq!(meta.endtime, None);
    }
}
