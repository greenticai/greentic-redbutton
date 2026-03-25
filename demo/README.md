# Red Button Demo

End-to-end demo showing the greentic-redbutton webhook event flowing through the Greentic platform: HTTP ingress, events-webhook provider, WASM component execution, and flow output.

## Prerequisites

- Patched `greentic-start` binary (built from `greentic-start` repo with events dispatch fixes)
- `greentic-pack` CLI (for rebuilding the app pack if you modify flows/components)
- Provider gtpack files (resolved automatically by `setup.sh` from the workspace)

## Quick Start

### 1. Setup (one-time)

```bash
cd greentic-redbutton
bash demo/setup.sh
```

This copies the required provider `.gtpack` files from the workspace and clears stale state.

### 2. Build the app pack (one-time, or after modifying flows/components)

```bash
greentic-pack build --in demo/apps/redbutton-app --allow-pack-schema
mkdir -p demo/packs
cp demo/apps/redbutton-app/dist/redbutton-app.gtpack demo/packs/default.gtpack
```

### 3. Start the server (Terminal 1)

```bash
rm -rf demo/{state,.greentic,logs}
/path/to/greentic-start/target/release/greentic-start start \
  --bundle demo/ --cloudflared off
```

Replace `/path/to/greentic-start` with the actual path to the `greentic-start` repo in your workspace. You should see:

```
HTTP ingress ready at http://127.0.0.1:8080
events: handled in-process (HTTP ingress + timer scheduler)
demo start running ...
```

> **Important:** Use the patched `greentic-start` binary, NOT `gtc start`. The installed `gtc` does not have the events dispatch fixes.

### 4. Start the live dashboard (Terminal 2)

```bash
bash demo/watch.sh
```

This watches `demo/state/runs/` for new flow execution results and displays them in real-time.

### 5. Send events (Terminal 3)

**Option A — curl (no hardware needed):**

```bash
curl -X POST http://127.0.0.1:8080/v1/events/ingress/webhook/demo/default/redbutton \
  -H "Content-Type: application/json" \
  -d '{
    "source": "greentic-redbutton",
    "event_type": "redbutton.click",
    "vendor_id": 32904,
    "product_id": 21,
    "key": "enter",
    "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "device_name": "LinTx Keyboard",
    "os": "macos",
    "arch": "aarch64"
  }'
```

**Option B — greentic-redbutton CLI (requires USB red button device):**

```bash
cargo run -- \
  --webhook-url http://127.0.0.1:8080/v1/events/ingress/webhook/demo/default/redbutton \
  --no-suppress
```

### 6. Verify

Terminal 2 (watch.sh) shows the flow execution result in real-time:

```
[#1] ✓ Success  run=1774360800
  │ Red Button pressed!
  │ Source: greentic-redbutton
  │ Event: redbutton.click
  │ Device: LinTx Keyboard
  │ Key: enter
  │ VID: 32904
  │ PID: 21
  │ OS: macos/aarch64
  │ Timestamp: 2026-03-25T08:00:00Z
  └─ 15:00:01
```

You can also inspect the raw run artifacts:

```bash
# Flow execution summary
cat demo/state/runs/events/redbutton-app/default/*/summary.txt

# Component transcript (JSON lines)
cat demo/state/runs/events/redbutton-app/default/*/transcript.jsonl

# Flow input (event envelope with payload)
cat demo/state/runs/events/redbutton-app/default/*/input.json
```

## Architecture

```
                           Terminal 3
                               │
                         curl / redbutton CLI
                               │
                          POST JSON body
                               │
                               ▼
Terminal 1              ┌──────────────┐
greentic-start ────────►│  HTTP :8080   │
                        │   Ingress     │
                        └──────┬───────┘
                               │
                    ┌──────────▼──────────┐
                    │  events-webhook     │
                    │  WASM provider      │
                    │  (ingest_http)      │
                    └──────────┬──────────┘
                               │
                        EventEnvelope
                     (with curl body as
                         payload)
                               │
                    ┌──────────▼──────────┐
                    │  event_router       │
                    │  → redbutton-app    │
                    │  → flow "default"   │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │  redbutton-handler   │
                    │  WASM component     │
                    │  (render operation) │
                    └──────────┬──────────┘
                               │
                         Flow output
                     (state/runs/...)
                               │
Terminal 2                     ▼
watch.sh ──────────► reads transcript.jsonl
                     displays result
```

## Troubleshooting

**"domain disabled" on curl:**
You are using `gtc start` instead of the patched `greentic-start` binary. Kill the old process and start with the correct binary.

**curl hangs (no response):**
Stale state. Stop the server, run `rm -rf demo/{state,.greentic,logs}`, restart.

**"device not found" from CLI:**
The USB red button (VID 0x8088, PID 0x0015) is not connected. Use curl instead.

**watch.sh shows nothing:**
Events haven't been sent yet, or the runs directory doesn't exist. Send a curl request first.
