# Quick Start

## Basic Usage

Navigate to your Snakemake pipeline directory and run:

```bash
cd /path/to/my/pipeline
charmer
```

The TUI will launch and display any jobs found in the `.snakemake/metadata/` directory and from scheduler queries.

## Running Alongside Snakemake

Start your pipeline in one terminal:

```bash
snakemake --profile slurm -j 100
```

In another terminal, start charmer:

```bash
charmer
```

Charmer will automatically detect new jobs as Snakemake submits them.

## Navigating the Interface

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ charmer                              Running  01:23:45                      │
├─────────────────────────────────────────────────────────────────────────────┤
│ ██████████████████████░░░░░░░░░░░░░░  42/100 jobs  42%                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ 12 Pending │ 8 Running │ 38 Done │ 1 Failed │ Filter: All │ Sort: Status  │
├───────────────────────────────────┬─────────────────────────────────────────┤
│ Jobs (3/54)                       │ Job Details                             │
│ ──────────────────────────────────│─────────────────────────────────────────│
│   ● align_reads[sample=S1]        │ Rule: align_reads                       │
│   ● align_reads[sample=S2]        │ SLURM Job: 12345678                     │
│ > ✗ call_variants[chr=chr1]       │ Node: node01                            │
│   ◐ merge_vcfs                    │ Status: Failed (exit 1)                 │
│   ○ annotate_variants             │ Runtime: 05:23                          │
│                                   │ CPUs: 4 | Memory: 32GB                  │
├───────────────────────────────────┴─────────────────────────────────────────┤
│ j/k:navigate  f:filter  s:sort  ?:help  q:quit                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Status Symbols

| Symbol | Status |
|--------|--------|
| `○` | Pending (waiting for dependencies) |
| `◐` | Queued (submitted to scheduler) |
| `●` | Running |
| `✓` | Completed |
| `✗` | Failed |
| `⊘` | Cancelled |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `g` | Go to first job |
| `G` | Go to last job |
| `f` | Cycle filter mode |
| `s` | Cycle sort mode |
| `?` | Show help |
| `q` | Quit |

## Filtering Jobs

Press `f` to cycle through filter modes:

- **All** - Show all jobs
- **Running** - Only running jobs
- **Failed** - Only failed jobs
- **Pending** - Pending and queued jobs
- **Completed** - Successfully completed jobs

## Sorting Jobs

Press `s` to cycle through sort modes:

- **Status** - Running first, then failed, queued, pending, completed
- **Rule** - Alphabetically by rule name
- **Time** - Most recently started first

## Next Steps

- [Configuration Options](../guide/configuration.md)
- [SLURM Integration](../guide/slurm.md)
- [LSF Integration](../guide/lsf.md)
