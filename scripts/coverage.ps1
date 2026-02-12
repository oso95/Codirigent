# PowerShell coverage script for Windows
# Runs test coverage analysis using cargo-tarpaulin

$ErrorActionPreference = "Stop"

Write-Host "Running test coverage analysis..." -ForegroundColor Cyan

# Check if tarpaulin is installed
if (-not (Get-Command cargo-tarpaulin -ErrorAction SilentlyContinue)) {
    Write-Host "Installing cargo-tarpaulin..." -ForegroundColor Yellow
    cargo install cargo-tarpaulin
}

# Run tarpaulin
Write-Host "Generating coverage report..." -ForegroundColor Cyan
cargo tarpaulin `
    --workspace `
    --all-features `
    --timeout 300 `
    --out Xml `
    --out Html `
    --output-dir target/coverage

if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Coverage generation failed" -ForegroundColor Red
    exit 1
}

Write-Host "Coverage report generated in target/coverage/" -ForegroundColor Green
Write-Host "Open target/coverage/index.html to view results" -ForegroundColor Green

# Extract coverage percentage
$coverageOutput = cargo tarpaulin --workspace --all-features --timeout 300 2>&1 | Out-String
if ($coverageOutput -match '(\d+\.\d+)%') {
    $coverage = [double]$Matches[1]
    $threshold = 70.0

    if ($coverage -lt $threshold) {
        Write-Host "ERROR: Coverage $coverage% is below threshold $threshold%" -ForegroundColor Red
        exit 1
    }

    Write-Host "✓ Coverage $coverage% meets threshold $threshold%" -ForegroundColor Green
} else {
    Write-Host "WARNING: Could not extract coverage percentage" -ForegroundColor Yellow
}
