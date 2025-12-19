# charmer-rs

A terminal user interface (TUI) for monitoring Snakemake pipelines on HPC clusters.

![Charmer Demo](docs/images/demo.gif)

## Features

- Real-time monitoring of Snakemake pipelines on SLURM and LSF clusters
- Interactive TUI with vim-style navigation
- Filtering, sorting, and log viewing

## Installation

```bash
cargo install --git https://github.com/rnabioco/charmer-rs.git
```

## Quick Start

```bash
# Monitor current directory
charmer

# Monitor specific directory
charmer /path/to/pipeline
```

## Documentation

Full documentation is available at **[rnabioco.github.io/charmer-rs](https://rnabioco.github.io/charmer-rs)**.

## License

MIT License - see [LICENSE](LICENSE) for details.
