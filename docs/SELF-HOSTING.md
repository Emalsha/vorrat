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

## Prebuilt image

Every version that passes CI on `main` is published as
`ghcr.io/emalsha/vorrat:latest`, and under its commit hash. To use it instead
of building, run `docker run -p 127.0.0.1:8087:8087 -v vorrat-data:/data ghcr.io/emalsha/vorrat`.
It is built for 64-bit x86; on a Raspberry Pi, build it yourself as above.

## Upgrading

`git pull`, then `docker compose … up -d --build`. With the prebuilt image,
`docker pull ghcr.io/emalsha/vorrat` and start the container again. The database is upgraded
automatically; a newer database refuses to open in an older Vorrat instead
of being damaged.

## Public demo

`VORRAT_DEMO=on` loads a sample household at start and every night and refuses
uploads, imports and settings changes. Don't set a password on a demo.

The project's own demo runs on a free Koyeb instance in Frankfurt. It needs no
disk, because the data is thrown away every night and on every restart:

1. Create a web service from the Docker image `ghcr.io/emalsha/vorrat:latest`,
   port 8087, with `VORRAT_DEMO=on`, `VORRAT_LISTEN=0.0.0.0:8087` and
   `VORRAT_LOOKUP=off`.
2. Create a Koyeb API token. In the GitHub repository, add it as the secret
   `KOYEB_TOKEN`, and add the service's ID as the variable `KOYEB_SERVICE_ID`.

After every merge to `main` that passes CI, the `release` workflow publishes a
new image and asks Koyeb to redeploy, so the demo always shows the latest
version. The same image runs on any host that runs containers.
