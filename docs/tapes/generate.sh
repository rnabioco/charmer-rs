#!/usr/bin/env bash
set -euo pipefail

# Change to repo root directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

# Install charmer binary
cargo install --path crates/charmer

# Clean and start the demo pipeline
pixi run clean-demo
snakemake --cores 4 --snakefile tests/pipelines/demo/Snakefile --directory tests/pipelines/demo &
PIPELINE_PID=$!

# Wait for jobs to register
sleep 3

# Generate tapes from repo root
vhs docs/tapes/demo.tape
vhs docs/tapes/quickstart.tape

# Cleanup
kill $PIPELINE_PID 2>/dev/null || true
wait $PIPELINE_PID 2>/dev/null || true

echo "Tapes generated in docs/images/"
