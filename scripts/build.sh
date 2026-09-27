#!/usr/bin/env bash
# Release build for Linux and Windows in Docker. Prints one line per stage;
# the full output goes to out/build.log. The binaries land in out/dist/.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
image=acore-launcher-build
dist="$repo/out/dist"
log="$repo/out/build.log"
stages=4
stage=0

mkdir -p "$dist"
: >"$log"

in_container() {
    docker run --rm -v "$repo:/src" -w /src -e CARGO_HOME=/src/target/cargo-home "$image" bash -euo pipefail -c "$1"
}

# Files written by the container belong to root; hand them back, also after a failure.
give_back() {
    docker run --rm -v "$repo:/src" "$image" chown -R "$(id -u):$(id -g)" /src/target /src/out 2>/dev/null || true
}
trap give_back EXIT

duration() {
    local s=$1
    if ((s < 60)); then echo "${s} s"; else echo "$((s / 60)) min $((s % 60)) s"; fi
}

# run_stage "Title" command...: one status line, a dot every 15 s while it runs.
run_stage() {
    local title=$1
    shift
    stage=$((stage + 1))
    printf '[%d/%d] %s ' "$stage" "$stages" "$title"
    echo "== [$stage/$stages] $title" >>"$log"
    local start=$SECONDS
    "$@" >>"$log" 2>&1 &
    local pid=$!
    while kill -0 "$pid" 2>/dev/null; do
        sleep 1
        (((SECONDS - start) % 15 == 0)) && printf '.'
    done
    if wait "$pid"; then
        echo " done ($(duration $((SECONDS - start))))"
    else
        echo " FAILED"
        echo
        tail -n 30 "$log"
        echo
        echo "Build failed at stage $stage/$stages. Full log: $log"
        exit 1
    fi
}

total=$SECONDS
echo "Building acore-launcher. The first build takes several minutes, later ones are quick."
run_stage "Preparing the build environment" \
    docker build --progress=plain -t "$image" -f "$repo/docker/Dockerfile.build" "$repo/docker"
run_stage "Downloading dependencies" in_container 'cargo fetch --locked'
run_stage "Compiling for Linux" in_container 'cargo build --release --locked'
run_stage "Compiling for Windows" in_container '
    cargo build --release --locked --target x86_64-pc-windows-gnu
    cp target/release/acore-launcher target/release/acore-launcher-cli out/dist/
    cp target/x86_64-pc-windows-gnu/release/acore-launcher.exe \
       target/x86_64-pc-windows-gnu/release/acore-launcher-cli.exe out/dist/'

echo
echo "Done in $(duration $((SECONDS - total))). Files in $dist:"
for entry in \
    "acore-launcher:launcher for Linux players" \
    "acore-launcher.exe:launcher for Windows players" \
    "acore-launcher-cli:manifest and launcher.json tool (Linux)" \
    "acore-launcher-cli.exe:manifest and launcher.json tool (Windows)"; do
    name=${entry%%:*}
    size=$(du -h "$dist/$name" | cut -f1)
    printf '  %-24s %6s  %s\n' "$name" "$size" "${entry#*:}"
done
echo "Full log: $log"
