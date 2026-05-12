#!/usr/bin/env bash
# Clones (or updates) Noobidoo/stoat-for-web from GitHub and builds the client.
# Run from the repository root or via tauri.conf.json beforeBuildCommand.
# Set STOAT_WEB_SKIP_FETCH=1 to skip (e.g. when CI already ran this step).
set -e

if [ "${STOAT_WEB_SKIP_FETCH:-0}" = "1" ]; then
    echo "[fetch-web] Skipping (STOAT_WEB_SKIP_FETCH=1)"
    exit 0
fi

# ── bootstrap pnpm (local / WSL only — CI gets pnpm from mise) ───────────────
if ! command -v pnpm &>/dev/null; then
    # Activate nvm if available
    export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
    # shellcheck source=/dev/null
    [ -s "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh" --no-use

    # Put the latest nvm-managed node on PATH
    if ! command -v node &>/dev/null && [ -d "$NVM_DIR/versions/node" ]; then
        NODE_VERSION=$(ls -1 "$NVM_DIR/versions/node" | sort -V | tail -1)
        export PATH="$NVM_DIR/versions/node/$NODE_VERSION/bin:$PATH"
    fi

    # Remove stale corepack shims (Node ≤ 22.12 key-rotation bug)
    for BIN in pnpm pnpx; do
        BIN_PATH=$(command -v "$BIN" 2>/dev/null || true)
        if [ -n "$BIN_PATH" ] && grep -q "corepack" "$BIN_PATH" 2>/dev/null; then
            echo "[fetch-web] Removing stale corepack shim: $BIN_PATH"
            rm -f "$BIN_PATH"
        fi
    done

    if command -v npm &>/dev/null; then
        echo "[fetch-web] Installing pnpm via npm..."
        npm install -g pnpm --quiet
    else
        echo "[fetch-web] ERROR: pnpm not found and npm unavailable. Install Node.js." >&2
        exit 1
    fi
fi
# ─────────────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WEB_DIR="$SCRIPT_DIR/../stoat-for-web"

if [ ! -d "$WEB_DIR/.git" ]; then
    echo "[fetch-web] Cloning stoat-for-web..."
    git clone --recursive https://github.com/Noobidoo/stoat-for-web.git "$WEB_DIR"
else
    echo "[fetch-web] Updating stoat-for-web..."
    git -C "$WEB_DIR" fetch --prune
    git -C "$WEB_DIR" reset --hard origin/main
    git -C "$WEB_DIR" submodule update --init
fi

cd "$WEB_DIR"

echo "[fetch-web] Installing dependencies..."
pnpm install --frozen-lockfile

echo "[fetch-web] Building dependencies..."
pnpm --filter stoat.js build
pnpm --filter solid-livekit-components build
pnpm --filter "@lingui-solid/babel-plugin-lingui-macro" build
pnpm --filter "@lingui-solid/babel-plugin-extract-messages" build

echo "[fetch-web] Compiling i18n catalogs..."
pnpm --filter client exec lingui compile --typescript

echo "[fetch-web] Copying assets (optional)..."
pnpm --filter client exec node scripts/copyAssets.mjs || echo "[fetch-web] Assets step skipped (no submodule)"

echo "[fetch-web] Building web client..."
pnpm --filter client exec vite build

echo "[fetch-web] Done."
