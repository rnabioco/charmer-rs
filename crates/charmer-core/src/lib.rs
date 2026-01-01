//! Snakemake metadata parsing for charmer.
//!
//! This crate handles parsing of `.snakemake/metadata/` files
//! and the main snakemake log file.

pub mod main_log;
pub mod metadata;

pub use main_log::{SnakemakeLogInfo, find_latest_log, parse_log_file, parse_main_log};
pub use metadata::{
    MetadataError, SnakemakeJob, SnakemakeMetadata, decode_metadata_filename, parse_metadata_file,
    scan_metadata_dir,
};
