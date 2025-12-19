# Configuration

## Command Line Options

```bash
charmer [OPTIONS] [DIR]
```

### Arguments

| Argument | Default | Description |
|----------|---------|-------------|
| `DIR` | `.` | Pipeline directory to monitor |

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--poll-interval <SECS>` | 5 | Seconds between scheduler queries |
| `--run-uuid <UUID>` | - | Filter to specific Snakemake run |
| `--theme <THEME>` | dark | Color theme (`dark` or `light`) |
| `--history-hours <N>` | 24 | Show completed jobs from last N hours |
| `-h, --help` | - | Print help information |
| `-V, --version` | - | Print version information |

## Examples

### Monitor Current Directory

```bash
charmer
```

### Monitor Specific Directory

```bash
charmer /path/to/pipeline
```

### Adjust Polling Frequency

```bash
# Poll every 10 seconds (less frequent, lower overhead)
charmer --poll-interval 10

# Poll every 2 seconds (more frequent updates)
charmer --poll-interval 2
```

### Filter to Specific Run

If you have multiple pipelines running, filter by Snakemake run UUID:

```bash
charmer --run-uuid abc123-def456
```

### Use Light Theme

```bash
charmer --theme light
```

### Show Longer History

```bash
# Show completed jobs from last 48 hours
charmer --history-hours 48
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `USER` | Used for filtering scheduler queries to your jobs |
| `RUST_LOG` | Set to `debug` for verbose logging |

## File Locations

Charmer reads data from these locations:

| Path | Description |
|------|-------------|
| `.snakemake/metadata/` | Snakemake job metadata files |
| `.snakemake/slurm_logs/` | SLURM job log files |
| `.snakemake/lsf_logs/` | LSF job log files |
