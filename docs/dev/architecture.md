# Architecture

Charmer is organized as a Rust workspace with multiple crates.

## Crate Structure

```
charmer/
├── crates/
│   ├── charmer/          # Main binary
│   ├── charmer-cli/      # CLI argument parsing
│   ├── charmer-core/     # Snakemake metadata parsing
│   ├── charmer-slurm/    # SLURM integration
│   ├── charmer-lsf/      # LSF integration
│   ├── charmer-state/    # Unified job state
│   └── charmer-monitor/  # TUI components
```

## Crate Responsibilities

### charmer

The main binary. Handles:
- Terminal setup/teardown
- Main event loop
- Coordinating polling and rendering

### charmer-cli

CLI argument parsing using clap:
- `Args` struct with all command-line options
- Validation and defaults

### charmer-core

Snakemake metadata parsing:
- Base64 filename decoding
- JSON metadata parsing
- Directory scanning

### charmer-slurm

SLURM integration:
- `squeue` parsing for active jobs
- `sacct` parsing for historical jobs
- State mapping to unified types

### charmer-lsf

LSF integration:
- `bjobs` parsing for active jobs
- `bhist` parsing for historical jobs
- State mapping to unified types

### charmer-state

Unified state management:
- `PipelineState` - All jobs and metadata
- `Job` - Unified job representation
- Merge functions for combining data sources
- Job correlation logic

### charmer-monitor

TUI components using ratatui:
- `App` - Main application state and event handling
- `Header` - Progress bar
- `JobList` - Scrollable job list
- `JobDetail` - Selected job information
- `Footer` - Keyboard shortcuts
- `LogViewer` - Log file display

## Data Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Snakemake  │     │   SLURM     │     │    LSF      │
│  Metadata   │     │ squeue/sacct│     │ bjobs/bhist │
└──────┬──────┘     └──────┬──────┘     └──────┬──────┘
       │                   │                   │
       ▼                   ▼                   ▼
┌──────────────────────────────────────────────────────┐
│                  charmer-state                        │
│                                                       │
│  merge_snakemake_jobs()  merge_slurm_jobs()          │
│                     merge_lsf_jobs()                  │
│                          │                            │
│                          ▼                            │
│                   PipelineState                       │
│                   ┌─────────────┐                     │
│                   │    Jobs     │                     │
│                   │  (HashMap)  │                     │
│                   └─────────────┘                     │
└──────────────────────────┬───────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────┐
│                  charmer-monitor                      │
│                                                       │
│   Header ──────────────────────────────────────┐     │
│   StatusBar ───────────────────────────────────│     │
│   ┌─────────────────┬──────────────────────────┤     │
│   │    JobList      │      JobDetail           │     │
│   │                 │                          │     │
│   └─────────────────┴──────────────────────────┘     │
│   Footer ──────────────────────────────────────┘     │
└──────────────────────────────────────────────────────┘
```

## Key Types

### Job (charmer-state)

```rust
pub struct Job {
    pub id: String,
    pub rule: String,
    pub wildcards: Option<String>,
    pub outputs: Vec<String>,
    pub inputs: Vec<String>,
    pub status: JobStatus,
    pub slurm_job_id: Option<String>,
    pub shellcmd: String,
    pub timing: JobTiming,
    pub resources: JobResources,
    pub log_files: Vec<String>,
    pub error: Option<JobError>,
    pub data_sources: DataSources,
}
```

### JobStatus (charmer-state)

```rust
pub enum JobStatus {
    Pending,    // Waiting for dependencies
    Queued,     // Submitted to scheduler
    Running,    // Currently executing
    Completed,  // Success
    Failed,     // Error
    Cancelled,  // User cancelled
    Unknown,    // Unknown state
}
```

## Event Loop

```rust
loop {
    // 1. Render UI
    terminal.draw(|frame| app.render(frame))?;

    // 2. Handle input events (100ms timeout)
    if app.poll_events(tick_rate)? {
        // Key pressed, state updated
    }

    // 3. Check for quit
    if app.should_quit {
        break;
    }

    // 4. Background: poll scheduler, watch files
    // (handled by separate tokio tasks)
}
```
