# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

`greentic-redbutton` is a Rust CLI that listens to a USB HID red-button device (by VID/PID) and posts `redbutton.click` webhook events. It auto-reconnects on unplug/replug, embeds 66-locale i18n at build time, and ships cross-platform binaries via `cargo binstall`.

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

CI runs on push to master and PRs: lint → test → package-dry-run → publish-crates → binstall-build (6 targets) → create-release. The `ci/publishable_crates.py` script determines crate publish order; `ci/release_version.py` extracts the version from `Cargo.toml`.

Release flow: bump version in `Cargo.toml` → commit → push tag `vX.Y.Z` → CI publishes to crates.io and builds archives for `cargo-binstall`.

## Repo Maintenance Rules

Per `.codex/global_rules.md`, every PR must:
1. Refresh `.codex/repo_overview.md` before and after changes
2. Run `ci/local_check.sh` at the end and ensure it passes
3. Prefer existing Greentic shared crates over re-inventing types locally

## Git Commit Rules

Do NOT add `Co-Authored-By: Claude` or AI attribution in commits or PRs. Use conventional commit format (`feat:`, `fix:`, `docs:`, `chore:`). Always create feature branches — never commit directly to master.
