# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

`greentic-redbutton` is a Rust CLI that listens to a USB HID red-button device (by VID/PID) and posts `redbutton.click` webhook events. It auto-reconnects on unplug/replug, embeds 66-locale i18n at build time, and ships cross-platform binaries via `cargo binstall`.

Crate version 1.2.0-dev.0, edition 2024, Rust 1.95.0 (pinned via `rust-toolchain.toml`).

## Build & Development Commands

```bash
# Full local CI (fmt + clippy + test + build + doc + i18n validate + package)
bash ci/local_check.sh

# Package checks only (i18n validate + crate packaging dry-run)
bash ci/local_check.sh package

# Individual steps
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --all-features
cargo doc --no-deps --all-features

# Run a single test
cargo test <test_name> --all-features

# i18n maintenance
tools/i18n.sh validate
tools/i18n.sh status
```

**Linux build prerequisite:** `sudo apt-get install pkg-config libudev-dev`

## Architecture

### Module Dependency Flow

```
main.rs → cli.rs (clap parsing) → config.rs (flag/env/default resolution)
       ↓
  runtime.rs (reconnecting listener, once, wait_for_press)
       ↓                        ↓
  device/mod.rs              webhook.rs (blocking HTTP POST via reqwest)
  (DeviceBackend/DeviceStream traits + GenericHidBackend using hidapi)
       ↓
  device/{linux,macos,windows}.rs (thin wrappers delegating to GenericHidBackend)
       ↓
  suppress.rs (platform-specific input suppression: Linux EVIOCGRAB, macOS CGEventTap, Windows WH_KEYBOARD_LL)

  event.rs        — core domain types: DeviceMatcher (VID/PID match), ButtonEvent { kind, timestamp }
  doctor.rs       — diagnostic subcommand: device probe + webhook dry-run (imports config, device, runtime)
  constants.rs    — built-in defaults (VID/PID, debounce, reconnect delay)
  i18n.rs         — runtime locale resolution using build-time EMBEDDED_LOCALES
  device/mock.rs  — MockBackend implementing DeviceBackend for tests (no physical hardware needed)
  integration_tests.rs — end-to-end tests with a local TCP webhook target
```

### Key Design Decisions

- **Synchronous/threaded, not async.** No tokio runtime. HID reads are blocking; `runtime.rs` spawns threads for webhook delivery and press detection timeouts via `std::sync::mpsc`.
- **Input suppression.** The `suppress.rs` module grabs the device (Linux `EVIOCGRAB`) or installs an OS-level event tap/hook (macOS `CGEventTapCreate`, Windows `SetWindowsHookExW`) to suppress the physical Enter key from reaching other apps during a short window after each button press.
- **Build-time i18n.** `build.rs` reads `i18n/locales.json` → embeds all `i18n/*.json` files as `EMBEDDED_LOCALES` in the binary → consumed by `src/i18n.rs` at runtime. Locale selection: `--locale` flag > `LC_ALL`/`LC_MESSAGES`/`LANG` > OS locale > `en`.
- **HID backend.** All platforms use the same `GenericHidBackend` (via `hidapi` crate). Platform files (`linux.rs`, `macos.rs`, `windows.rs`) are thin wrappers with platform-specific backend names. The `device/mod.rs` module handles HID report parsing for keyboard-style reports with a debounce mechanism (`PRESS_DEBOUNCE_MS = 120ms`).
- **Config precedence:** CLI flags > environment variables (`GREENTIC_REDBUTTON_*`) > built-in defaults in `constants.rs`.

### Core Traits

- `DeviceBackend` (`device/mod.rs`): `list_devices()` and `connect(matcher)` — returns a `Box<dyn DeviceStream>`
- `DeviceStream` (`device/mod.rs`): `next_event()` — blocking read that returns `ButtonEvent { kind: Down|Up, timestamp }`
- `InputSuppressor` (`suppress.rs`): `notify_button_press()` — signals the suppression window

## CI/Release

CI runs on push to main and PRs: lint → test → package-dry-run → publish-crates → binstall-build (6 targets) → create-release. The `ci/publishable_crates.py` script determines crate publish order; `ci/release_version.py` extracts the version from `Cargo.toml`. The repo also participates in the nightly dev-publish pipeline (`dev-publish.yml`).

Release flow: bump version in `Cargo.toml` → commit → push tag `vX.Y.Z` → CI publishes to crates.io and builds archives for `cargo-binstall`.

## Repo Maintenance Rules

Per `.codex/global_rules.md`, every PR must:
1. Refresh `.codex/repo_overview.md` before and after changes
2. Run `ci/local_check.sh` at the end and ensure it passes
3. Prefer existing Greentic shared crates over re-inventing types locally

## Git Commit Rules

Do NOT add `Co-Authored-By: Claude` or AI attribution in commits or PRs. Use conventional commit format (`feat:`, `fix:`, `docs:`, `chore:`). Always create feature branches — never commit directly to main.

## Branches

Default branch is **main**. A `develop` branch exists for the nightly dev-publish pipeline.

## Testing Without Hardware

All tests run without a physical button. `device/mock.rs` provides a `MockBackend` implementing `DeviceBackend` that emits synthetic press/release events. `integration_tests.rs` spins up a local TCP server as a webhook target and drives the full runtime loop end-to-end.

## Demo Bundle

`demo/` contains a self-contained demo environment: a WASM handler component (`component-redbutton-handler/`), a dashboard SPA (`dashboard/`), a bundle descriptor (`bundle.yaml`), GTCwizard answers (`gtc_wizard_answers.json`), an app pack (`apps/redbutton-app/` with flows, components, and `pack.yaml`), and a demo environment config (`greentic.demo.yaml`). `setup.sh` provisions the demo via `gtc`; `watch.sh` monitors button events. `README.md` documents the demo prerequisites and usage.
