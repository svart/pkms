#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

fail_on_match() {
  local description="$1"
  local pattern="$2"
  local path="$3"
  local output
  local status

  set +e
  output="$(rg -n --glob '*.rs' "$pattern" "$path")"
  status=$?
  set -e

  if [[ "$status" -eq 0 ]]; then
    printf '%s\n' "$output"
    printf '\nForbidden %s in %s\n' "$description" "$path" >&2
    return 1
  fi

  if [[ "$status" -gt 1 ]]; then
    printf 'Could not check %s in %s\n' "$description" "$path" >&2
    return "$status"
  fi
}

fail_on_match \
  "raw filesystem writes; use pkms-org typed edit APIs" \
  'std::fs::(write|create_dir_all)' \
  crates/pkms-task/src

fail_on_match \
  "raw org task syntax formatting; use pkms-org task edit APIs" \
  'format!\("\*|SCHEDULED:|DEADLINE:|\[#' \
  crates/pkms-task/src

fail_on_match \
  "Todoist HTTP/API execution from umbrella task commands; keep it in pkms-task" \
  'todoist::TodoistClient|TodoistCreateTaskRequest|quick_add|update_task|close_task|reopen_task|create_task|task_to_item_with_metadata|enrich_items_with_pkms_notes' \
  crates/pkms/src/commands/task

printf 'Task crate boundaries OK\n'
