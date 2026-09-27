#!/usr/bin/env bash
# Builds the launcher in Docker and saves a screenshot of every mockup state
# to docs/mockups/. Runs headless (Xvfb), nothing is installed on the host.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
image=acore-launcher-build

echo "Preparing the build environment (the first run takes a few minutes)..."
docker build -q -t "$image" -f "$repo/docker/Dockerfile.build" "$repo/docker" >/dev/null

docker run --rm -v "$repo:/src" -w /src -e CARGO_HOME=/src/target/cargo-home "$image" bash -euo pipefail -c '
    cargo build --release
    mkdir -p docs/mockups
    Xvfb :99 -screen 0 1024x768x24 >/dev/null 2>&1 &
    # FemtoVG (OpenGL, here Mesa llvmpipe) is what players get; the software
    # renderer fallback draws gradient fills without rounded corners.
    export DISPLAY=:99 SLINT_BACKEND=winit-femtovg SLINT_SCALE_FACTOR=1
    sleep 1
    for state in downloading ready error install install-error tools clear-cache about; do
        MOCK=$state ./target/release/acore-launcher &
        pid=$!
        sleep 3
        import -window root -crop 760x475+0+0 +repage "docs/mockups/$state.png"
        kill $pid
        wait $pid 2>/dev/null || true
    done
    chown -R '"$(id -u):$(id -g)"' target docs/mockups
'
ls -la "$repo/docs/mockups"
