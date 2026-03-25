#!/usr/bin/env bash
set -euo pipefail

# Bootstrap the redbutton demo bundle.
# Usage: bash demo/setup.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEMO_DIR="$SCRIPT_DIR"
WORKSPACE="$(cd "$SCRIPT_DIR/../.." && pwd)"
GREENTIC_START="$WORKSPACE/greentic-start/target/release/greentic-start"

step() { printf '\n==> %s\n' "$1"; }

resolve_provider() {
  local type="$1"
  local name="$2"
  local dest_dir="$DEMO_DIR/providers/$type"
  local dest="$dest_dir/$name.gtpack"

  if [[ -f "$dest" ]]; then
    echo "  [skip] $name.gtpack already exists"
    return 0
  fi

  mkdir -p "$dest_dir"

  local candidates=(
    "$WORKSPACE/demo-bundle/providers/$type/$name.gtpack"
    "$WORKSPACE/all-message-demo/providers/$type/$name.gtpack"
    "$WORKSPACE/greentic-events-providers/dist/$name.gtpack"
    "$WORKSPACE/greentic-events-providers/packs/$name/dist/$name.gtpack"
    "$WORKSPACE/packs/$name/dist/$name.gtpack"
  )

  if [[ "$type" != "events" ]]; then
    candidates+=(
      "$WORKSPACE/demo-bundle/providers/messaging/$name.gtpack"
      "$WORKSPACE/all-message-demo/providers/messaging/$name.gtpack"
    )
  fi

  for candidate in "${candidates[@]}"; do
    if [[ -f "$candidate" ]]; then
      cp "$candidate" "$dest"
      echo "  [ok]   $name.gtpack"
      return 0
    fi
  done

  echo "  [WARN] $name.gtpack not found in workspace"
  return 1
}

step "Clearing stale runtime state"
rm -rf "$DEMO_DIR/state" "$DEMO_DIR/logs"
echo "  done"

step "Resolving provider gtpack files from workspace"
MISSING=0
resolve_provider events  events-webhook        || MISSING=$((MISSING + 1))
resolve_provider messaging messaging-webchat-gui || MISSING=$((MISSING + 1))
resolve_provider state   state-memory           || MISSING=$((MISSING + 1))

if [[ "$MISSING" -gt 0 ]]; then
  echo ""
  echo "WARNING: $MISSING provider(s) not found."
  exit 1
fi

step "Building app pack"
if command -v greentic-pack &>/dev/null; then
  greentic-pack build --in "$DEMO_DIR/apps/redbutton-app" --allow-pack-schema 2>&1 | tail -1
  mkdir -p "$DEMO_DIR/packs"
  cp "$DEMO_DIR/apps/redbutton-app/dist/redbutton-app.gtpack" "$DEMO_DIR/packs/default.gtpack"
  echo "  packs/default.gtpack ready"
else
  echo "  [skip] greentic-pack not found, using existing packs/default.gtpack"
fi

step "Setup webchat secrets (if not already done)"
if [[ ! -f "$DEMO_DIR/.greentic/dev/.dev.secrets.env" ]]; then
  echo "  Run: gtc setup demo/"
  echo "  Then re-run this script."
else
  echo "  secrets already configured"
fi

step "Demo ready"
cat << 'USAGE'

Start server (Terminal 1):
  GREENTIC_PROVIDER_CORE_ONLY=false GREENTIC_ENV=dev \
    greentic-start start --bundle demo/ --cloudflared off

Live dashboard (Terminal 2):
  bash demo/watch.sh

Send event (Terminal 3):
  curl -X POST http://127.0.0.1:8080/v1/events/ingress/webhook/demo/default/redbutton \
    -H "Content-Type: application/json" \
    -d '{"source":"greentic-redbutton","event_type":"redbutton.click","vendor_id":32904,"product_id":21,"key":"enter","timestamp":"2026-03-25T08:00:00Z","device_name":"LinTx Keyboard","os":"macos","arch":"aarch64"}'

Webchat (browser):
  http://127.0.0.1:8080/v1/web/webchat/demo/

USAGE
