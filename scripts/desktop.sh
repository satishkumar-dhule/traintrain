#!/usr/bin/env bash
# Full desktop experience for Replit VNC (:1)
# - Kills minimal fluxbox, starts enhanced fluxbox + tint2 + pcmanfm desktop + full app suite
# - Sets 1920x1080 resolution, wallpaper, theme, and app autostart
# - Works on top of Replit's managed Xvnc (:1 @ 5901). No root needed.
# Usage: ./scripts/desktop.sh  [--xfce|--fluxbox] [--resolution 1920x1080] [--with-browser]
# Default: fluxbox-enhanced (lighter, faster). Use --xfce for full XFCE session if installed.
set -euo pipefail

MODE="fluxbox"
RES="1920x1080"
WITH_BROWSER=0
# Robust arg parsing: handles --resolution VALUE and bare 1920x1080
ARGS=("$@")
i=0
while (( i < $# )); do
  arg="${ARGS[i]}"
  case "$arg" in
    --xfce) MODE="xfce" ;;
    --fluxbox) MODE="fluxbox" ;;
    --with-browser) WITH_BROWSER=1 ;;
    --resolution)
      if (( i+1 < $# )); then
        RES="${ARGS[i+1]}"
        i=$((i+1))
      fi
      ;;
    1920*|1680*|1600*|1280*|1024*|800*) RES="$arg" ;;
  esac
  i=$((i+1))
done

# Ensure nix X tools are on PATH
export PATH="/nix/store/vgnn3flhbf3pgaj8bz8kr914fblqqjv6-replit-runtime-path/bin:$PATH"
export PATH="$HOME/.nix-profile/bin:/nix/var/nix/profiles/default/bin:$PATH"

# Prefer VNC display :1 if available, otherwise :0
# Use xrandr (fast, always in runtime) instead of xdpyinfo (slow nix glob)
check_display() {
  DISPLAY="$1" xrandr >/dev/null 2>&1 || DISPLAY="$1" xdpyinfo >/dev/null 2>&1
}
if check_display :1; then
  export DISPLAY=:1
elif check_display :0; then
  export DISPLAY=:0
else
  export DISPLAY=${DISPLAY:-:1}
  if ! check_display "$DISPLAY"; then
    echo "[desktop] No X display found. Is Xvnc running? (ps aux | grep Xvnc)"
    echo "          Replit auto-starts Xvnc :1. Wait a few seconds and retry."
    exit 1
  fi
fi
# If user explicitly sets DISPLAY, respect it but also configure the other display
VNC_DISPLAY=:1
MAIN_DISPLAY=:0

echo "[desktop] DISPLAY=$DISPLAY (VNC=$VNC_DISPLAY, MAIN=$MAIN_DISPLAY) MODE=$MODE RES=$RES"

# --- 1. Resolution ---
if command -v xrandr >/dev/null 2>&1; then
  echo "[desktop] Setting resolution $RES on $DISPLAY and $VNC_DISPLAY"
  for d in "$DISPLAY" "$VNC_DISPLAY" "$MAIN_DISPLAY"; do
    DISPLAY=$d xrandr -s "$RES" 2>/dev/null || DISPLAY=$d xrandr --output VNC-0 --mode "$RES" 2>/dev/null || true
  done
  echo "--- $DISPLAY ---"
  xrandr | grep -E "Screen|current|connected" | head -5
  echo "--- $VNC_DISPLAY ---"
  DISPLAY=$VNC_DISPLAY xrandr | grep -E "Screen|current|connected" | head -5 || true
fi

# --- 2. Kill old minimal fluxbox if needed (we replace it) ---
# Replit starts: fluxbox -rc /nix/store/.../init -log /tmp/fluxbox.log
# We keep Xvnc alive, just swap WM.
if pgrep -x fluxbox >/dev/null 2>&1; then
  echo "[desktop] Stopping old fluxbox..."
  pkill -x fluxbox || true
  sleep 0.8
fi
# also stop any stale xfce components
pkill xfce4-panel 2>/dev/null || true
pkill xfdesktop 2>/dev/null || true
pkill pcmanfm 2>/dev/null || true
pkill tint2 2>/dev/null || true
sleep 0.5

# --- 3. Prepare dirs & theme ---
mkdir -p "$HOME/.fluxbox" "$HOME/.config/tint2" "$HOME/Desktop" "$HOME/.config/pcmanfm/default"
XRDB="$HOME/.Xresources"
cat > "$XRDB" <<'EOF'
Xft.antialias: true
Xft.hinting: true
Xft.rgba: rgb
Xft.hintstyle: hintslight
Xft.dpi: 96
EOF
xrdb -merge "$XRDB" 2>/dev/null || true
xset r rate 250 35 2>/dev/null || true
xset b off 2>/dev/null || true

# Wallpaper - solid color (avoid expensive /nix/store globs)
if command -v hsetroot >/dev/null 2>&1; then
  hsetroot -solid "#0f172a" 2>/dev/null || true
else
  xsetroot -solid "#0f172a" 2>/dev/null || true
fi

# --- 4. Launch desktop ---
if [[ "$MODE" == "xfce" ]]; then
  echo "[desktop] Starting XFCE session..."
  # XFCE needs dbus; start if not running
  if ! pgrep -x dbus-daemon >/dev/null 2>&1; then
    dbus-launch --sh-syntax >/tmp/dbus.env 2>/dev/null || true
    # shellcheck source=/dev/null
    source /tmp/dbus.env 2>/dev/null || true
  fi
  # xfce session (if available) otherwise manual components
  if command -v xfce4-session >/dev/null 2>&1; then
    # Use xfce session; it will manage xfwm4 + panel + desktop
    xfce4-session >/tmp/xfce.log 2>&1 &
  elif command -v xfwm4 >/dev/null 2>&1; then
    xfwm4 --replace >/tmp/xfwm.log 2>&1 &
    sleep 1
    xfdesktop >/tmp/xfdesktop.log 2>&1 &
    xfce4-panel >/tmp/xfce-panel.log 2>&1 &
  else
    echo "[desktop] XFCE not installed, falling back to fluxbox-enhanced"
    MODE="fluxbox"
  fi
fi

if [[ "$MODE" == "fluxbox" ]]; then
  echo "[desktop] Starting Fluxbox enhanced desktop..."

  # Fluxbox config - use Meta (avoid expensive /nix/store globs)
  FLUX_STYLE="Meta"
  # Write minimal fluxbox init with toolbar visible + slit
  cat > "$HOME/.fluxbox/init" <<EOF
session.screen0.toolbar.widthPercent: 100
session.screen0.toolbar.tools: workspacename, prevworkspace, nextworkspace, iconbar, systemtray, clock
session.screen0.toolbar.visible: true
session.screen0.toolbar.onhead: 0
session.screen0.toolbar.layer: Dock
session.screen0.toolbar.alpha: 255
session.styleFile: $FLUX_STYLE
session.menuFile: $HOME/.fluxbox/menu
session.keyFile: $HOME/.fluxbox/keys
session.screen0.workspaces: 4
session.screen0.workspaceNames: One,Two,Three,Four,
session.screen0.edgeSnapThreshold: 10
session.screen0.focusModel: ClickFocus
session.screen0.windowPlacement: RowSmartPlacement
EOF

  cat > "$HOME/.fluxbox/menu" <<'EOF'
[begin] (Fluxbox - Full Desktop)
  [exec] (Terminal - xfce4-terminal) {xfce4-terminal} <>
  [exec] (Terminal - xterm) {xterm} <>
  [exec] (File Manager - Thunar) {thunar} <>
  [exec] (File Manager - PCManFM) {pcmanfm} <>
  [exec] (Browser - Firefox) {firefox} <>
  [exec] (Browser - Chromium) {chromium} <>
  [exec] (Editor - Geany) {geany} <>
  [exec] (Run Command - Rofi) {rofi -show drun} <>
  [exec] (RailwayRs - http://localhost:3000) {firefox http://localhost:3000} <>
  [separator]
  [submenu] (Appearance)
    [exec] (LXAppearance) {lxappearance} <>
    [exec] (Set Wallpaper - Nitrogen) {nitrogen} <>
  [end]
  [submenu] (System)
    [exec] (Task Manager - htop) {xfce4-terminal -e htop} <>
    [exec] (Desktop Info) {xfce4-terminal -e "echo DISPLAY=$DISPLAY; xrandr; ps aux | head -20; read"} <>
  [end]
  [separator]
  [restart] (Restart Fluxbox) {fluxbox} <>
  [exit] (Exit) {exit} <>
[end]
EOF

  cat > "$HOME/.fluxbox/keys" <<'EOF'
Mod1 Tab :NextWindow
Mod1 Shift Tab :PrevWindow
Mod1 F1 :Workspace 1
Mod1 F2 :Workspace 2
Mod1 F3 :Workspace 3
Mod1 F4 :Workspace 4
Control Mod1 d :Exec rofi -show drun
Control Mod1 t :Exec xfce4-terminal
Control Mod1 f :Exec thunar
Control Mod1 b :Exec firefox
EOF

  # Start fluxbox
  fluxbox -log /tmp/fluxbox.log >/tmp/fluxbox.out 2>&1 &
  sleep 1.2

  # Tint2 panel (bottom) - provides taskbar, systray, clock
  # Use lxpanel as fallback if tint2 not available
  if command -v tint2 >/dev/null 2>&1; then
    cat > "$HOME/.config/tint2/tint2rc" <<'EOF'
panel_position = bottom center horizontal
panel_size = 100% 30
panel_layer = top
panel_margin = 0 0
panel_padding = 2 2 2
panel_background_id = 1
wm_menu = 1
panel_dock = 0
panel_items = LTSC
autohide = 0
strut_policy = follow_size
# Taskbar
taskbar_mode = multi_desktop
taskbar_padding = 2 2 2
taskbar_background_id = 0
taskbar_active_background_id = 0
# Task
task_icon = 1
task_text = 1
task_centered = 1
task_padding = 2 2
task_background_id = 0
task_active_background_id = 2
task_urgent_background_id = 2
task_iconified_background_id = 0
# System tray
systray_padding = 2 2 2
systray_background_id = 0
systray_sort = ascending
# Launcher
launcher_padding = 2 2 2
launcher_background_id = 0
launcher_icon_size = 22
launcher_item_app = /run/current-system/sw/share/applications/firefox.desktop
launcher_item_app = /run/current-system/sw/share/applications/thunar.desktop
launcher_item_app = /run/current-system/sw/share/applications/xfce4-terminal.desktop
# Clock
time1_format = %H:%M
time1_font = sans 10
clock_font_color = #ffffff 100
clock_padding = 4 0
clock_background_id = 0
# Backgrounds
rounded = 0
border_width = 0
background_color = #1e293b 100
border_color = #334155 100
rounded = 4
border_width = 1
background_color = #334155 100
border_color = #475569 100
rounded = 4
border_width = 0
background_color = #0ea5e9 30
border_color = #0ea5e9 100
EOF
    tint2 -c "$HOME/.config/tint2/tint2rc" >/tmp/tint2.log 2>&1 &
  elif command -v lxpanel >/dev/null 2>&1; then
    lxpanel >/tmp/lxpanel.log 2>&1 &
  fi

  # Desktop icons via pcmanfm --desktop (or xfdesktop)
  if command -v pcmanfm >/dev/null 2>&1; then
    pcmanfm --desktop --profile default >/tmp/pcmanfm.log 2>&1 &
  elif command -v xfdesktop >/dev/null 2>&1; then
    xfdesktop >/tmp/xfdesktop.log 2>&1 &
  fi
fi

# --- 5. Autostart useful apps ---
sleep 1
# Desktop shortcut for RailwayRs
cat > "$HOME/Desktop/RailwayRs.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Train Bro (RailwayRs)
Comment=Open http://localhost:3000
Exec=firefox http://localhost:3000
Icon=firefox
Terminal=false
Categories=Network;
EOF
chmod +x "$HOME/Desktop/RailwayRs.desktop" 2>/dev/null || true

cat > "$HOME/Desktop/Terminal.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Terminal
Exec=xfce4-terminal
Icon=utilities-terminal
Terminal=false
EOF
chmod +x "$HOME/Desktop/Terminal.desktop" 2>/dev/null || true

# Apply icon theme if lxappearance available
if command -v gsettings >/dev/null 2>&1; then
  gsettings set org.gnome.desktop.interface icon-theme 'Papirus' 2>/dev/null || true
  gsettings set org.gnome.desktop.interface gtk-theme 'Arc-Dark' 2>/dev/null || true
fi

# Launch a terminal by default so VNC isn't empty
if command -v xfce4-terminal >/dev/null 2>&1; then
  xfce4-terminal --geometry 90x24 >/tmp/term.log 2>&1 &
elif command -v xterm >/dev/null 2>&1; then
  xterm -geometry 90x24 >/tmp/term.log 2>&1 &
fi

if [[ "$WITH_BROWSER" == "1" ]]; then
  if command -v firefox >/dev/null 2>&1; then
    firefox http://localhost:3000 >/tmp/firefox.log 2>&1 &
  elif command -v chromium >/dev/null 2>&1; then
    chromium --no-sandbox http://localhost:3000 >/tmp/chromium.log 2>&1 &
  fi
fi

echo "[desktop] Done. DISPLAY=$DISPLAY"
echo "[desktop] WM: $(ps aux | grep -E 'fluxbox|xfwm4|openbox' | grep -v grep | head -1 || echo 'none')"
echo "[desktop] Panel: $(ps aux | grep -E 'tint2|lxpanel|xfce4-panel' | grep -v grep | head -1 || echo 'none')"
echo "[desktop] File manager: $(ps aux | grep -E 'pcmanfm|thunar|xfdesktop' | grep -v grep | head -1 || echo 'none')"
xrandr | grep -E "current|*"
echo "[desktop] Right-click desktop for menu | Alt+Tab switch | Ctrl+Alt+T terminal | Ctrl+Alt+D rofi"
echo "[desktop] Workspaces: 4 | Panel at bottom | System tray enabled"
