#!/bin/bash
# Coverage script for Unix (Linux/macOS)
# Runs test coverage analysis using cargo-tarpaulin

set -e

echo "Running test coverage analysis..."

# Check if tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
fi

# Run tarpaulin
echo "Generating coverage report..."
cargo tarpaulin \
    --workspace \
    --all-features \
    --timeout 300 \
    --out Xml \
    --out Html \
    --output-dir target/coverage

echo "Coverage report generated in target/coverage/"
echo "Open target/coverage/index.html to view results"

# Extract and check coverage threshold
COVERAGE=$(cargo tarpaulin --workspace --all-features --timeout 300 2>&1 | grep -oP '\d+\.\d+%' | head -1 | tr -d '%')

if [ -z "$COVERAGE" ]; then
    echo "WARNING: Could not extract coverage percentage"
    exit 0
fi

THRESHOLD=70.0

# Use bc for floating point comparison (if available), otherwise use awk
if command -v bc &> /dev/null; then
    if (( $(echo "$COVERAGE < $THRESHOLD" | bc -l) )); then
        echo "ERROR: Coverage $COVERAGE% is below threshold $THRESHOLD%"
        exit 1
    fi
else
    if awk "BEGIN {exit !($COVERAGE < $THRESHOLD)}"; then
        echo "ERROR: Coverage $COVERAGE% is below threshold $THRESHOLD%"
        exit 1
    fi
fi

echo "✓ Coverage $COVERAGE% meets threshold $THRESHOLD%"
