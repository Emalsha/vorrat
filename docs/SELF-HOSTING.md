# Self-hosting Vorrat

Vorrat is one small program and one SQLite file. It needs about 10 MB of
memory and runs on a Raspberry Pi, a NAS or the smallest cloud server.

## Choose how people reach it

| Setup | Good for | HTTPS (needed for the camera scanner and the app icon) |
|---|---|---|
| Only on this computer | Trying it out | Not needed on `localhost` |
| Home network via Tailscale | A household, no open ports | `tailscale serve --bg 8087` gives a `https://….ts.net` address |
| Public server | Family in several places, a demo | Caddy gets a certificate automatically (below) |

Vorrat listens on `127.0.0.1` by default, so nothing is exposed until you choose to.

## Docker, public server with HTTPS

```sh
git clone https://github.com/Emalsha/vorrat && cd vorrat
# edit deploy/Caddyfile (your domain) and deploy/compose.caddy.yaml (password, URL)
docker compose -f deploy/compose.caddy.yaml up -d --build
```

Set `VORRAT_PASSWORD` on anything reachable from the internet. Sign-in
sessions last 30 days; a restart signs everyone out.

## Docker, home network only

```sh
docker compose -f compose.example.yaml up -d --build
tailscale serve --bg 8087    # optional: HTTPS inside your tailnet
```

## Without Docker

```sh
cargo build --release
VORRAT_DATA_DIR=/var/lib/vorrat ./target/release/vorrat
```

Run it as a systemd service or under any supervisor; it stops cleanly on Ctrl-C or SIGTERM.

## Raspberry Pi and other small machines

Vorrat is built for this. Any 64-bit Raspberry Pi (3, 4, 5 or Zero 2 W with a
64-bit OS) is enough: the service uses about 13 MB of memory and needs no
internet. With Docker, the same `compose.example.yaml` builds natively on the
Pi (the first build takes a while; later ones are cached). Without Docker,
build on a faster machine and copy the binary over:

```sh
rustup target add aarch64-unknown-linux-gnu
sudo apt install gcc-aarch64-linux-gnu
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
  cargo build --release --target aarch64-unknown-linux-gnu
scp target/aarch64-unknown-linux-gnu/release/vorrat pi@raspberrypi.local:
```

Keep the data folder on the SD card or, better, a USB SSD. Without internet,
everything works except filling in unknown barcodes from Open Food Facts and
push reminders; type the product in and read reminders on the home page.

## Backups

Set `VORRAT_BACKUP_DIR`. Every night Vorrat writes a consistent copy of the
database and an archive of attached files there and keeps `VORRAT_BACKUP_KEEP_DAYS`
(default 7). Let your normal backup tool (restic, Borg, a NAS snapshot) pick
up that folder. Settings → Your data also downloads the database file at any time.

**Restore:** stop Vorrat, put the backup `.db` file at `<data dir>/vorrat.db`,
unpack the `-files.tar.gz` archive inside `<data dir>` (it contains `files/`), start Vorrat.

## Upgrading

`git pull`, then `docker compose … up -d --build`. The database is upgraded
automatically; a newer database refuses to open in an older Vorrat instead
of being damaged.

## Public demo

`VORRAT_DEMO=on` loads a sample household at start and every night and refuses
uploads, imports and settings changes. Don't set a password on a demo.
