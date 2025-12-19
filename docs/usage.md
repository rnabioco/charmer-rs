# Usage

## Basic Usage

Navigate to your Snakemake pipeline directory and run:

```bash
cd /path/to/my/pipeline
charmer
```

The TUI will display jobs from `.snakemake/metadata/` and scheduler queries.

## Running Alongside Snakemake

Start your pipeline in one terminal:

```bash
snakemake --profile slurm -j 100
```

In another terminal, start charmer:

```bash
charmer
```

Charmer automatically detects new jobs as Snakemake submits them.

## Interface

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
| `○` | Pending |
| `◐` | Queued |
| `●` | Running |
| `✓` | Completed |
| `✗` | Failed |
| `⊘` | Cancelled |

## Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `g` / `Home` | Go to first job |
| `G` / `End` | Go to last job |

### Filtering & Sorting

| Key | Action |
|-----|--------|
| `f` | Cycle filter (All → Running → Failed → Pending → Completed) |
| `s` | Cycle sort (Status → Rule → Time) |

### Log Viewer

| Key | Action |
|-----|--------|
| `l` / `Enter` | Open log viewer |
| `F` | Toggle follow mode |
| `q` / `Escape` | Close log viewer |

### General

| Key | Action |
|-----|--------|
| `?` | Toggle help |
| `r` | Force refresh |
| `q` / `Ctrl+C` | Quit |
