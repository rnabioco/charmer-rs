# charmer

A terminal user interface (TUI) for monitoring Snakemake pipelines running on HPC clusters.

<div class="grid cards" markdown>

- :material-monitor: **Real-time Monitoring**

  Watch your pipeline jobs as they run on SLURM or LSF clusters.

- :material-merge: **Unified View**

  Combines scheduler data with Snakemake metadata for complete visibility.

- :material-keyboard: **Interactive TUI**

  Vim-style navigation, filtering, sorting, and log viewing.

- :material-server: **Multi-Scheduler**

  Supports both SLURM and LSF cluster schedulers.

</div>

## Demo

![Charmer Demo](images/demo.gif)

## Quick Start

```bash
# Install
cargo install charmer

# Run in your pipeline directory
cd /path/to/snakemake/pipeline
charmer
```

## Features

### Job Monitoring

- View all pipeline jobs with real-time status updates
- Filter by status: Running, Failed, Pending, Completed
- Sort by rule name, status, or start time
- See detailed job information including resources and timing

### Log Viewing

- View job log files directly in the TUI
- Follow mode for watching running jobs
- Scroll through historical output

### Data Integration

Charmer combines data from multiple sources:

| Source | Data |
|--------|------|
| Snakemake metadata | Rule, inputs, outputs, shell command |
| squeue/bjobs | Active job status, resources |
| sacct/bhist | Historical data, exit codes |
| Log files | Job output, errors |

## Requirements

- Rust 1.85+ (for building from source)
- Access to SLURM or LSF cluster commands
- A running Snakemake pipeline

## Next Steps

- [Installation Guide](getting-started/installation.md)
- [Quick Start Tutorial](getting-started/quickstart.md)
- [Configuration Options](guide/configuration.md)
