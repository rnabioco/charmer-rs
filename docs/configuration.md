# Configuration

## Command Line Options

```bash
charmer [OPTIONS] [DIR]
```

| Option | Default | Description |
|--------|---------|-------------|
| `DIR` | `.` | Pipeline directory to monitor |
| `--poll-interval <SECS>` | 5 | Seconds between scheduler queries |
| `--run-uuid <UUID>` | - | Filter to specific Snakemake run |
| `--theme <THEME>` | dark | Color theme (`dark` or `light`) |
| `--history-hours <N>` | 24 | Show completed jobs from last N hours |

## Examples

```bash
# Monitor current directory
charmer

# Monitor specific directory
charmer /path/to/pipeline

# Poll every 10 seconds
charmer --poll-interval 10

# Filter to specific run
charmer --run-uuid abc123-def456

# Use light theme
charmer --theme light

# Show 48 hours of history
charmer --history-hours 48
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `USER` | Used for filtering scheduler queries to your jobs |
| `RUST_LOG` | Set to `debug` for verbose logging |

## File Locations

Charmer reads data from:

| Path | Description |
|------|-------------|
| `.snakemake/metadata/` | Snakemake job metadata |
| `.snakemake/slurm_logs/` | SLURM job logs |
| `.snakemake/lsf_logs/` | LSF job logs |
