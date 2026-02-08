#!/usr/bin/env bash
set -euo pipefail

HARDENING_BIN="${HARDENING_BIN:-hardening}"
INTERVAL_SECONDS="${HARDENING_OPS_INTERVAL_SECONDS:-3600}"

usage() {
  cat <<'EOF'
Usage: hardening-ops.sh [--timer] [--interval seconds] <command>

Commands:
  bootstrap  Run hardening bootstrap step.
  auth       Run hardening auth step.
  apply      Run hardening apply step.
  backup     Run hardening backup step.
  all        Run all steps in order: bootstrap, auth, apply, backup.

Options:
  --timer              Run all steps on an interval (default 3600s).
  --interval seconds   Override timer interval in seconds.
EOF
}

run_step() {
  local step="$1"
  "${HARDENING_BIN}" "${step}"
}

run_all() {
  run_step bootstrap
  run_step auth
  run_step apply
  run_step backup
}

TIMER_MODE=false
COMMAND=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --timer)
      TIMER_MODE=true
      shift
      ;;
    --interval)
      if [[ $# -lt 2 ]]; then
        echo "Error: --interval requires a value." >&2
        usage
        exit 1
      fi
      INTERVAL_SECONDS="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      if [[ -n "${COMMAND}" ]]; then
        echo "Error: multiple commands provided." >&2
        usage
        exit 1
      fi
      COMMAND="$1"
      shift
      ;;
  esac
done

if [[ -z "${COMMAND}" ]]; then
  COMMAND="all"
fi

if [[ "${TIMER_MODE}" == "true" ]]; then
  if [[ "${COMMAND}" != "all" ]]; then
    echo "Error: --timer only supports the 'all' command." >&2
    usage
    exit 1
  fi
  while true; do
    run_all
    sleep "${INTERVAL_SECONDS}"
  done
fi

case "${COMMAND}" in
  bootstrap|auth|apply|backup)
    run_step "${COMMAND}"
    ;;
  all)
    run_all
    ;;
  *)
    echo "Error: unknown command '${COMMAND}'." >&2
    usage
    exit 1
    ;;
esac
