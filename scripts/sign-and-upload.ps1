# Usage: .\scripts\sign-and-upload.ps1 -Tag v0.1.3-alpha3
param(
    [Parameter(Mandatory)]
    [string]$Tag
)

$ErrorActionPreference = "Stop"

$archiveName = "codirigent-$Tag-x86_64-pc-windows-msvc"
$msiName     = "$archiveName.msi"
$tmpDir      = Join-Path $env:TEMP "codirigent-sign-$Tag"

New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null

# ── 1. Download unsigned MSI from the release ─────────────────────────────────
Write-Host "Downloading $msiName from release $Tag..."
gh release download $Tag --pattern $msiName --output "$tmpDir\$msiName" --clobber

# ── 2. Find signtool ──────────────────────────────────────────────────────────
$signtool = (Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe" |
    Sort-Object FullName -Descending | Select-Object -First 1).FullName

if (-not $signtool) {
    Write-Error "signtool.exe not found. Install Windows SDK."
}

# ── 3. Find the Certum certificate ───────────────────────────────────────────
$cert = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object { $_.Issuer -like "*Certum*" -and $_.EnhancedKeyUsageList.FriendlyName -contains "Code Signing" } |
    Select-Object -First 1

if (-not $cert) {
    Write-Error "Certum code signing certificate not found in CurrentUser\My store."
}

Write-Host "Using certificate: $($cert.Subject)"

# ── 4. Sign the MSI ───────────────────────────────────────────────────────────
Write-Host "Signing $msiName..."
& $signtool sign `
    /sha1 $cert.Thumbprint `
    /fd sha256 `
    /tr http://time.certum.pl `
    /td sha256 `
    /v "$tmpDir\$msiName"

if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# ── 5. Verify signature ───────────────────────────────────────────────────────
Write-Host "Verifying signature..."
& $signtool verify /pa /v "$tmpDir\$msiName"
if ($LASTEXITCODE -ne 0) { Write-Warning "Signature verification failed!" }

# ── 6. Compute new checksum ───────────────────────────────────────────────────
Write-Host "Computing checksum..."
$hash = (Get-FileHash "$tmpDir\$msiName" -Algorithm SHA256).Hash.ToLower()
$checksumLine = "$hash  $msiName"
Write-Host "Checksum: $checksumLine"

# ── 7. Download existing checksums-sha256.txt and append MSI checksum ─────────
$checksumFile = "$tmpDir\checksums-sha256.txt"
gh release download $Tag --pattern "checksums-sha256.txt" --output $checksumFile --clobber

# Remove any stale MSI line (in case of re-run) then append new one
$lines = Get-Content $checksumFile | Where-Object { $_ -notmatch '\.msi' }
$lines + $checksumLine | Set-Content $checksumFile

Write-Host "`nUpdated checksums-sha256.txt:"
Get-Content $checksumFile

# ── 8. Upload signed MSI and updated checksums ────────────────────────────────
Write-Host "`nUploading signed MSI and updated checksums to release $Tag..."
gh release upload $Tag "$tmpDir\$msiName" --clobber
gh release upload $Tag $checksumFile --clobber

Write-Host "`nDone! Signed MSI and checksums uploaded to release $Tag."
Write-Host "Temp files at: $tmpDir"
