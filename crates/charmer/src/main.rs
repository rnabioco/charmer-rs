//! Charmer - Snakemake pipeline monitor for SLURM.

use charmer_cli::Args;
use charmer_monitor::App;
use charmer_state::PipelineState;
use clap::Parser;
use miette::Result;

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize pipeline state
    let state = PipelineState::new(args.dir.clone());
    let mut app = App::new(state);

    println!("charmer - monitoring {}", args.dir);
    println!("Poll interval: {}s", args.poll_interval);
    println!("Theme: {}", args.theme);

    // TODO: Start TUI event loop
    // TODO: Start SLURM polling loop
    // TODO: Start metadata file watcher

    Ok(())
}
