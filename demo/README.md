# Red Button Demo

End-to-end demo: HTTP webhook event flows through the Greentic platform (events-webhook provider, WASM component execution, flow output) with webchat GUI.

## Quick Start

### 1. Setup (one-time)

```bash
cd greentic-redbutton
bash demo/setup.sh
gtc setup demo/          # configure webchat secrets
```

### 2. Start server (Terminal 1)

```bash
GREENTIC_PROVIDER_CORE_ONLY=false GREENTIC_ENV=dev \
  /path/to/greentic-start/target/release/greentic-start start \
  --bundle demo/ --cloudflared off
```

> **Required env vars:**
> - `GREENTIC_PROVIDER_CORE_ONLY=false` — allow WASM secret access
> - `GREENTIC_ENV=dev` — match secret URIs from `gtc setup`

### 3. Live dashboard (Terminal 2)

```bash
bash demo/watch.sh
```

### 4. Send events (Terminal 3)

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

### 5. Webchat (browser)

http://127.0.0.1:8080/v1/web/webchat/demo/

## Architecture

```
Terminal 3                    Terminal 1                    Terminal 2
curl / redbutton CLI          greentic-start                watch.sh
       |                           |                           |
  POST JSON ---------->  HTTP :8080 ingress                    |
                               |                               |
                    events-webhook provider                     |
                       (ingest_http WASM)                       |
                               |                               |
                        EventEnvelope                          |
                    (payload = curl body)                       |
                               |                               |
                        event_router                           |
                    -> redbutton-app pack                       |
                    -> flow "default"                           |
                               |                               |
                    redbutton-handler WASM                      |
                      (render operation)                       |
                               |                               |
                      state/runs/...  ------------------->  reads transcript
                                                           displays output

Browser
  http://127.0.0.1:8080/v1/web/webchat/demo/
       |
  Direct Line API --> messaging-webchat-gui provider
       |                       |
  on_message flow <------------+
       |
  msg2events component
       |
  response ----------> webchat GUI
```

## Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| `domain disabled` | Using `gtc start` instead of patched binary | Use patched `greentic-start` with env vars |
| `secret_error: denied` | Missing env var | Add `GREENTIC_PROVIDER_CORE_ONLY=false` |
| `secret_error: not-found` | Missing env var | Add `GREENTIC_ENV=dev` |
| curl hangs | Stale state | `rm -rf demo/{state,logs}` and restart |
| `device not found` | No USB red button | Use curl instead |
| watch.sh empty | No events yet | Send curl request first |
