#!/usr/bin/env bash
# Regenerates THIRD-PARTY-LICENSES.md from the dependency tree with cargo-about
# (config: about.toml, template: about.hbs). Run after changing dependencies.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
image=acore-launcher-build

echo "Preparing the build environment (the first run takes a few minutes)..."
docker build -q -t "$image" -f "$repo/docker/Dockerfile.build" "$repo/docker" >/dev/null

docker run --rm -v "$repo:/src" -w /src -e CARGO_HOME=/src/target/cargo-home "$image" bash -euo pipefail -c '
    [[ -x target/cargo-home/bin/cargo-about ]] || cargo install --locked --features cli cargo-about
    target/cargo-home/bin/cargo-about generate --workspace --fail about.hbs -o THIRD-PARTY-LICENSES.md
    # The Slint Royalty-free text, which cargo-about leaves out (see about.toml).
    slint=$(cargo metadata --format-version 1 --quiet |
        grep -o "\"manifest_path\":\"[^\"]*/slint-[0-9.]*/Cargo.toml\"" | head -1 | cut -d\" -f4)
    {
        echo "## Slint Royalty-free License 2.0"
        echo
        echo "Used by: slint and its i-slint-* crates"
        echo
        echo "\`\`\`text"
        cat "$(dirname "$slint")/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md"
        echo "\`\`\`"
    } >> THIRD-PARTY-LICENSES.md
    chown -R '"$(id -u):$(id -g)"' target THIRD-PARTY-LICENSES.md
'
echo "$repo/THIRD-PARTY-LICENSES.md"
