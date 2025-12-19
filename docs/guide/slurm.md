# SLURM Integration

Charmer queries SLURM to get real-time job status information.

## How It Works

Charmer uses two SLURM commands:

### squeue (Active Jobs)

Queries every `--poll-interval` seconds:

```bash
squeue -u $USER -h -o "%A|%j|%T|%P|%V|%S|%e|%N|%C|%m|%l|%k"
```

This retrieves:
- Job ID
- Job name (run UUID)
- State
- Partition
- Submit/start/end times
- Node list
- Resources (CPUs, memory, time limit)
- Comment field (contains rule info)

### sacct (Job History)

Queries every 30 seconds for completed jobs:

```bash
sacct -X --parsable2 --noheader \
  --format=JobIDRaw,JobName,State,Partition,Submit,Start,End,NodeList,AllocCPUS,ReqMem,Timelimit,Comment,ExitCode \
  --starttime {since}
```

## Snakemake SLURM Executor

Charmer is designed to work with the [snakemake-executor-plugin-slurm](https://github.com/snakemake/snakemake-executor-plugin-slurm).

### Job Correlation

The SLURM executor sets the comment field to:

```
rule_{rulename}_wildcards_{wildcards}
```

For example:
- `rule_align_reads_wildcards_sample=S1`
- `rule_call_variants_wildcards_sample=S1,chrom=chr1`

Charmer parses this to correlate SLURM jobs with Snakemake rules.

### Snakemake Profile

Example profile for SLURM (`~/.config/snakemake/slurm/config.yaml`):

```yaml
executor: slurm

default-resources:
  slurm_partition: "short"
  mem_mb: 4000
  runtime: 60

# Enable comment field for charmer
set-resources:
  __default__:
    slurm_extra: "'--comment=rule_{rule}_wildcards_{wildcards}'"
```

## Job States

| SLURM State | Charmer Status | Description |
|-------------|----------------|-------------|
| PENDING | Queued | Waiting in queue |
| RUNNING | Running | Currently executing |
| COMPLETED | Completed | Finished successfully |
| FAILED | Failed | Exited with error |
| CANCELLED | Cancelled | Cancelled by user |
| TIMEOUT | Failed | Exceeded time limit |
| OUT_OF_MEMORY | Failed | Exceeded memory limit |

## Troubleshooting

### Jobs Not Appearing

1. Check that you're running charmer as the same user who submitted jobs
2. Verify squeue works: `squeue -u $USER`
3. Check the `--run-uuid` filter if specified

### Missing Historical Jobs

1. Increase `--history-hours` (default 24)
2. Verify sacct works: `sacct --starttime=now-24hours`
3. Check that SLURM accounting is enabled on your cluster

### Comment Field Empty

Ensure your Snakemake profile sets the comment field. See the profile example above.
