# Repository Overview

## 1. High-Level Purpose
`greentic-redbutton` is a Rust CLI for listening to a configured HID button device and posting `redbutton.click` webhook events to a Greentic-compatible HTTP endpoint. The current implementation covers command parsing, config resolution from flags and environment variables, HID device enumeration/connection, reconnecting runtime behavior, webhook delivery, embedded i18n assets, and CI/release automation.

The repository is no longer just a scaffold: `list-devices`, `doctor`, `once`, and the default long-running listener are implemented. The remaining gap to the PR brief is depth rather than absence: the device backend is a shared HID implementation reused across platforms, and translation coverage is still mostly English-seeded.

## 2. Main Components and Functionality
- **Path:** `Cargo.toml`
- **Role:** Root manifest for the single publishable Rust binary crate.
- **Key functionality:** Declares the CLI package metadata, release packaging includes, and dependencies for HID access, webhook delivery, locale handling, and CLI parsing.
- **Key dependencies / integration points:** Provides the version used by the release workflow and the package metadata required for crates.io publication.

- **Path:** `build.rs`
- **Role:** Build-time i18n bundler.
- **Key functionality:** Reads `i18n/locales.json` and embeds all locale JSON files into the compiled binary.
- **Key dependencies / integration points:** Feeds the generated locale bundle consumed by `src/i18n.rs`.

- **Path:** `src/cli.rs`
- **Role:** CLI surface definition.
- **Key functionality:** Defines the default listener mode plus `doctor`, `list-devices`, `once`, `version`, and hidden i18n maintenance commands, along with global config flags.
- **Key dependencies / integration points:** Parsed by `src/main.rs` and resolved into `Config` by `src/config.rs`.

- **Path:** `src/config.rs`
- **Role:** Effective configuration resolver.
- **Key functionality:** Applies precedence of CLI flags over environment variables over built-in defaults, validates timeout and webhook URL values, and builds a `DeviceMatcher`.
- **Key dependencies / integration points:** Used by all runtime commands.

- **Path:** `src/constants.rs`
- **Role:** Default values and runtime constants.
- **Key functionality:** Stores the built-in VID/PID/key, default webhook URL, timeout, reconnect delay, and event naming constants.
- **Key dependencies / integration points:** Shared across config, runtime, and webhook code.

- **Path:** `src/event.rs`
- **Role:** Shared runtime models.
- **Key functionality:** Defines `DeviceMatcher`, `ButtonKey`, `DeviceInfo`, `ButtonEvent`, and the outbound JSON webhook payload.
- **Key dependencies / integration points:** Used by config parsing, device backends, runtime, and webhook posting.

- **Path:** `src/device/`
- **Role:** Device backend layer.
- **Key functionality:** Exposes the `DeviceBackend` and `DeviceStream` traits plus a generic HID implementation that enumerates HID devices, opens the matching VID/PID device, and converts keyboard-style HID reports into button up/down events. `linux.rs`, `macos.rs`, and `windows.rs` currently wrap the same generic HID backend with platform labels.
- **Key dependencies / integration points:** Selected by `default_backend()` and consumed by `runtime.rs` and `doctor.rs`.

- **Path:** `src/runtime.rs`
- **Role:** Listener and one-shot runtime logic.
- **Key functionality:** Runs the reconnecting listener loop, waits for one matching button-down event in `once`, performs the doctor press test with a timeout, and sends webhook payloads on key-down only.
- **Key dependencies / integration points:** Calls into the device backend and `src/webhook.rs`.

- **Path:** `src/webhook.rs`
- **Role:** HTTP delivery layer.
- **Key functionality:** Sends JSON POST requests to the configured webhook with request timeout handling and non-success HTTP rejection.
- **Key dependencies / integration points:** Used by `src/runtime.rs`.

- **Path:** `src/doctor.rs`
- **Role:** Diagnostics command implementation.
- **Key functionality:** Reports effective config values, lists matching devices, and performs a simple button-press timeout check.
- **Key dependencies / integration points:** Used by the `doctor` subcommand in `src/main.rs`.

- **Path:** `src/i18n.rs` and `i18n/`
- **Role:** Embedded localization system.
- **Key functionality:** Selects the runtime locale, renders key-based messages, validates locale files against `en.json`, and keeps the 66-language locale set embedded in the binary.
- **Key dependencies / integration points:** Used by command output and `tools/i18n.sh`.

- **Path:** `src/main.rs`
- **Role:** Top-level program entrypoint.
- **Key functionality:** Parses CLI arguments, dispatches commands, integrates i18n, and launches the listener/runtime flows.
- **Key dependencies / integration points:** Orchestrates all application modules.

- **Path:** `tools/i18n.sh`
- **Role:** Repo-local i18n maintenance entrypoint.
- **Key functionality:** Seeds locale files, runs validation/status via the crate itself, and supports batched external translation generation capped at 200 translations per batch.
- **Key dependencies / integration points:** Reads the in-repo locale list and optionally uses the external Greentic translator repo.

- **Path:** `ci/local_check.sh`, `ci/publishable_crates.py`, `ci/release_version.py`
- **Role:** Local and release verification helpers.
- **Key functionality:** Run formatting/lint/test/build/doc/i18n/package checks, determine publish order, and extract the authoritative crate version.
- **Key dependencies / integration points:** Used by both GitHub workflows and by local development.

- **Path:** `.github/workflows/ci.yml` and `.github/workflows/publish.yml`
- **Role:** CI and release automation.
- **Key functionality:** Run PR/push CI, perform package dry-runs, enforce `v<version>` tag matching, publish to crates.io, and build six `cargo-binstall` release artifacts.
- **Key dependencies / integration points:** Depend on `ci/local_check.sh`, `ci/publishable_crates.py`, and `CARGO_REGISTRY_TOKEN`.

## 3. Work In Progress, TODOs, and Stubs
- **Location:** `src/device/mod.rs` with `src/device/linux.rs`, `src/device/macos.rs`, `src/device/windows.rs`
- **Status:** partial
- **Short description:** The platform files exist, but all three currently delegate to the same generic HID backend rather than distinct Linux/macOS/Windows implementations.

- **Location:** `src/device/mod.rs:149`
- **Status:** partial
- **Short description:** HID event parsing currently assumes keyboard-style reports and a key usage code. This fits the default `enter` use case but may not cover every red-button report format.

- **Location:** `i18n/*.json` and `tools/i18n.sh:48`
- **Status:** partial
- **Short description:** The i18n system is wired in, but the expanded command set is still English-seeded across most locales until the external translator workflow is run.

- **Location:** repository-wide marker scan
- **Status:** none found
- **Short description:** No explicit `TODO`, `FIXME`, `XXX`, `HACK`, `todo!`, or `unimplemented!` markers were found during this refresh.

## 4. Broken, Failing, or Conflicting Areas
- **Location:** PR architecture vs current backend depth
- **Evidence:** `.codex/PR-01.md` calls for dedicated Linux/macOS/Windows implementations, while the current code uses one shared HID backend behind thin platform wrappers.
- **Likely cause / nature of issue:** The first implementation slice prioritized a working cross-platform HID path over platform-specific backends; this is a design simplification rather than a compile/runtime failure.

- **Location:** translation coverage
- **Evidence:** `i18n/locales.json` contains 66 locales, but the expanded runtime strings were reseeded from `en.json` for all locales during this implementation pass.
- **Likely cause / nature of issue:** Locale infrastructure is complete, but real translations still depend on the external Greentic translator tooling and credentials.

## 5. Notes for Future Work
- Split the shared HID backend into genuinely platform-specific implementations if the generic report path proves insufficient on any target OS.
- Broaden HID report parsing beyond keyboard-style usage arrays if the physical red button emits a different report format.
- Add integration tests around webhook delivery and runtime reconnect behavior.
- Run the external Greentic translation workflow so the expanded command surface is translated instead of English-seeded.
