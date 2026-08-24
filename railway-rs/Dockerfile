# Builder stage
FROM rust:1-slim-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN cargo build --release --locked

# Runtime stage
FROM debian:bookworm-slim

ARG WITH_VNC=0

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && if [ "$WITH_VNC" = "1" ]; then \
         apt-get install -y --no-install-recommends \
           xvfb x11vnc x11-xserver-utils x11-utils \
           fluxbox tint2 pcmanfm openbox \
           lxappearance papirus-icon-theme arc-theme \
           firefox-esr chromium \
           xfce4-terminal thunar rofi feh hsetroot xdotool x11-apps dbus-x11 \
           novnc websockify net-tools procps; \
       fi \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /usr/sbin/nologin railway

WORKDIR /app
COPY --from=builder /app/target/release/railway-rs /app/railway-rs
COPY static/ /app/static/
COPY data/ /app/data/
# Optional local AI model (GGUF + tokenizer.json). Absent = local backend
# reports unavailable and deployments stay on the Zen gateway.
COPY models/ /app/models/

# VNC desktop entrypoint (only used when WITH_VNC=1 or DESKTOP=1 at runtime)
COPY <<'EOS' /usr/local/bin/start-vnc-desktop
#!/bin/bash
set -e
export DISPLAY=:1
RES=${RESOLUTION:-1280x800}
echo "[desktop] Starting Xvfb :1 -screen 0 ${RES}x24"
Xvfb :1 -screen 0 ${RES}x24 -ac +extension GLX +render -noreset &
sleep 1
fluxbox >/tmp/fluxbox.log 2>&1 &
sleep 1
tint2 >/tmp/tint2.log 2>&1 & || true
pcmanfm --desktop >/tmp/pcmanfm.log 2>&1 & || true
x11vnc -display :1 -nopw -forever -shared -rfbport 5901 -bg -o /tmp/x11vnc.log 2>&1 || true
websockify --web /usr/share/novnc 6080 localhost:5901 >/tmp/novnc.log 2>&1 & || true
hsetroot -solid "#0f172a" 2>/dev/null || xsetroot -solid "#0f172a" 2>/dev/null || true
xfce4-terminal --geometry 90x24 2>/dev/null & || xterm 2>/dev/null & || true
echo "[desktop] VNC ready: :1 -> 5901, noVNC http://localhost:6080/vnc.html"
EOS
COPY <<'EOS' /usr/local/bin/docker-entrypoint.sh
#!/bin/bash
set -e
if [ "${DESKTOP:-0}" = "1" ] || [ "${WITH_VNC:-0}" = "1" ]; then
  chmod +x /usr/local/bin/start-vnc-desktop
  /usr/local/bin/start-vnc-desktop &
  for i in 1 2 3 4 5; do DISPLAY=:1 xdpyinfo >/dev/null 2>&1 && break; sleep 1; done
fi
exec /app/railway-rs
EOS
RUN chmod +x /usr/local/bin/start-vnc-desktop /usr/local/bin/docker-entrypoint.sh || true

USER railway
EXPOSE 3000 5901 6080
ENV PORT=3000 \
    RAILWAY_LOG_DIR=/tmp/railway-logs
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -f http://127.0.0.1:${PORT:-3000}/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
