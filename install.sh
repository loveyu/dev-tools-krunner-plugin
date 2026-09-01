#!/usr/bin/env bash
#
# Install the DevTools KRunner runner for the current user.
#
#   ./install.sh
#
# It builds the Vue UI and Rust workspace, copies both binaries to
# ~/.local/bin, installs the KRunner and Worker D-Bus activation services,
# and restarts KRunner. No root required.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_DIR="$SCRIPT_DIR/web/devtools-ui"
PREBUILT=false
if [ ! -f "$SCRIPT_DIR/Cargo.toml" ] \
    && [ -x "$SCRIPT_DIR/devtools-runner" ] \
    && [ -x "$SCRIPT_DIR/devtools-workerd" ]; then
    PREBUILT=true
fi

if [ "$PREBUILT" = false ]; then
    # --- Rust toolchain (user-local; install if missing) ------------------
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

    # --- Node.js toolchain (fnm + Node 26 + pnpm) -------------------------
    FNM_BIN="$(command -v fnm || true)"
    if [ -z "$FNM_BIN" ] && [ -x "$HOME/.local/share/fnm/fnm" ]; then
        FNM_BIN="$HOME/.local/share/fnm/fnm"
    fi
    if [ -z "$FNM_BIN" ]; then
        echo "error: fnm is required (Node 26 + pnpm 11)" >&2
        exit 1
    fi

    NODE_VERSION="$("$FNM_BIN" exec --using 26 node --version)"
    if [[ "$NODE_VERSION" != v26.* ]]; then
        echo "error: expected Node 26 from fnm, got $NODE_VERSION" >&2
        exit 1
    fi

    # --- Native WebView build dependencies --------------------------------
    if ! pkg-config --exists gtk+-3.0 webkit2gtk-4.1; then
        echo "error: missing GTK/WebKitGTK development packages" >&2
        echo "install on Debian 13: sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev" >&2
        exit 1
    fi

    # --- Build ------------------------------------------------------------
    echo "==> Checking and building Vue UI (Node $NODE_VERSION)..."
    "$FNM_BIN" exec --using 26 pnpm --dir "$WEB_DIR" install --frozen-lockfile
    "$FNM_BIN" exec --using 26 pnpm --dir "$WEB_DIR" check

    echo "==> Building Rust workspace (release)..."
    ( cd "$SCRIPT_DIR" && cargo build --release --workspace )
    RUNNER_BIN="$SCRIPT_DIR/target/release/devtools-runner"
    WORKER_BIN="$SCRIPT_DIR/target/release/devtools-workerd"
else
    echo "==> Installing prebuilt DevTools binaries..."
    RUNNER_BIN="$SCRIPT_DIR/devtools-runner"
    WORKER_BIN="$SCRIPT_DIR/devtools-workerd"
fi

if ! command -v tesseract >/dev/null 2>&1; then
    echo "warning: tesseract is not installed; OCR will be disabled" >&2
fi
if ! command -v zbarimg >/dev/null 2>&1; then
    echo "warning: zbarimg is not installed; barcode recognition will be disabled" >&2
fi

# --- Install the binary ---------------------------------------------------
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
install -m 0755 "$RUNNER_BIN" "$INSTALL_DIR/devtools-runner"
install -m 0755 "$WORKER_BIN" "$INSTALL_DIR/devtools-workerd"
echo "==> binaries -> $INSTALL_DIR/devtools-runner, $INSTALL_DIR/devtools-workerd"

# --- KRunner DBus-runner metadata -----------------------------------------
KRUNNER_DIR="$HOME/.local/share/krunner/dbusplugins"
mkdir -p "$KRUNNER_DIR"
install -m 0644 "$SCRIPT_DIR/assets/org.kde.devtools.desktop" "$KRUNNER_DIR/"
echo "==> krunner plugin -> $KRUNNER_DIR/org.kde.devtools.desktop"

# --- DevTools application icon -------------------------------------------
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
mkdir -p "$ICON_DIR"
install -m 0644 "$SCRIPT_DIR/assets/org.loveyu.DevTools.svg" "$ICON_DIR/"
echo "==> application icon -> $ICON_DIR/org.loveyu.DevTools.svg"

# --- Standalone application launcher (works without KRunner) --------------
APPLICATIONS_DIR="$HOME/.local/share/applications"
mkdir -p "$APPLICATIONS_DIR"
sed "s|@WORKER_EXEC@|$INSTALL_DIR/devtools-workerd|g" \
    "$SCRIPT_DIR/assets/org.loveyu.DevTools.desktop.in" \
    > "$APPLICATIONS_DIR/org.loveyu.DevTools.desktop"
chmod 0644 "$APPLICATIONS_DIR/org.loveyu.DevTools.desktop"
echo "==> application launcher -> $APPLICATIONS_DIR/org.loveyu.DevTools.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPLICATIONS_DIR" >/dev/null 2>&1 || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
fi

# --- D-Bus activation service (substitute the absolute binary path) -------
DBUS_DIR="$HOME/.local/share/dbus-1/services"
mkdir -p "$DBUS_DIR"
sed "s|@EXEC@|$INSTALL_DIR/devtools-runner|g" \
    "$SCRIPT_DIR/assets/org.kde.devtools.service" \
    > "$DBUS_DIR/org.kde.devtools.service"
echo "==> dbus service -> $DBUS_DIR/org.kde.devtools.service"
sed "s|@WORKER_EXEC@|$INSTALL_DIR/devtools-workerd|g" \
    "$SCRIPT_DIR/assets/org.loveyu.DevTools.service" \
    > "$DBUS_DIR/org.loveyu.DevTools.service"
echo "==> dbus service -> $DBUS_DIR/org.loveyu.DevTools.service"

# --- Make sure DBus-activated services can reach the Wayland/X display ----
# (Plasma imports these at login; this is an idempotent safety net.)
dbus-update-activation-environment \
    WAYLAND_DISPLAY DISPLAY XAUTHORITY XDG_SESSION_TYPE >/dev/null 2>&1 || true

# --- Stop any running instance so the fresh binary / activation is used ---
killall devtools-runner >/dev/null 2>&1 || true
killall devtools-workerd >/dev/null 2>&1 || true

# --- Restart KRunner so it picks up the new runner metadata ---------------
if command -v krunner >/dev/null 2>&1 && command -v kquitapp6 >/dev/null 2>&1; then
    kquitapp6 krunner >/dev/null 2>&1 || true
fi
if command -v krunner >/dev/null 2>&1; then
    sleep 1
    # Plasma respawns KRunner; start it explicitly as a fallback.
    if ! command -v qdbus6 >/dev/null 2>&1 \
        || ! qdbus6 org.kde.krunner /App >/dev/null 2>&1; then
        ( setsid krunner >/dev/null 2>&1 & ) || true
        sleep 1
    fi
fi

cat <<EOF

Installed. Open KRunner (Alt+Space) and try:
    date       (or  time / da / tim)
    json       (when the clipboard contains valid JSON)
    convert    (or cv, with text in the clipboard)
    ocr         (local image text recognition)
    barcode     (or bar / qr / qrcode)
    compress    (or squoosh / image-compress / imgcompress)
    editor      (or image-editor / edit-image / imageedit / imgedit)
Press Enter to copy the selected value.

Open Worker settings directly:
    $INSTALL_DIR/devtools-workerd --settings

Open the standalone launcher (also works without KRunner):
    $INSTALL_DIR/devtools-workerd --launcher

To open KRunner filtered to this runner from a terminal:
    qdbus6 org.kde.krunner /App org.kde.krunner.App.querySingleRunner org.kde.devtools date
EOF
