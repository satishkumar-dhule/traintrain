# Full Desktop via VNC

Replit runs `Xvnc :1` on port `5901` with a minimal `fluxbox`. This repo upgrades it to a full desktop (panel, file manager, browsers, terminal, menu) while keeping the same `Xvnc :1`.

## Quick start (Replit)

```bash
# Fluxbox-enhanced (default, lighter, faster)
./scripts/desktop.sh

# Full XFCE session (if xfce packages installed)
./scripts/desktop.sh --xfce

# Custom resolution + auto-open browser
./scripts/desktop.sh --resolution 1920x1080 --with-browser
# Other resolutions: 1684x867, 1600x900, 1280x800, 1280x1024, 1024x768
```

What the script does (`scripts/desktop.sh:1-60`):
- Prefers `DISPLAY=:1` (Replit VNC) then `:0`, adds nix `xdpyinfo`/`xrandr` to `PATH`
- Sets resolution via `xrandr` on both `:1` and `:0` (`1920x1080` default)
- Kills minimal fluxbox, writes `~/.fluxbox/init|menu|keys`, restarts fluxbox with toolbar + 4 workspaces
- Starts `tint2` panel (taskbar + systray + clock) with fallback to `lxpanel`
- Starts `pcmanfm --desktop` (or `xfdesktop`) for desktop icons
- Creates `~/Desktop/RailwayRs.desktop` + `Terminal.desktop`, sets `Papirus`/`Arc-Dark` theme
- Autostarts `xfce4-terminal` (or `xterm`)

### Access

- **Replit VNC pane**: Open the `Desktop`/`VNC` tab (already wired to `:1` / `5901`). If blank, re-run `./scripts/desktop.sh`.
- Resolution is RandR-virtual; `Xvnc -geometry 800x600` is overridden by `xrandr -s 1920x1080`.

### Keybindings & menu (`scripts/desktop.sh:175-185`)

- `Alt+Tab` / `Alt+Shift+Tab` – next/prev window
- `Alt+F1..F4` – workspace 1..4
- `Ctrl+Alt+D` – `rofi -show drun` (app launcher)
- `Ctrl+Alt+T` – `xfce4-terminal`
- `Ctrl+Alt+F` – `thunar`
- `Ctrl+Alt+B` – `firefox`
- Right-click desktop → Fluxbox menu: Terminal, File Managers (`thunar`/`pcmanfm`), Browsers (`firefox`/`chromium`), Editor (`geany`), Run (`rofi`), `RailwayRs → firefox http://localhost:3000`, Appearance (`lxappearance`/`nitrogen`), System (`htop`), Restart/Exit.

### Nix dependencies (`.replit:10`)

```nix
packages = [
  "linuxquota" "tint2" "feh" "rofi" "pcmanfm" "lxappearance" "arc-theme" "papirus-icon-theme"
  "xfce.xfce4-terminal" "xfce.thunar" "firefox" "chromium" "geany" "flameshot" "xdotool"
  "xorg.xrandr" "xorg.xset" "xorg.xrdb" "hsetroot" "nitrogen" "lxde.lxpanel" "openbox"
  "tigervnc" "novnc"
]
```

If `tint2`/`pcmanfm`/`firefox` show `command not found`, the nix profile is empty (seen after home wipe). Trigger a Replit environment rebuild: change `.replit` (already done) and run `Reload` or `nix profile install` / wait for `Replit → System Dependencies` reinstall, then re-run `scripts/desktop.sh`.

### VNC workflow (`outputType: vnc`)

To show the desktop persistently in Replit:

1. `Tools → Workflows` (or `.replit` UI) → `Create Workflow`
2. `name: Desktop (VNC)`, `command: ./scripts/desktop.sh`, `outputType: vnc`, `autoStart: true`
3. Save – the VNC pane will auto-launch the full desktop on restart.

Or via code (when workflows skill is loaded):

```js
await configureWorkflow({
  name: "Desktop (VNC)",
  command: "./scripts/desktop.sh --resolution 1920x1080",
  outputType: "vnc",
  autoStart: true
});
```

## Docker (deployment)

### Option A – main `Dockerfile` with build-arg (`railway-rs/Dockerfile:12`)

```bash
docker build --build-arg WITH_VNC=1 -t railway-rs .
docker run -p 3000:3000 -p 6080:6080 -p 5901:5901 \
  -e DESKTOP=1 -e RESOLUTION=1280x800 railway-rs
```

- `ARG WITH_VNC=0` → `ENV WITH_VNC` (`Dockerfile:12-13`) gates `apt-get` VNC deps
- Runtime: `DESKTOP=1` or `WITH_VNC=1` starts `Xvfb :1` + `fluxbox` + `tint2` + `pcmanfm` + `x11vnc -rfbport 5901` + `websockify 6080 → 5901` (`Dockerfile:37-65`), exposes `3000 5901 6080`

### Option B – dedicated desktop image (`railway-rs/Dockerfile.desktop`)

```bash
docker build -f Dockerfile.desktop -t railway-rs:desktop .
docker run -p 3000:3000 -p 6080:6080 -p 5901:5901 railway-rs:desktop
# Override resolution: -e RESOLUTION=1920x1080
```

- Unconditional desktop install, `ENV RESOLUTION=1280x800 VNC_PORT=5901 NOVNC_PORT=6080`
- `http://localhost:6080/vnc.html?autoconnect=1&resize=remote` (noVNC) or VNC `localhost:5901`

Healthcheck remains `curl /healthz` on `PORT` (default `3000`).

## Troubleshooting

- `xdpyinfo: command not found` – script auto-adds `/nix/store/*xdpyinfo*/bin` to `PATH`; if still failing, `export PATH="$HOME/.nix-profile/bin:/nix/store/*xdpyinfo*/bin:/nix/store/vgnn*/bin:$PATH"` then retry.
- `tint2: command not found` / empty panel – nix packages not installed (profile empty). Rebuild env, then `rm ~/.nix-profile` check `nix profile list --json`.
- `Failed to read: session.*` in `/tmp/fluxbox.log` – benign defaults from `fluxbox -rc /nix/store/.../init` override.
- `Xvnc -geometry 800x600` vs `xrandr 1920x1080` – RandR overrides geometry; `xrandr` line shows `current 1920x1080*`.
