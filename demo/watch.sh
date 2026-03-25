#!/usr/bin/env bash
# Live dashboard that watches actual flow execution results.
# Usage: bash demo/watch.sh
set -euo pipefail

RUNS_DIR="demo/state/runs/events/redbutton-app/default"
SEEN_FILE="/tmp/redbutton-seen-$$"
touch "$SEEN_FILE"
COUNT=0

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
DIM='\033[2m'
BOLD='\033[1m'
RESET='\033[0m'

clear
printf "${RED}${BOLD}"
cat << 'BANNER'
  ____          _   ____        _   _
 |  _ \ ___  __| | | __ ) _   _| |_| |_ ___  _ __
 | |_) / _ \/ _` | |  _ \| | | | __| __/ _ \| '_ \
 |  _ <  __/ (_| | | |_) | |_| | |_| || (_) | | | |
 |_| \_\___|\__,_| |____/ \__,_|\__|\__\___/|_| |_|
BANNER
printf "${RESET}\n"
printf "${DIM}Watching flow executions in ${RUNS_DIR}${RESET}\n"
printf "${DIM}Send events via curl or redbutton CLI. Ctrl+C to stop.${RESET}\n"
printf "${DIM}─────────────────────────────────────────────────────${RESET}\n\n"

while true; do
  if [[ -d "$RUNS_DIR" ]]; then
    for run_dir in "$RUNS_DIR"/*/; do
      [[ -d "$run_dir" ]] || continue
      run_id=$(basename "$run_dir")

      # Skip already seen
      if grep -q "^${run_id}$" "$SEEN_FILE" 2>/dev/null; then
        continue
      fi
      echo "$run_id" >> "$SEEN_FILE"

      summary="$run_dir/summary.txt"
      transcript="$run_dir/transcript.jsonl"

      [[ -f "$summary" ]] || continue

      COUNT=$((COUNT + 1))
      status=$(grep "^status:" "$summary" | cut -d' ' -f2)

      if [[ "$status" == "Success" ]]; then
        STATUS_COLOR="$GREEN"
        STATUS_ICON="✓"
      else
        STATUS_COLOR="$RED"
        STATUS_ICON="✗"
      fi

      # Extract component output from transcript
      output=""
      if [[ -f "$transcript" ]]; then
        output=$(python3 -c "
import sys, json
for line in open('$transcript'):
    obj = json.loads(line)
    if obj.get('outputs'):
        msg = obj['outputs'].get('message','')
        try:
            parsed = json.loads(msg)
            print(parsed.get('text', msg) if isinstance(parsed, dict) else msg)
        except:
            print(msg)
" 2>/dev/null || true)
      fi

      printf "${RED}${BOLD}[#%d]${RESET} " "$COUNT"
      printf "${STATUS_COLOR}${STATUS_ICON} %s${RESET} " "$status"
      printf "${DIM}run=%s${RESET}\n" "$run_id"

      if [[ -n "$output" ]]; then
        echo "$output" | while IFS= read -r line; do
          printf "  ${CYAN}│${RESET} %s\n" "$line"
        done
      fi
      printf "  ${CYAN}└─${RESET} ${DIM}%s${RESET}\n\n" "$(date '+%H:%M:%S')"
    done
  fi
  sleep 1
done
