# Releasing updates

*[Wersja polska](releasing-updates.pl.md)*

The admin side: where the files live on your host, how to publish a client update and how to
publish a new launcher. For building the binaries see [Building](building.md); for the file
formats see [Manifest format](manifest-format.md).

## One-time configuration

Set `MANIFEST_URL` in [`crates/app/src/config.rs`](../crates/app/src/config.rs) to the full URL of
`version.json` on your host, e.g. `https://example.org/patch/version.json`, and build the
launcher. Everything else is found relative to it:

- the game files — next to `version.json`, or under `baseUrl` if the manifest sets one;
- `launcher.json` — next to `version.json` (`LAUNCHER_INFO` in `config.rs`).

## Host layout

Any web server that serves static files works. It should support `Range` requests (Apache and
nginx do) so interrupted and paused downloads resume instead of starting over.

```
/srv/patch/                  ← served as https://example.org/patch/
├── version.json             ← game manifest (MANIFEST_URL points here)
├── launcher.json            ← self-update info, next to version.json
├── launcher/
│   ├── acore-launcher       ← launcher binaries for self-update and for your download page
│   └── acore-launcher.exe
├── Wow.exe                  ← the client, exactly as players should have it
├── Data/…
└── …
```

### Apache

If your server already runs a website (for example
[acore-simple-web](https://github.com/nolt/acore-simple-web)), keep the client out of the
website's folder and map it with an alias, inside the site's `<VirtualHost>`:

```apache
Alias /patch /srv/patch
<Directory /srv/patch>
    Require all granted
    Options -Indexes
    AllowOverride None
    # only files to download, never run PHP from here
    php_admin_flag engine off
</Directory>
```

```bash
apache2ctl configtest && systemctl reload apache2
chmod -R a+rX /srv/patch        # the web server must be able to read every file
```

### nginx

```nginx
location /patch/ {
    alias /srv/patch/;
    autoindex off;
}
```

### A quick test server

For a test on your own machine, Python's built-in server is enough — see
[Building → Test it locally](building.md#test-it-locally-before-publishing). It cannot resume
downloads, so don't use it for players.

## Releasing a client update

1. Keep a **local folder with the client exactly as players should have it** — separate from the
   host folder, and without the launcher binaries.
2. Change what you need there (a new patch, a translation, `realmlist.wtf` …) and generate the
   manifest:

   ```bash
   out/dist/acore-launcher-cli manifest /path/to/client
   # /path/to/client/version.json: version 2026.09.28, 3412 files, 17.1 GB
   ```

   | Option | Meaning |
   |---|---|
   | `--version V` | The version players see as "Updated to V". Default: today's date (UTC), e.g. `2026.09.28`. Informational only — the hashes decide what is downloaded. |
   | `--base-url URL` | Where the files are served from, relative to the manifest. Default: next to it. |
   | `--output FILE` | Write the manifest somewhere else. Default: `version.json` inside the folder. |

   Hidden files and folders (names starting with `.`) are left out and listed, because web
   servers usually refuse to serve them.

3. **Upload the files first and `version.json` last**, so players never get a manifest pointing
   at files that are not there yet. `rsync` does it in two steps:

   ```bash
   rsync -av --exclude version.json /path/to/client/ host:/srv/patch/
   rsync -av /path/to/client/version.json host:/srv/patch/
   ```

At their next start, players download only the files whose hash changed. Files you removed from
the manifest stay on players' disks — the launcher never deletes game files.

> **Don't generate the manifest on the host folder.** It lists everything in the folder, so the
> `launcher/` binaries would become "game files" that every player downloads, and a launcher
> sitting in the root would be told to overwrite itself.

### Only your own files

The launcher does not care whether the manifest lists the whole client or just a few files. If
players bring their own 3.3.5a client, publish only what you add — the translation patch,
`realmlist.wtf`, fixes — and tell them to pick their existing WoW folder at the install step.
The launcher then downloads megabytes instead of the whole client.

## Releasing a new launcher

1. Raise `version` in the root [`Cargo.toml`](../Cargo.toml) (e.g. `0.1.0` → `0.2.0`) and run
   `scripts/build.sh`.
2. Copy the new binaries to the host, e.g. into `launcher/`, and generate `launcher.json` next to
   `version.json`:

   ```bash
   out/dist/acore-launcher-cli launcher 0.2.0 \
       --linux /srv/patch/launcher/acore-launcher \
       --windows /srv/patch/launcher/acore-launcher.exe \
       --output /srv/patch/launcher.json
   ```

   It hashes the binaries and stores their paths relative to `launcher.json`
   (`launcher/acore-launcher`). Either platform can be left out.
3. Upload the binaries first, then `launcher.json`.

At the next start, every launcher older than the published version downloads the new one,
replaces itself and restarts; a launcher with the same or a newer version does nothing. Without
`launcher.json` on the host there is simply no self-update.

## What players see

- The launcher checks for a new launcher, then for game files, on **every start**. There is no
  background service and nothing runs when the launcher is closed.
- While files download, the status line shows the file, bytes, speed and time left; players can
  pause and resume.
- After an update the status says "Updated to *version* — ready to play."

More in the [Player guide](player-guide.md).
