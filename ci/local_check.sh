#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-all}"
ALLOW_DIRTY_FLAG=()
if [[ -z "${CI:-}" ]]; then
  ALLOW_DIRTY_FLAG=(--allow-dirty)
fi

step() {
  printf '\n==> %s\n' "$1"
}

publishable_crates() {
  python3 ci/publishable_crates.py
}

verify_packaged_contents() {
  local crate="$1"
  local listing
  listing="$(cargo package --list -p "$crate" "${ALLOW_DIRTY_FLAG[@]}")"

  local required=(
    "Cargo.toml"
    "README.md"
    "LICENSE"
    "build.rs"
    "src/main.rs"
    "src/cli.rs"
    "src/i18n.rs"
    "i18n/en.json"
    "i18n/locales.json"
    "tools/i18n.sh"
  )

  local item
  for item in "${required[@]}"; do
    if [[ "$listing" != *"$item"* ]]; then
      printf 'missing packaged asset for crate %s: %s\n' "$crate" "$item" >&2
      return 1
    fi
  done
}

run_package_checks() {
  local crate
  for crate in $(publishable_crates); do
    step "Packaging checks for $crate"
    cargo package --no-verify -p "$crate" "${ALLOW_DIRTY_FLAG[@]}"
    cargo package -p "$crate" "${ALLOW_DIRTY_FLAG[@]}"
    verify_packaged_contents "$crate"
    cargo publish -p "$crate" --dry-run "${ALLOW_DIRTY_FLAG[@]}"
  done
}

case "$MODE" in
  package)
    step "i18n validation"
    tools/i18n.sh validate
    run_package_checks
    ;;
  all)
    step "cargo fmt"
    cargo fmt --all -- --check

    step "cargo clippy"
    cargo clippy --all-targets --all-features -- -D warnings

    step "cargo test"
    cargo test --all-features

    step "cargo build"
    cargo build --all-features

    step "cargo doc"
    cargo doc --no-deps --all-features

    step "i18n validation"
    tools/i18n.sh validate

    run_package_checks
    ;;
  *)
    printf 'usage: %s [all|package]\n' "$0" >&2
    exit 2
    ;;
esac
