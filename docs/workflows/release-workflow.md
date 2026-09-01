# Release Workflow

Step-by-step procedure for creating a new Codirigent release with
code-signed installers.

---

## Overview

Releases use a two-phase process:

1. **CI phase** — Push a git tag. GitHub Actions builds binaries, creates
   unsigned installers (MSI + DMG), and publishes a **draft** release.
2. **Local phase** — Sign the Windows MSI locally with the Certum
   certificate via SimplySign, update checksums, upload, then publish.

macOS DMG signing and notarization happen automatically in CI (Apple
Developer certificate is stored in GitHub Secrets).

---

## Prerequisites

- GitHub CLI (`gh`) authenticated
- SimplySign Desktop running and authenticated (provides the Certum
  code-signing certificate to the Windows certificate store)
- Windows SDK installed (provides `signtool.exe`)
- All changes committed and pushed to `main`

---

## Step 1: Create and Push the Tag

```bash
git tag -a v0.1.X -m "v0.1.X"
git push origin v0.1.X
```

This triggers the `Release` workflow (`.github/workflows/release.yml`),
which:

- Builds release binaries for Windows x64 and macOS ARM64
- Packages Windows `.zip` and `.msi` (unsigned)
- Packages macOS `.tar.gz` and `.dmg` (signed + notarized in CI)
- Generates `checksums-sha256.txt` (excludes `.msi` — it will be replaced)
- Creates a **draft** GitHub Release with all artifacts attached

### Monitor the workflow

```bash
gh run list --limit 3
gh run watch <run-id>
```

Wait for the workflow to complete successfully before proceeding.

---

## Step 2: Sign the Windows MSI

Run the signing script from the repo root:

```bash
powershell -File scripts/sign-and-upload.ps1 -Tag v0.1.X
```

The script performs these steps automatically:

1. Downloads the unsigned MSI from the draft release
2. Locates `signtool.exe` from the Windows SDK
3. Finds the Certum code-signing certificate (OID `1.3.6.1.5.5.7.3.3`)
   in `Cert:\CurrentUser\My` (provided by SimplySign)
4. Signs the MSI with SHA-256 and timestamps via `http://time.certum.pl`
5. Verifies the signature
6. Computes the SHA-256 checksum of the signed MSI
7. Downloads `checksums-sha256.txt`, replaces the MSI line, re-uploads
8. Uploads the signed MSI (overwrites the unsigned one)

### Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| "Certum code signing certificate not found" | SimplySign not running or not authenticated | Launch SimplySign Desktop and sign in |
| "signtool.exe not found" | Windows SDK not installed | Install Windows 10/11 SDK |
| "Failed to send Windows toast notification" | SimplySign session expired | Re-authenticate in SimplySign |

---

## Step 3: Verify the Draft Release

Check the draft release before publishing:

```bash
gh release view v0.1.X
```

Confirm:

- [ ] All expected assets are present (`.msi`, `.dmg`, `.zip`, `.tar.gz`,
      `.wixpdb`, `checksums-sha256.txt`)
- [ ] The MSI checksum in `checksums-sha256.txt` matches the signed file
- [ ] The release is still in **draft** status

---

## Step 4: Edit Release Notes

Update the draft release body with the changelog. Follow the format from
previous releases:

```
## What's New

### Feature Name
- Description of changes

---

## Bug Fixes
- Fix description (closes #N)

---

**Full Changelog**: https://github.com/oso95/Codirigent/compare/vPREV...vCURR
```

You can edit via the GitHub web UI or:

```bash
gh release edit v0.1.X --notes "$(cat release-notes.md)"
```

---

## Step 5: Publish

Once everything is verified, publish the draft:

```bash
gh release edit v0.1.X --draft=false
```

Or use the "Publish release" button in the GitHub web UI.

---

## Pre-release Tags

Tags containing `alpha`, `beta`, or `rc` are automatically marked as
pre-releases by the CI workflow. Use semantic naming:

```
v0.1.X-alpha.1    # early testing
v0.1.X-beta.1     # feature complete, testing
v0.1.X-rc.1       # release candidate
v0.1.X            # stable release
```

---

## Recovery

### If the workflow fails

```bash
# Delete the failed release (if created)
gh release delete v0.1.X --yes

# Delete the tag
git tag -d v0.1.X
git push origin :refs/tags/v0.1.X

# Fix the issue, then re-tag and push
git tag -a v0.1.X -m "v0.1.X"
git push origin v0.1.X
```

### If you need to re-sign the MSI

Just re-run the signing script — it uses `--clobber` to overwrite:

```bash
powershell -File scripts/sign-and-upload.ps1 -Tag v0.1.X
```

---

## File Reference

| File | Purpose |
|------|---------|
| `.github/workflows/release.yml` | CI workflow (build, package, draft release) |
| `scripts/sign-and-upload.ps1` | Local MSI signing and upload |
| `scripts/gen-license-rtf.ps1` | Generate `License.rtf` for WiX (used by CI) |
| `wix/main.wxs` | WiX installer definition |
| `tools/wix/` | Local WiX toolset (candle, light) |
