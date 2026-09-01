# Windows Release Signing Workflow

Required procedure for signing the Windows MSI installer after CI builds a
release.

---

## Goal

The Certum Open Source Code Signing certificate private key is non-exportable
(SimplySign protected). CI cannot sign the MSI. After CI completes, the MSI
must be signed locally on the machine where SimplySign Desktop is installed,
then uploaded back to the GitHub Release with updated checksums.

---

## Prerequisites

- SimplySign Desktop installed and logged in (system tray icon visible)
- Windows SDK installed (`signtool.exe` available)
- WiX Toolset available at `tools/wix/` in the repo
- `gh` CLI authenticated with push access to the repo
- Certum certificate visible in `certmgr.msc` under Personal > Certificates

---

## Automated Flow (CI Release)

When a tag is pushed, CI handles everything except Windows signing:

### 1. Push the Tag

```bash
git tag -a v0.X.Y -m "Release v0.X.Y"
git push origin v0.X.Y
```

### 2. Wait for CI

The GitHub Actions `Release` workflow builds:

- Windows x64: `.msi` (unsigned) + `.zip`
- macOS Apple Silicon: `.dmg` (signed + notarized) + `.tar.gz`

`checksums-sha256.txt` intentionally excludes the MSI.

### 3. Sign and Upload

Run the signing script after CI completes:

```powershell
.\scripts\sign-and-upload.ps1 -Tag v0.X.Y
```

The script:

1. downloads the unsigned MSI from the release
2. finds the Certum certificate by issuer in the Windows cert store
3. signs the MSI with SHA-256 + Certum timestamp server
4. verifies the signature
5. computes SHA-256 hash and appends to `checksums-sha256.txt`
6. uploads signed MSI and updated checksums to the release

### 4. Verify

After the script completes, confirm:

- [ ] MSI on the release page has a newer timestamp than the original
- [ ] `checksums-sha256.txt` includes a line for the `.msi` file
- [ ] Download the signed MSI and check Properties > Digital Signatures

---

## Local Build Flow (Testing)

For testing installer changes before pushing a tag.

### 1. Set the Version

The workspace version in `Cargo.toml` must match the installer version.
CI does this automatically from the tag. Locally, do it manually:

```bash
# Check current version
grep '^version' Cargo.toml | head -1

# If it needs updating (revert after testing):
# Edit Cargo.toml line 16: version = "0.X.Y"
```

### 2. Build Binaries

```bash
export PATH="$PATH:/c/Program Files (x86)/Windows Kits/10/bin/10.0.19041.0/x64"
cargo build --profile dist --features gpui-full -p codirigent -p codirigent-hook
```

Binaries output to `target/dist/`.

### 3. Build MSI

```bash
WIX_BIN="tools/wix"
"$WIX_BIN/candle.exe" \
  -dBinaryPath="target/dist" \
  -dVersion=0.X.Y.0 \
  -dLicensePath=wix/License.rtf \
  -arch x64 wix/main.wxs -o wix/main.wixobj

"$WIX_BIN/light.exe" \
  -ext "$WIX_BIN/WixUIExtension.dll" \
  wix/main.wixobj -o dist/codirigent.msi
```

Note: WiX version format requires `x.x.x.x` (four integers, no prerelease
suffix). Use `0.1.3.0` not `0.1.3-alpha.1`.

### 4. Sign MSI

Ensure SimplySign Desktop is running and logged in, then:

```powershell
$signtool = (Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe" |
  Sort-Object FullName -Descending | Select-Object -First 1).FullName

$cert = Get-ChildItem Cert:\CurrentUser\My |
  Where-Object { $_.Issuer -like "*Certum*" } |
  Select-Object -First 1

& $signtool sign `
  /sha1 $cert.Thumbprint `
  /fd sha256 `
  /tr http://time.certum.pl `
  /td sha256 `
  /v dist\codirigent.msi
```

### 5. Verify Signature

```powershell
& $signtool verify /pa /v dist\codirigent.msi
```

Or: right-click `dist\codirigent.msi` > Properties > Digital Signatures.

### 6. Test Install

Double-click `dist\codirigent.msi` and verify:

- [ ] Installer shows correct version
- [ ] `codirigent.exe` runs and reports the correct version
- [ ] `codirigent-hook.exe` is installed alongside
- [ ] PATH is updated (new terminal session required)

### 7. Revert Version (if changed)

If you modified `Cargo.toml` for testing, revert it:

```bash
git checkout Cargo.toml Cargo.lock
```

---

## Failure Handling

- **SimplySign not running:** signtool returns "No certificates were found".
  Launch SimplySign Desktop and log in.
- **Certificate not found:** Run `certmgr.msc` and confirm the Certum cert is
  under Personal > Certificates with the key icon.
- **Timestamp server unreachable:** Retry. If `time.certum.pl` is down, try
  `http://timestamp.digicert.com` as a fallback.
- **WiX candle/light fails:** Ensure `tools/wix/` exists. If missing, download
  WiX Toolset v3.14 binaries and extract to `tools/wix/`.
- **Version mismatch (MSI vs binary):** The `-dVersion` passed to WiX controls
  what Windows shows in Add/Remove Programs. The binary version comes from
  `Cargo.toml`. Both must be updated for a consistent release.

---

## Completion Standard

A Windows release is only complete when:

- [ ] CI workflow finished successfully
- [ ] MSI is signed with the Certum certificate
- [ ] `checksums-sha256.txt` includes the signed MSI hash
- [ ] Both files are uploaded to the GitHub Release
- [ ] The release page shows the correct file sizes (signed MSI is larger)
