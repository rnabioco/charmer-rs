# charmer

A terminal user interface (TUI) for monitoring Snakemake pipelines running on HPC clusters.

![Charmer Demo](docs/assets/demo.gif)

## Features

- **Real-time monitoring** of Snakemake pipelines on SLURM and LSF clusters
- **Unified view** merging data from scheduler queries and Snakemake metadata
- **Interactive TUI** with vim-style navigation
- **Filtering & sorting** by job status, rule name, or time
- **Log viewer** for examining job output
- **Cross-platform** support for Linux and macOS

## Installation

### From source

```bash
# Clone the repository
git clone https://github.com/rnabioco/charmer.git
cd charmer

# Build with cargo
cargo build --release

# Install to ~/.cargo/bin
cargo install --path crates/charmer
```

### Using pixi (for development)

```bash
pixi install
pixi run build
```

## Usage

```bash
# Monitor current directory
charmer

# Monitor specific directory
charmer /path/to/pipeline

# With options
charmer --poll-interval 10 --theme dark /path/to/pipeline
```

### CLI Options

| Option | Default | Description |
|--------|---------|-------------|
| `--poll-interval` | 5 | Seconds between scheduler queries |
| `--run-uuid` | - | Filter to specific Snakemake run |
| `--theme` | dark | Color theme (dark/light) |
| `--history-hours` | 24 | Show completed jobs from last N hours |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `g` / `Home` | Go to first job |
| `G` / `End` | Go to last job |
| `f` | Cycle filter (All/Running/Failed/Pending/Completed) |
| `s` | Cycle sort (Status/Rule/Time) |
| `l` / `Enter` | View job logs |
| `F` | Toggle follow mode in logs |
| `?` | Show help |
| `q` / `Ctrl+C` | Quit |

## Supported Schedulers

### SLURM

Charmer queries SLURM using:
- `squeue` for active jobs
- `sacct` for job history

Jobs are correlated with Snakemake using the comment field format:
`rule_{rulename}_wildcards_{wildcards}`

### LSF

Charmer queries LSF using:
- `bjobs` for active jobs
- `bhist` for job history

## How It Works

Charmer combines data from multiple sources:

1. **Snakemake metadata** (`.snakemake/metadata/`) - Job inputs, outputs, shell commands
2. **Scheduler queries** - Job status, resource usage, timing
3. **Log files** (`.snakemake/slurm_logs/`) - Job output and errors

Data is merged using rule names and timing windows to correlate jobs across sources.

## Development

```bash
# Install dependencies
pixi install

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- .

# Build documentation
pixi run docs
```

### Project Structure

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
├── docs/                 # Documentation
└── tests/                # Integration tests
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions welcome! Please read our [contributing guide](CONTRIBUTING.md) first.
