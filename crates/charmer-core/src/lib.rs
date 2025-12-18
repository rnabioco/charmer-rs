//! Snakemake metadata parsing for charmer.
//!
//! This crate handles parsing of `.snakemake/metadata/` files.

pub mod metadata;

pub use metadata::{SnakemakeJob, SnakemakeMetadata};
