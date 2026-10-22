#!/bin/zsh
# Compila o bSound para Linux x86_64 e Windows x86_64 dentro de um container (Docker/Colima).
# Caches do cargo e artefatos ficam no SSD externo; os pacotes finais vão para dist/.
set -euo pipefail
cd "$(dirname "$0")/.."

CACHE=/Volumes/ExternalSSD/rust/docker
mkdir -p "$CACHE/registry" "$CACHE/target" dist

# Config Docker isolada: evita helpers de credencial de instalações antigas do Docker Desktop.
# O socket do contexto atual (ex.: Colima) é guardado antes, porque a config isolada não o tem.
export DOCKER_HOST="${DOCKER_HOST:-$(docker context inspect --format '{{.Endpoints.docker.Host}}')}"
export DOCKER_CONFIG="${DOCKER_CONFIG_OVERRIDE:-$CACHE/docker-config}"
mkdir -p "$DOCKER_CONFIG"
[ -f "$DOCKER_CONFIG/config.json" ] || echo '{}' > "$DOCKER_CONFIG/config.json"

docker build --platform linux/amd64 -t bsound-cross scripts/docker
docker run --rm --platform linux/amd64 \
    -v "$PWD:/src" \
    -v "$CACHE/registry:/usr/local/cargo/registry" \
    -v "$CACHE/target:/target" \
    -e CARGO_TARGET_DIR=/target \
    bsound-cross sh -c '
        set -e
        cargo build --release --target x86_64-unknown-linux-gnu
        cargo build --release --target x86_64-pc-windows-gnu
    '

VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)

# Linux: binário + ícone + .desktop
LNX=dist/bsound-$VERSION-linux-x86_64
rm -rf "$LNX" && mkdir -p "$LNX"
cp "$CACHE/target/x86_64-unknown-linux-gnu/release/bsound" "$LNX/"
cp assets/icon-256.png "$LNX/bsound.png"
cat > "$LNX/bsound.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=bSound
Comment=Soundboard com microfone virtual
Exec=bsound
Icon=bsound
Categories=AudioVideo;Audio;
DESKTOP
tar -C dist -czf "$LNX.tar.gz" "$(basename "$LNX")"

# Windows: .exe (ícone embutido)
WIN=dist/bsound-$VERSION-windows-x86_64
rm -rf "$WIN" && mkdir -p "$WIN"
cp "$CACHE/target/x86_64-pc-windows-gnu/release/bsound.exe" "$WIN/bSound.exe"
(cd dist && rm -f "$(basename "$WIN").zip" && zip -qr "$(basename "$WIN").zip" "$(basename "$WIN")")

ls -la dist/*.tar.gz dist/*.zip
