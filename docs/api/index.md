# API Reference

Charmer is organized as a Rust workspace with multiple crates.

## Crates

### charmer-core

Snakemake metadata parsing.

```rust
use charmer_core::{scan_metadata_dir, SnakemakeJob, SnakemakeMetadata};

// Scan metadata directory
let jobs = scan_metadata_dir(working_dir)?;

// Parse single file
let job = parse_metadata_file(path)?;
```

### charmer-slurm

SLURM integration.

```rust
use charmer_slurm::{query_squeue, query_sacct, SlurmJob};

// Query active jobs
let active = query_squeue(Some("run-uuid")).await?;

// Query historical jobs
let history = query_sacct(Some("run-uuid"), Some(since)).await?;
```

### charmer-lsf

LSF integration.

```rust
use charmer_lsf::{query_bjobs, query_bhist, LsfJob};

// Query active jobs
let active = query_bjobs(Some("job-name")).await?;

// Query historical jobs
let history = query_bhist(Some("job-name"), Some(since)).await?;
```

### charmer-state

Unified state management.

```rust
use charmer_state::{PipelineState, Job, JobStatus};

// Create state
let mut state = PipelineState::new(working_dir);

// Merge data
merge_snakemake_jobs(&mut state, snakemake_jobs);
merge_slurm_jobs(&mut state, slurm_jobs, false);

// Get counts
let counts = state.job_counts();
```

### charmer-monitor

TUI components.

```rust
use charmer_monitor::App;

// Create app
let app = App::new(state);

// Handle events
app.handle_key(key_event);

// Render
app.render(frame);
```

## Full API Documentation

For complete API documentation, build the Rust docs:

```bash
cargo doc --open
```

This will open the generated documentation in your browser.
