#!/bin/zsh
# Gera dist/bSound.app (com permissão de microfone própria) a partir do build release.
set -euo pipefail
cd "$(dirname "$0")/.."
export RUSTUP_HOME=${RUSTUP_HOME:-/Volumes/ExternalSSD/rust/rustup}
export CARGO_HOME=${CARGO_HOME:-/Volumes/ExternalSSD/rust/cargo}
export PATH="$CARGO_HOME/bin:$PATH"

cargo build --release
TARGET_DIR=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
APP=dist/bSound.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET_DIR/release/bsound" "$APP/Contents/MacOS/bsound"
cp branding/final/bSound.icns "$APP/Contents/Resources/bSound.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>bSound</string>
  <key>CFBundleDisplayName</key><string>bSound</string>
  <key>CFBundleIdentifier</key><string>br.miguelbalbo.bsound</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleExecutable</key><string>bsound</string>
  <key>CFBundleIconFile</key><string>bSound</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>O bSound mistura seu microfone com os sons e envia para o microfone virtual.</string>
  <key>NSAppleEventsUsageDescription</key><string>Usado para abrir o Terminal e instalar o BlackHole.</string>
</dict>
</plist>
PLIST
codesign --force --deep -s - "$APP"
echo "Pronto: $APP"
