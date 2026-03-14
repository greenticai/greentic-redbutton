#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:-all}"
AUTH_MODE="${AUTH_MODE:-auto}"
LOCALE="${LOCALE:-en}"
EN_PATH="${EN_PATH:-i18n/en.json}"
LOCALES_PATH="${LOCALES_PATH:-i18n/locales.json}"
I18N_TRANSLATOR_MANIFEST="${I18N_TRANSLATOR_MANIFEST:-../greentic-i18n/Cargo.toml}"
BATCH_SIZE="${BATCH_SIZE:-200}"

usage() {
  cat <<'USAGE'
Usage: tools/i18n.sh [seed|translate|validate|status|all]

Environment overrides:
  AUTH_MODE=...                   Translator auth mode for translate (default: auto)
  LOCALE=...                      CLI locale used for local validation output (default: en)
  EN_PATH=...                     English source file path (default: i18n/en.json)
  LOCALES_PATH=...                Supported locale list path (default: i18n/locales.json)
  I18N_TRANSLATOR_MANIFEST=...    Path to greentic-i18n Cargo.toml
  BATCH_SIZE=200                  Translation batch size ceiling
USAGE
}

locale_list() {
  python3 - <<'PY'
import json
from pathlib import Path
print("\n".join(json.loads(Path("i18n/locales.json").read_text())))
PY
}

seed_missing_locales() {
  local lang
  while IFS= read -r lang; do
    [[ -n "$lang" ]] || continue
    local path="i18n/${lang}.json"
    if [[ ! -f "$path" ]]; then
      cp "$EN_PATH" "$path"
    fi
  done < <(locale_list)
}

translate_batches() {
  if [[ ! -f "$I18N_TRANSLATOR_MANIFEST" ]]; then
    printf 'translator manifest not found at %s; seeding locale files only\n' "$I18N_TRANSLATOR_MANIFEST" >&2
    return 0
  fi

  local keys_per_locale
  keys_per_locale="$(python3 - <<'PY'
import json
from pathlib import Path
print(len(json.loads(Path("i18n/en.json").read_text())))
PY
)"
  if [[ "$keys_per_locale" -lt 1 ]]; then
    printf 'no English keys found in %s\n' "$EN_PATH" >&2
    return 1
  fi

  local langs=()
  local lang
  while IFS= read -r lang; do
    [[ "$lang" != "en" ]] && langs+=("$lang")
  done < <(locale_list)

  local batch_lang_cap=$(( BATCH_SIZE / keys_per_locale ))
  if [[ "$batch_lang_cap" -lt 1 ]]; then
    batch_lang_cap=1
  fi

  local total="${#langs[@]}"
  local index=0
  while [[ "$index" -lt "$total" ]]; do
    local slice=("${langs[@]:index:batch_lang_cap}")
    local joined
    joined="$(IFS=,; printf '%s' "${slice[*]}")"
    cargo run --manifest-path "$I18N_TRANSLATOR_MANIFEST" -p greentic-i18n-translator -- \
      --locale "$LOCALE" \
      translate --langs "$joined" --en "$EN_PATH" --auth-mode "$AUTH_MODE"
    index=$(( index + batch_lang_cap ))
  done
}

run_validate() {
  cargo run --quiet -- --locale "$LOCALE" i18n validate
}

run_status() {
  cargo run --quiet -- --locale "$LOCALE" i18n status
}

if [[ "$MODE" == "-h" || "$MODE" == "--help" ]]; then
  usage
  exit 0
fi

case "$MODE" in
  seed)
    seed_missing_locales
    ;;
  translate)
    seed_missing_locales
    translate_batches
    ;;
  validate)
    seed_missing_locales
    run_validate
    ;;
  status)
    seed_missing_locales
    run_status
    ;;
  all)
    seed_missing_locales
    translate_batches
    run_validate
    run_status
    ;;
  *)
    usage
    exit 2
    ;;
esac
