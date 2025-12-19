# charmer

A terminal user interface (TUI) for monitoring Snakemake pipelines running on HPC clusters.

![Charmer Demo](images/demo.gif)

## Features

- **Real-time monitoring** - Watch pipeline jobs as they run on SLURM or LSF clusters
- **Unified view** - Combines scheduler data with Snakemake metadata for complete visibility
- **Interactive TUI** - Vim-style navigation, filtering, sorting, and log viewing
- **Multi-scheduler** - Supports both SLURM and LSF cluster schedulers

## Quick Start

```bash
# Install
cargo install --git https://github.com/rnabioco/charmer-rs.git

# Run in your pipeline directory
cd /path/to/snakemake/pipeline
charmer
```

Press `?` for help within the interface.
