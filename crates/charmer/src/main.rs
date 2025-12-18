//! Charmer - Snakemake pipeline monitor for SLURM.

use charmer_cli::Args;
use charmer_monitor::App;
use charmer_state::PipelineState;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use miette::{IntoDiagnostic, Result};
use ratatui::prelude::*;
use std::io;
use std::time::Duration;

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize pipeline state
    let state = PipelineState::new(args.dir.clone());
    let mut app = App::new(state);
    app.update_job_list();

    // Setup terminal
    enable_raw_mode().into_diagnostic()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).into_diagnostic()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).into_diagnostic()?;

    // Run the main loop
    let res = run_app(&mut terminal, &mut app, args.poll_interval);

    // Restore terminal
    disable_raw_mode().into_diagnostic()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .into_diagnostic()?;
    terminal.show_cursor().into_diagnostic()?;

    // Handle result
    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

/// Main application loop.
fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    poll_interval: u64,
) -> io::Result<()> {
    let tick_rate = Duration::from_millis(100);
    let _poll_duration = Duration::from_secs(poll_interval);

    loop {
        // Draw UI
        terminal.draw(|frame| app.render(frame))?;

        // Handle events
        if app.poll_events(tick_rate)? {
            // Event was handled
        }

        // Check if we should quit
        if app.should_quit {
            return Ok(());
        }

        // TODO: Poll SLURM/LSF for updates on poll_duration intervals
        // TODO: Watch metadata directory for changes
    }
}
