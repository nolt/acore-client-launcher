#!/usr/bin/env bash
# Smoke test of the Windows build under Wine: first-run install from Downloads,
# shortcuts, deleting the original. Screenshots and a file listing land in
# out/wine-test/. Needs target/x86_64-pc-windows-gnu/release/acore-launcher.exe
# (see docs/building.md). Runs in a throw-away container.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
exe="$repo/target/x86_64-pc-windows-gnu/release/acore-launcher.exe"
out="$repo/out/wine-test"
[[ -f $exe ]] || { echo "missing $exe - build the Windows target first" >&2; exit 1; }
mkdir -p "$out"

# Debian trixie: Wine 10. Wine 8 (bookworm) lacks bcryptprimitives.dll,
# which Rust programs need on Windows.
docker run --rm -v "$repo:/src:ro" -v "$out:/out" debian:trixie bash -euo pipefail -c '
    apt-get update -qq >/dev/null
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
        wine wine64 xserver-xorg-core xserver-xorg-video-dummy imagemagick procps xdotool libgl1-mesa-dri \
        >/dev/null 2>&1
    export DISPLAY=:99 WINEDEBUG=err+all,fixme-all WINEPREFIX=/tmp/wp
    Xorg :99 -config /src/docker/xorg-dummy.conf -noreset -logfile /tmp/xorg.log >/dev/null 2>&1 &
    sleep 3
    wineboot -i >/dev/null 2>&1
    wineserver -w

    home=/tmp/wp/drive_c/users/root
    mkdir -p "$home/Downloads"
    cp /src/target/x86_64-pc-windows-gnu/release/acore-launcher.exe "$home/Downloads/"
    wine "C:\\users\\root\\Downloads\\acore-launcher.exe" >/out/wine.log 2>&1 &
    sleep 25
    import -window root -crop 760x475+0+0 +repage /out/1-install.png

    # "Install" in the first-run dialog.
    xdotool mousemove 540 358 click 1
    sleep 25
    import -window root -crop 760x475+0+0 +repage /out/2-installed.png

    {
        echo "== files"
        find /tmp/wp/drive_c/Games "$home/Downloads" "$home/Desktop" \
            "$home/AppData/Roaming/Microsoft/Windows/Start Menu/Programs" -maxdepth 2 \
            -not -path "*Administrative Tools*" -not -path "*StartUp*"
        echo "== running"
        ps -eo args | grep -i "acore-launcher" | grep -v grep || true
    } | tee /out/result.txt
    chown -R '"$(id -u):$(id -g)"' /out
'
echo "screenshots and logs: $out"
