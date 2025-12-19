# Installation

## Requirements

- Rust 1.85+ (for building from source)
- Access to SLURM or LSF cluster commands

## From Source

```bash
cargo install --git https://github.com/rnabioco/charmer-rs.git
```

Or clone and build:

```bash
git clone https://github.com/rnabioco/charmer-rs.git
cd charmer-rs
cargo build --release
./target/release/charmer --help
```

## Pre-built Binaries

Download from the [releases page](https://github.com/rnabioco/charmer-rs/releases):

- `charmer-linux-x86_64.tar.gz` - Linux (glibc)
- `charmer-linux-x86_64-musl.tar.gz` - Linux (musl, static)
- `charmer-macos-x86_64.tar.gz` - macOS Intel
- `charmer-macos-aarch64.tar.gz` - macOS Apple Silicon

```bash
curl -LO https://github.com/rnabioco/charmer-rs/releases/latest/download/charmer-linux-x86_64.tar.gz
tar xzf charmer-linux-x86_64.tar.gz
./charmer --help
```

## Verify Installation

```bash
charmer --version
charmer --help
```
