# greentic-redbutton

`greentic-redbutton` is a Rust CLI for listening to a matching HID button device and posting `redbutton.click` webhook events. The current implementation covers the CLI, config resolution, HID-based device enumeration/connection, reconnecting runtime, webhook delivery, embedded i18n, and CI/release automation.

The HID backend is the first implementation slice of `.codex/PR-01.md`. It is aimed at keyboard-style HID devices and defaults to Greentic's target VID/PID plus the `enter` key.

## Development

Run the local CI wrapper from the repo root:

```bash
bash ci/local_check.sh
```

The CLI currently exposes:

```bash
cargo run --
cargo run -- doctor
cargo run -- list-devices
cargo run -- once
cargo run -- version
cargo run -- i18n status
cargo run -- i18n validate
```

Supported config flags:

```bash
--vendor-id <u16>
--product-id <u16>
--key <string>
--webhook-url <url>
--timeout-ms <u64>
--verbose
```

Matching environment variables:

```bash
GREENTIC_REDBUTTON_VENDOR_ID
GREENTIC_REDBUTTON_PRODUCT_ID
GREENTIC_REDBUTTON_KEY
GREENTIC_REDBUTTON_WEBHOOK_URL
GREENTIC_REDBUTTON_TIMEOUT_MS
```

Default values:

```text
vendor_id   = 32904
product_id  = 21
key         = enter
webhook_url = http://127.0.0.1:8080/events/webhook
timeout_ms  = 5000
```

## i18n

The crate embeds locale JSON files from `i18n/` at build time. Runtime locale selection uses this precedence:

1. `--locale <tag>`
2. `LC_ALL`, `LC_MESSAGES`, `LANG`
3. OS locale via `sys-locale`
4. `en`

Manage locale files with:

```bash
tools/i18n.sh status
tools/i18n.sh validate
tools/i18n.sh all
```

`tools/i18n.sh` is prepared for the Greentic translator flow, uses all supported locales from `i18n/locales.json`, and caps translation batches at `200` translations per batch.

## CI and Releases

`ci/local_check.sh` is the single developer entrypoint. It runs formatting, clippy, tests, build, docs, i18n validation, and crates.io packaging checks for every publishable crate in the workspace.

Local validation:

```bash
bash ci/local_check.sh
```

Release flow:

1. Bump `version` in `Cargo.toml`.
2. Commit the change.
3. Create and push a matching git tag: `vX.Y.Z`.
4. GitHub Actions runs `.github/workflows/publish.yml`.

`publish.yml` verifies that the tag matches `Cargo.toml`, reruns `ci/local_check.sh`, performs crates.io dry-runs, publishes to crates.io, and builds `cargo-binstall` archives for these targets:

- Windows `x86_64`
- Windows `aarch64`
- Linux `x86_64`
- Linux `aarch64`
- macOS 15 `x86_64`
- macOS 15 `aarch64`

Required GitHub secret:

- `CARGO_REGISTRY_TOKEN`
