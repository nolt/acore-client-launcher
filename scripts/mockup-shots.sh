#!/usr/bin/env bash
# Builds the launcher in Docker and takes a screenshot of every MOCK state:
#   docs/mockups[/<lang>]/<state>.png  - full-quality PNGs (not in git)
#   docs/assets/<state>[.<lang>].jpg   - the images the README and docs use
# UI_LANG=pl_PL takes them in Polish (docs/assets/<state>.pl.jpg). Runs headless
# (Xvfb), nothing is installed on the host.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
image=acore-launcher-build
lang="${UI_LANG:-}"
shots="docs/mockups${lang:+/$lang}"
# "pl_PL" -> ".pl"; English has no suffix.
suffix="${lang:+.${lang%%_*}}"

echo "Preparing the build environment (the first run takes a few minutes)..."
docker build -q -t "$image" -f "$repo/docker/Dockerfile.build" "$repo/docker" >/dev/null

echo "Building the launcher and taking screenshots${lang:+ ($lang)}..."
docker run --rm -v "$repo:/src" -w /src -e CARGO_HOME=/src/target/cargo-home \
    -e LANG="${lang:+$lang.UTF-8}" -e SHOTS="$shots" -e SUFFIX="$suffix" "$image" bash -euo pipefail -c '
    cargo build --release --quiet
    mkdir -p "$SHOTS" docs/assets
    Xvfb :99 -screen 0 1024x768x24 >/dev/null 2>&1 &
    # FemtoVG (OpenGL, here Mesa llvmpipe) is what players get; the software
    # renderer fallback draws gradient fills without rounded corners.
    export DISPLAY=:99 SLINT_BACKEND=winit-femtovg SLINT_SCALE_FACTOR=1
    sleep 1
    for state in downloading paused ready no-wine error install install-error tools clear-cache about licenses; do
        MOCK=$state ./target/release/acore-launcher 2>/dev/null &
        pid=$!
        sleep 3
        import -window root -crop 760x475+0+0 +repage "$SHOTS/$state.png"
        convert "$SHOTS/$state.png" -strip -quality 88 "docs/assets/$state$SUFFIX.jpg"
        kill $pid
        wait $pid 2>/dev/null || true
        echo "  $state"
    done
    chown -R '"$(id -u):$(id -g)"' target docs/mockups docs/assets
'
echo "Screenshots: $repo/$shots/ and $repo/docs/assets/"
