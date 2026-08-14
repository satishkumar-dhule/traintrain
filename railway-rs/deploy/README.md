# railway-rs deployment guide

`railway-rs` is an axum backend that serves a static SPA and a JSON API. All
configuration comes from environment variables; every variable is optional and
has a sane default, so the app runs with zero configuration.

## Prerequisites

You need a `railway-rs` binary. Build it from source with a Rust toolchain:

```
cargo build --release
# binary: target/release/railway-rs
```

or produce a Docker image following your existing image build process.

## Install steps

1. Create a dedicated system user:

   ```
   sudo useradd --system --home /opt/railway-rs --shell /usr/sbin/nologin railway
   ```

2. Create the install directory and copy the binary plus the `static/` and
   `data/` directories alongside it:

   ```
   sudo mkdir -p /opt/railway-rs
   sudo cp target/release/railway-rs /opt/railway-rs/
   sudo cp -r static /opt/railway-rs/
   sudo cp -r data /opt/railway-rs/
   sudo chown -R railway:railway /opt/railway-rs
   ```

3. Write the environment file. It is optional (all values have defaults) but
   lets you override port, directories, timeouts, and upstream source URLs:

   ```
   sudo mkdir -p /etc/railway-rs
   sudo install -o root -g root -m 640 /dev/null /etc/railway-rs/railway-rs.env
   ```

   See `.env.example` for every variable and its default.

4. Install and start the systemd unit:

   ```
   sudo cp deploy/railway-rs.service /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable railway-rs
   sudo systemctl start railway-rs
   ```

5. Verify it is healthy:

   ```
   curl -f http://127.0.0.1:3000/healthz
   ```

## Environment variables

| Variable                          | Default                                           | Description                                          |
| --------------------------------- | ------------------------------------------------- | ---------------------------------------------------- |
| `RAILWAY_PORT`                    | `3000`                                            | TCP port the HTTP server listens on                  |
| `RAILWAY_DATA_DIR`                | `./data`                                          | Directory containing `stations.json` and `trains.json` |
| `RAILWAY_STATIC_DIR`              | `./static`                                        | Directory with the SPA static files                  |
| `RAILWAY_HTTP_TIMEOUT`            | `15` (seconds)                                    | Timeout for outbound upstream HTTP requests          |
| `RAILWAY_CACHE_TTL`               | `120` (seconds)                                   | TTL for cached upstream responses                    |
| `RAILWAY_USER_AGENT`              | a current Chrome desktop UA                       | `User-Agent` header sent to upstream sources         |
| `RAILWAY_SOURCE_RAILYATRI_BASE`   | `https://www.railyatri.in`                        | Base URL of the RailYatri upstream source            |
| `RAILWAY_SOURCE_ETRAIN_BASE`      | `https://etrain.info`                             | Base URL of the etrain.info upstream source          |
| `RAILWAY_SOURCE_NTES_BASE`        | `https://enquiry.indianrail.gov.in`               | Base URL of the NTES (Indian Railways enquiry) source |

All variables are optional. `RAILWAY_HTTP_TIMEOUT` and `RAILWAY_CACHE_TTL` are
parsed as seconds. The app needs no API keys: it reads only public, free
sources.

## Operations

Follow the service log:

```
journalctl -u railway-rs -f
```

Restart the service after configuration or deployment changes:

```
sudo systemctl restart railway-rs
```

Check status:

```
sudo systemctl status railway-rs
```

## Upgrading

1. Build the new binary:

   ```
   cargo build --release
   ```

2. Replace the binary and refresh static assets:

   ```
   sudo cp target/release/railway-rs /opt/railway-rs/
   sudo cp -r static /opt/railway-rs/
   sudo chown -R railway:railway /opt/railway-rs
   ```

3. Restart the service:

   ```
   sudo systemctl restart railway-rs
   ```

Note: the service runs with `ProtectSystem=strict` and `ReadWritePaths=/opt/railway-rs`,
so any cache/data writes are confined to `/opt/railway-rs`. The application
needs no API keys and uses only public, free data sources.
