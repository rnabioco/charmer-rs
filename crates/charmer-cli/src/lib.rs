//! CLI argument parsing for charmer.

use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "charmer")]
#[command(about = "Monitor snakemake pipelines running on SLURM")]
pub struct Args {
    /// Pipeline directory
    #[arg(default_value = ".")]
    pub dir: Utf8PathBuf,

    /// SLURM poll interval in seconds
    #[arg(long, default_value = "5")]
    pub poll_interval: u64,

    /// Filter to specific snakemake run UUID
    #[arg(long)]
    pub run_uuid: Option<String>,

    /// Color theme (dark or light)
    #[arg(long, default_value = "dark")]
    pub theme: String,

    /// Show completed jobs from last N hours
    #[arg(long, default_value = "24")]
    pub history_hours: u64,
}
