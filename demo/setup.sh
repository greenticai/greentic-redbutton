#!/usr/bin/env bash
set -euo pipefail

# Bootstrap the redbutton demo bundle by copying provider gtpack files
# from the workspace. Run this from the greentic-redbutton repo root:
#
#   bash demo/setup.sh
#
# Then start the demo:
#
#   gtc start demo/ --cloudflared off
#
# In another terminal, send a button press:
#
#   cargo run -- --webhook-url http://127.0.0.1:8080/v1/events/ingress/webhook/demo/default/redbutton --no-suppress once

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEMO_DIR="$SCRIPT_DIR"
WORKSPACE="$(cd "$SCRIPT_DIR/../.." && pwd)"

step() { printf '\n==> %s\n' "$1"; }

# Resolve provider gtpack files from workspace
resolve_provider() {
  local type="$1"      # events, messaging, state
  local name="$2"      # events-webhook, messaging-webchat-gui, state-memory
  local dest_dir="$DEMO_DIR/providers/$type"
  local dest="$dest_dir/$name.gtpack"

  if [[ -f "$dest" ]]; then
    echo "  [skip] $name.gtpack already exists"
    return 0
  fi

  mkdir -p "$dest_dir"

  # Try common workspace locations
  local candidates=(
    "$WORKSPACE/demo-bundle/providers/$type/$name.gtpack"
    "$WORKSPACE/all-message-demo/providers/$type/$name.gtpack"
    "$WORKSPACE/greentic-events-providers/dist/$name.gtpack"
    "$WORKSPACE/greentic-events-providers/packs/$name/dist/$name.gtpack"
    "$WORKSPACE/packs/$name/dist/$name.gtpack"
  )

  # For messaging/state providers, also check under messaging/
  if [[ "$type" != "events" ]]; then
    candidates+=(
      "$WORKSPACE/demo-bundle/providers/messaging/$name.gtpack"
      "$WORKSPACE/all-message-demo/providers/messaging/$name.gtpack"
    )
  fi

  for candidate in "${candidates[@]}"; do
    if [[ -f "$candidate" ]]; then
      cp "$candidate" "$dest"
      echo "  [ok]   $name.gtpack <- $(basename "$(dirname "$(dirname "$candidate")")")/..."
      return 0
    fi
  done

  echo "  [WARN] $name.gtpack not found in workspace"
  return 1
}

step "Clearing stale runtime state"
rm -rf "$DEMO_DIR/state" "$DEMO_DIR/.greentic"
echo "  done"

step "Resolving provider gtpack files from workspace"
MISSING=0
resolve_provider events  events-webhook        || MISSING=$((MISSING + 1))
resolve_provider messaging messaging-webchat-gui || MISSING=$((MISSING + 1))
resolve_provider state   state-memory           || MISSING=$((MISSING + 1))

if [[ "$MISSING" -gt 0 ]]; then
  echo ""
  echo "WARNING: $MISSING provider(s) not found."
  echo "Build them first or use 'gtc wizard --answers demo/gtc_wizard_answers.json' to pull from OCI."
  exit 1
fi

step "Demo bundle ready"
echo ""
echo "Start the runtime:"
echo "  gtc start demo/ --cloudflared off"
echo ""
echo "Then send a red button event (in another terminal):"
echo "  cargo run -- --webhook-url http://127.0.0.1:8080/v1/events/ingress/webhook/demo/default/redbutton --no-suppress once"
echo ""
echo "Or with curl:"
echo '  curl -X POST http://127.0.0.1:8080/v1/events/ingress/webhook/demo/default/redbutton \'
echo '    -H "Content-Type: application/json" \'
echo '    -d '"'"'{"source":"greentic-redbutton","event_type":"redbutton.click","vendor_id":32904,"product_id":21,"key":"enter","timestamp":"2026-03-24T12:00:00Z"}'"'"
