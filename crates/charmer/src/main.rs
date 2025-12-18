//! Charmer - Snakemake pipeline monitor for SLURM/LSF.

mod polling;
mod watcher;

use charmer_cli::Args;
use charmer_core::{parse_main_log, parse_metadata_file, scan_metadata_dir};
use charmer_monitor::App;
use charmer_state::{merge_snakemake_jobs, PipelineState};
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use miette::{IntoDiagnostic, Result};
use polling::{init_polling, PollingConfig};
use ratatui::prelude::*;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use watcher::{MetadataWatcher, WatcherEvent};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize pipeline state wrapped in Arc<Mutex<>> for sharing with polling service
    let state = Arc::new(Mutex::new(PipelineState::new(args.dir.clone())));

    // Scan existing metadata files on startup, filtering to recent jobs
    if let Ok(existing_jobs) = scan_metadata_dir(&args.dir) {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(args.history_hours as i64);
        let recent_jobs: Vec<_> = existing_jobs
            .into_iter()
            .filter(|job| {
                // Keep jobs that are incomplete (still running) or started recently
                job.metadata.incomplete || job.metadata.starttime > cutoff.timestamp() as f64
            })
            .collect();

        if !recent_jobs.is_empty() {
            let mut state_guard = state.lock().await;
            merge_snakemake_jobs(&mut state_guard, recent_jobs);
        }
    }

    // Initialize polling service in the background
    let poll_config = PollingConfig {
        active_poll_interval: Duration::from_secs(args.poll_interval),
        history_poll_interval: Duration::from_secs(30),
        run_uuid: args.run_uuid.clone(),
        history_hours: args.history_hours,
    };

    let _polling_handle = init_polling(Arc::clone(&state), poll_config).await;

    // Initialize app with a clone of the initial state
    let initial_state = {
        let state_guard = state.lock().await;
        state_guard.clone()
    };
    let mut app = App::new(initial_state);
    app.update_job_list();

    // Setup terminal
    enable_raw_mode().into_diagnostic()?;
    let mut stdout = io::stdout();
    // Note: We don't enable mouse capture to allow text selection with trackpad
    execute!(stdout, EnterAlternateScreen).into_diagnostic()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).into_diagnostic()?;

    // Create file watcher
    let watcher = MetadataWatcher::new(&args.dir).ok();

    // Run the main loop
    let res = run_app(&mut terminal, &mut app, state, watcher).await;

    // Restore terminal
    disable_raw_mode().into_diagnostic()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).into_diagnostic()?;
    terminal.show_cursor().into_diagnostic()?;

    // Handle result
    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

/// Main application loop.
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    shared_state: Arc<Mutex<PipelineState>>,
    watcher: Option<MetadataWatcher>,
) -> io::Result<()> {
    let tick_rate = Duration::from_millis(100);
    let update_interval = Duration::from_millis(500);
    let rescan_interval = Duration::from_secs(2); // Periodic re-scan fallback
    let log_parse_interval = Duration::from_secs(1); // Parse main log frequently for progress

    let mut last_update = std::time::Instant::now();
    let mut last_rescan = std::time::Instant::now();
    let mut last_log_parse = std::time::Instant::now();
    let mut debounce_map: HashMap<String, std::time::Instant> = HashMap::new();
    let debounce_duration = Duration::from_millis(500);

    loop {
        // Periodically sync app state from shared state (updated by polling service)
        if last_update.elapsed() >= update_interval {
            let state_guard = shared_state.lock().await;
            app.update_from_state(state_guard.clone());
            drop(state_guard);
            last_update = std::time::Instant::now();
        }

        // Periodically parse main snakemake log for pipeline-level info
        if last_log_parse.elapsed() >= log_parse_interval {
            let state_guard = shared_state.lock().await;
            let working_dir = state_guard.working_dir.clone();
            drop(state_guard);

            if let Ok(log_info) = parse_main_log(&working_dir) {
                let mut state_guard = shared_state.lock().await;
                state_guard.update_from_log_info(&log_info);
            }
            last_log_parse = std::time::Instant::now();
        }

        // Draw UI
        terminal.draw(|frame| app.render(frame))?;

        // Handle keyboard events (non-blocking)
        if app.poll_events(tick_rate)? {
            // Event was handled
        }

        // Check for file watcher events (non-blocking)
        if let Some(ref w) = watcher {
            while let Some(event) = w.try_recv_nonblocking() {
                match event {
                    WatcherEvent::MetadataFile(path) => {
                        // Debounce rapid changes to the same file
                        let path_str = path.to_string();
                        let now = std::time::Instant::now();

                        if let Some(last_time) = debounce_map.get(&path_str) {
                            if now.duration_since(*last_time) < debounce_duration {
                                continue; // Skip this event - too soon
                            }
                        }

                        debounce_map.insert(path_str, now);

                        // Parse and merge the metadata file
                        if let Ok(job) = parse_metadata_file(&path) {
                            let mut state_guard = shared_state.lock().await;
                            merge_snakemake_jobs(&mut state_guard, vec![job]);
                            drop(state_guard);
                        }
                    }
                    WatcherEvent::MetadataDirectoryCreated => {
                        // Metadata directory was just created - scan for any existing files
                        let state_guard = shared_state.lock().await;
                        let working_dir = state_guard.working_dir.clone();
                        drop(state_guard);

                        if let Ok(jobs) = scan_metadata_dir(&working_dir) {
                            let mut state_guard = shared_state.lock().await;
                            merge_snakemake_jobs(&mut state_guard, jobs);
                        }
                    }
                    WatcherEvent::Error(err) => {
                        eprintln!("File watcher error: {}", err);
                    }
                }
            }

            // Clean up old debounce entries (keep map from growing unbounded)
            let now = std::time::Instant::now();
            debounce_map.retain(|_, time| now.duration_since(*time) < debounce_duration * 10);
        }

        // Periodic re-scan as fallback (in case file watcher misses events)
        if last_rescan.elapsed() >= rescan_interval {
            let state_guard = shared_state.lock().await;
            let working_dir = state_guard.working_dir.clone();
            drop(state_guard);

            if let Ok(jobs) = scan_metadata_dir(&working_dir) {
                if !jobs.is_empty() {
                    let mut state_guard = shared_state.lock().await;
                    merge_snakemake_jobs(&mut state_guard, jobs);
                }
            }
            last_rescan = std::time::Instant::now();
        }

        // Check if we should quit
        if app.should_quit {
            return Ok(());
        }

        // Small sleep to prevent CPU spinning
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
