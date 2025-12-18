# Installation

## Requirements

- **Rust 1.85+** (for building from source)
- Access to SLURM or LSF cluster commands
- A running or completed Snakemake pipeline

## From Source

### Using Cargo

```bash
# Clone the repository
git clone https://github.com/rnabioco/charmer.git
cd charmer

# Build release binary
cargo build --release

# The binary will be at target/release/charmer
./target/release/charmer --help

# Or install to ~/.cargo/bin
cargo install --path crates/charmer
```

### Using Pixi (Recommended for Development)

[Pixi](https://pixi.sh) manages both Rust and Python dependencies:

```bash
# Install pixi if you haven't already
curl -fsSL https://pixi.sh/install.sh | bash

# Clone and install
git clone https://github.com/rnabioco/charmer.git
cd charmer
pixi install

# Build
pixi run build

# Run
pixi run charmer
```

## Pre-built Binaries

Pre-built binaries are available for each release:

- `charmer-linux-x86_64.tar.gz` - Linux (glibc)
- `charmer-linux-x86_64-musl.tar.gz` - Linux (musl, static)
- `charmer-macos-x86_64.tar.gz` - macOS Intel
- `charmer-macos-aarch64.tar.gz` - macOS Apple Silicon

Download from the [releases page](https://github.com/rnabioco/charmer/releases).

```bash
# Example for Linux
curl -LO https://github.com/rnabioco/charmer/releases/latest/download/charmer-linux-x86_64.tar.gz
tar xzf charmer-linux-x86_64.tar.gz
./charmer --help
```

## Verifying Installation

```bash
# Check version
charmer --version

# Show help
charmer --help
```

## Next Steps

- [Quick Start Guide](quickstart.md)
- [Configuration Options](../guide/configuration.md)
