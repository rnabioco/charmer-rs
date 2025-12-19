# Keyboard Shortcuts

## Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `g` / `Home` | Go to first job |
| `G` / `End` | Go to last job |

## Filtering & Sorting

| Key | Action |
|-----|--------|
| `f` | Cycle filter (All → Running → Failed → Pending → Completed) |
| `s` | Cycle sort (Status → Rule → Time) |

## Log Viewer

| Key | Action |
|-----|--------|
| `l` / `Enter` | Open log viewer for selected job |
| `F` | Toggle follow mode (auto-scroll to end) |
| `q` / `Escape` | Close log viewer |

When in log viewer:

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `g` | Go to beginning of log |
| `G` | Go to end of log |
| `F` | Toggle follow mode |
| `q` | Return to job list |

## General

| Key | Action |
|-----|--------|
| `?` | Toggle help overlay |
| `q` / `Ctrl+C` | Quit charmer |
| `r` | Force refresh (re-query scheduler) |

## Filter Modes

| Mode | Shows |
|------|-------|
| All | All jobs |
| Running | Jobs currently executing |
| Failed | Jobs that exited with errors |
| Pending | Jobs waiting for dependencies or queued |
| Completed | Successfully finished jobs |

## Sort Modes

| Mode | Order |
|------|-------|
| Status | Running → Failed → Queued → Pending → Completed |
| Rule | Alphabetically by rule name |
| Time | Most recently started first |
