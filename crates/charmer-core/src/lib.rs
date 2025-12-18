//! Snakemake metadata parsing for charmer.
//!
//! This crate handles parsing of `.snakemake/metadata/` files.

pub mod metadata;

pub use metadata::{
    decode_metadata_filename, parse_metadata_file, scan_metadata_dir, MetadataError,
    SnakemakeJob, SnakemakeMetadata,
};
