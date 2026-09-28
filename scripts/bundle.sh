#!/usr/bin/env bash
# Build a release binary and assemble <target>/Wallbar.app around it.
#
# The .app is not cosmetic: LSUIElement keeps wallbar out of the Dock, and
# SMAppService (start at login) needs a bundle identifier to register.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Honour CARGO_TARGET_DIR / .cargo/config.toml rather than assuming ./target.
TARGET_DIR="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT/Cargo.toml" \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
TARGET_DIR="${TARGET_DIR:-$ROOT/target}"

APP="$TARGET_DIR/Wallbar.app"
BIN="${WALLBAR_BIN:-$TARGET_DIR/release/wallbar}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"

if [ -z "${WALLBAR_BIN:-}" ]; then
  cargo build --release -p wallbar --manifest-path "$ROOT/Cargo.toml"
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/wallbar"
cp "$ROOT/assets/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>Wallbar</string>
	<key>CFBundleDisplayName</key>
	<string>Wallbar</string>
	<key>CFBundleExecutable</key>
	<string>wallbar</string>
	<key>CFBundleIdentifier</key>
	<string>com.gauthamv.wallbar</string>
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>${VERSION}</string>
	<key>CFBundleVersion</key>
	<string>1</string>
	<key>LSMinimumSystemVersion</key>
	<string>13.0</string>
	<key>LSUIElement</key>
	<true/>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
PLIST

# Sign with a stable identity when the machine has one, else ad hoc. wallbar
# touches no Keychain items, so an ad-hoc signature costs nothing but a
# fresh identity per build. Override with CODESIGN_IDENTITY.
IDENTITY="${CODESIGN_IDENTITY:-}"
if [ -z "$IDENTITY" ]; then
  IDENTITIES="$(security find-identity -v -p codesigning 2>/dev/null | sed -n 's/.*"\(.*\)"/\1/p')"
  IDENTITY="$(printf '%s\n' "$IDENTITIES" | grep -m1 '^Developer ID Application' || printf '%s\n' "$IDENTITIES" | head -n 1)"
fi
if [ -n "$IDENTITY" ]; then
  codesign --force --options runtime --sign "$IDENTITY" "$APP"
else
  echo "note: no codesigning identity found; signing ad hoc"
  codesign --force --sign - "$APP"
fi

echo "built $APP"
