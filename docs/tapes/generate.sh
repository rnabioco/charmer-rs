#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$ROOT_DIR"

# Build release binary
cargo build --release

# Clean and start the test pipeline
pixi run clean-test
cd tests/pipelines/simple
snakemake --cores 2 &
PIPELINE_PID=$!

# Wait for jobs to register
sleep 3

# Generate tapes
cd "$SCRIPT_DIR"
vhs demo.tape
vhs quickstart.tape

# Cleanup
kill $PIPELINE_PID 2>/dev/null || true
wait $PIPELINE_PID 2>/dev/null || true

echo "Tapes generated in ../images/"
