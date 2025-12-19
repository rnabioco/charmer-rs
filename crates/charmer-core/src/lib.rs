//! Snakemake metadata parsing for charmer.
//!
//! This crate handles parsing of `.snakemake/metadata/` files
//! and the main snakemake log file.

pub mod main_log;
pub mod metadata;

pub use main_log::{find_latest_log, parse_log_file, parse_main_log, SnakemakeLogInfo};
pub use metadata::{
    decode_metadata_filename, parse_metadata_file, scan_metadata_dir, MetadataError, SnakemakeJob,
    SnakemakeMetadata,
};
