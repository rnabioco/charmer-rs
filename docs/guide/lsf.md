# LSF Integration

Charmer queries IBM Spectrum LSF to get real-time job status information.

## How It Works

Charmer uses two LSF commands:

### bjobs (Active Jobs)

Queries every `--poll-interval` seconds:

```bash
bjobs -o "jobid stat queue submit_time start_time finish_time exec_host nprocs memlimit job_description delimiter='|'" -noheader
```

This retrieves:
- Job ID
- State
- Queue
- Submit/start/finish times
- Execution host
- Resources (processors, memory)
- Job description (contains rule info)

### bhist (Job History)

Queries every 30 seconds for completed jobs:

```bash
bhist -a -l
```

## Snakemake LSF Executor

Charmer works with the [snakemake-executor-plugin-lsf](https://github.com/snakemake/snakemake-executor-plugin-lsf).

### Job Correlation

The LSF executor should set the job description to:

```
rule_{rulename}_wildcards_{wildcards}
```

### Snakemake Profile

Example profile for LSF (`~/.config/snakemake/lsf/config.yaml`):

```yaml
executor: lsf

default-resources:
  lsf_queue: "short"
  mem_mb: 4000
  runtime: 60
```

## Job States

| LSF State | Charmer Status | Description |
|-----------|----------------|-------------|
| PEND | Queued | Waiting in queue |
| RUN | Running | Currently executing |
| DONE | Completed | Finished successfully (exit 0) |
| EXIT | Failed | Exited with non-zero status |
| PSUSP | Pending | Suspended while pending |
| USUSP | Pending | Suspended by user |
| SSUSP | Pending | Suspended by system |
| ZOMBI | Unknown | Zombie job |

## Queue Information

Charmer displays the LSF queue as the "partition" in job details, matching the SLURM terminology for consistency.

## Troubleshooting

### Jobs Not Appearing

1. Check that you're running charmer as the same user who submitted jobs
2. Verify bjobs works: `bjobs -a`
3. Check the `--run-uuid` filter if specified

### Permission Issues

On some clusters, bhist may require special permissions. Contact your system administrator if historical jobs don't appear.

### Missing Job Description

Ensure your Snakemake profile or job submission script sets the job description field for proper rule correlation.
