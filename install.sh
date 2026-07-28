#!/usr/bin/env bash
#
# Install the DevTools KRunner runner for the current user.
#
#   ./install.sh
#
# It builds the Rust binary, copies it to ~/.local/bin, drops the KRunner
# DBus-runner metadata + D-Bus activation service in place, and restarts
# KRunner. No root required.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# --- Rust toolchain (user-local; install if missing) ----------------------
if ! command -v cargo >/dev/null 2>&1; then
    # shellcheck disable=SC1091
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "==> Installing Rust toolchain (rustup, minimal profile)..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --no-modify-path
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
fi

# --- Build ----------------------------------------------------------------
echo "==> Building devtools-runner (release)..."
( cd "$SCRIPT_DIR" && cargo build --release )
BIN="$SCRIPT_DIR/target/release/devtools-runner"

# --- Install the binary ---------------------------------------------------
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
install -m 0755 "$BIN" "$INSTALL_DIR/devtools-runner"
echo "==> binary -> $INSTALL_DIR/devtools-runner"

# --- KRunner DBus-runner metadata -----------------------------------------
KRUNNER_DIR="$HOME/.local/share/krunner/dbusplugins"
mkdir -p "$KRUNNER_DIR"
install -m 0644 "$SCRIPT_DIR/assets/org.kde.devtools.desktop" "$KRUNNER_DIR/"
echo "==> krunner plugin -> $KRUNNER_DIR/org.kde.devtools.desktop"

# --- D-Bus activation service (substitute the absolute binary path) -------
DBUS_DIR="$HOME/.local/share/dbus-1/services"
mkdir -p "$DBUS_DIR"
sed "s|@EXEC@|$INSTALL_DIR/devtools-runner|g" \
    "$SCRIPT_DIR/assets/org.kde.devtools.service" \
    > "$DBUS_DIR/org.kde.devtools.service"
echo "==> dbus service -> $DBUS_DIR/org.kde.devtools.service"

# --- Make sure DBus-activated services can reach the Wayland/X display ----
# (Plasma imports these at login; this is an idempotent safety net.)
dbus-update-activation-environment \
    WAYLAND_DISPLAY DISPLAY XAUTHORITY XDG_SESSION_TYPE >/dev/null 2>&1 || true

# --- Stop any running instance so the fresh binary / activation is used ---
killall devtools-runner >/dev/null 2>&1 || true

# --- Restart KRunner so it picks up the new runner metadata ---------------
if command -v kquitapp6 >/dev/null 2>&1; then
    kquitapp6 krunner >/dev/null 2>&1 || true
fi
sleep 1
# Plasma respawns KRunner; start it explicitly as a fallback.
if ! qdbus6 org.kde.krunner /App >/dev/null 2>&1; then
    ( setsid krunner >/dev/null 2>&1 & ) || true
    sleep 1
fi

cat <<EOF

Installed. Open KRunner (Alt+Space) and try:
    date       (or  time / da / tim)
Press Enter to copy the selected value.

To open KRunner filtered to this runner from a terminal:
    qdbus6 org.kde.krunner /App org.kde.krunner.App.querySingleRunner org.kde.devtools date
EOF
