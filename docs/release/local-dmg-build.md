# Local macOS DMG Build

This documents the local unsigned DMG build flow for testing on macOS.

The result is a drag-to-install DMG that contains:

- `Codirigent.app`
- `Applications -> /Applications`

The bundled app also includes both binaries the app needs at runtime:

- `Codirigent.app/Contents/MacOS/codirigent`
- `Codirigent.app/Contents/MacOS/codirigent-hook`

## Prerequisites

- macOS
- Rust toolchain
- Xcode command line tools (`hdiutil`, `sips`, `iconutil`)

## Build

From the repo root:

```bash
TARGET="aarch64-apple-darwin"
VERSION="0.1.0"

cargo build --profile dist --features gpui-full --target "$TARGET" -p codirigent
cargo build --profile dist --target "$TARGET" -p codirigent-hook
```

## Package an unsigned DMG

This uses a repo-local temporary staging directory so it does not leave
`Codirigent.app`, `AppIcon.iconset`, or `dmg-staging` clutter in the repo root.

If an older Codirigent DMG is still mounted, detach it first:

```bash
for vol in "/Volumes/Codirigent 1" "/Volumes/Codirigent"; do
  if [ -e "$vol" ]; then
    hdiutil detach "$vol" -quiet || hdiutil detach "$vol" -force -quiet || true
  fi
done
```

Then package:

```bash
set -euo pipefail

TARGET="aarch64-apple-darwin"
VERSION="0.1.0"
OUT_DMG="dist/codirigent-v${VERSION}-${TARGET}-unsigned.dmg"
TMP_ROOT="dist/.packtmp"
BUNDLE="$TMP_ROOT/Codirigent.app"
ICONSET="$TMP_ROOT/AppIcon.iconset"
STAGING="$TMP_ROOT/dmg-staging"

rm -rf "$TMP_ROOT"

mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"
cp "target/${TARGET}/dist/codirigent" "$BUNDLE/Contents/MacOS/"
cp "target/${TARGET}/dist/codirigent-hook" "$BUNDLE/Contents/MacOS/"

cat > "$BUNDLE/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key><string>codirigent</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundleIdentifier</key><string>com.codirigent.app</string>
    <key>CFBundleName</key><string>Codirigent</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundleVersion</key><string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

mkdir -p "$ICONSET"
ICON_SRC="assets/icons/app-icon-preview.png"
sips -z 16 16 "$ICON_SRC" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32 "$ICON_SRC" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$ICON_SRC" --out "$ICONSET/icon_32x32.png" >/dev/null
sips -z 64 64 "$ICON_SRC" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$ICON_SRC" --out "$ICONSET/icon_128x128.png" >/dev/null
sips -z 256 256 "$ICON_SRC" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$ICON_SRC" --out "$ICONSET/icon_256x256.png" >/dev/null
sips -z 512 512 "$ICON_SRC" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$ICON_SRC" --out "$ICONSET/icon_512x512.png" >/dev/null
sips -z 1024 1024 "$ICON_SRC" --out "$ICONSET/icon_512x512@2x.png" >/dev/null
iconutil -c icns "$ICONSET" -o "$BUNDLE/Contents/Resources/AppIcon.icns"

mkdir -p "$STAGING"
cp -R "$BUNDLE" "$STAGING/"
ln -s /Applications "$STAGING/Applications"

rm -f "$OUT_DMG"
hdiutil create -volname "Codirigent" -srcfolder "$STAGING" -ov -format UDZO "$OUT_DMG"

rm -rf "$TMP_ROOT"

echo "$OUT_DMG"
```

## Verify the DMG

Mount it once and check the DMG root and app bundle contents:

```bash
MNT=$(mktemp -d /tmp/codirigent-dmg.XXXXXX)
hdiutil attach -nobrowse -mountpoint "$MNT" dist/codirigent-v0.1.0-aarch64-apple-darwin-unsigned.dmg >/dev/null

ls -la "$MNT"
ls -la "$MNT/Codirigent.app/Contents/MacOS"

hdiutil detach "$MNT" -quiet
rmdir "$MNT"
```

Expected root contents:

- `Codirigent.app`
- `Applications -> /Applications`

Expected app bundle contents:

- `codirigent`
- `codirigent-hook`

## Notes

- This is unsigned and not notarized.
- Gatekeeper may warn on first launch.
- The signed/notarized release flow still lives in [release.yml](/Users/cyw/Desktop/github/Dirigent/.github/workflows/release.yml).
