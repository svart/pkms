#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

tree_for() {
  cargo tree -p "$1" --all-features --prefix none
}

contains_crate() {
  local tree="$1"
  local crate="$2"
  local line

  while IFS= read -r line; do
    if [[ "$line" == "$crate v"* ]]; then
      return 0
    fi
  done <<< "$tree"

  return 1
}

require_present() {
  local package="$1"
  local required="$2"
  local tree="$3"

  if ! contains_crate "$tree" "$required"; then
    printf 'Expected %s to depend on %s\n' "$package" "$required" >&2
    return 1
  fi
}

reject_forbidden() {
  local package="$1"
  local tree="$2"
  shift 2

  local forbidden
  for forbidden in "$@"; do
    if contains_crate "$tree" "$forbidden"; then
      printf 'Forbidden dependency: %s -> %s\n' "$package" "$forbidden" >&2
      return 1
    fi
  done
}

pkms_tree="$(tree_for pkms)"
require_present pkms pkms-org "$pkms_tree"
require_present pkms pkms-db "$pkms_tree"
require_present pkms pkms-task "$pkms_tree"
require_present pkms pkms-web "$pkms_tree"

org_tree="$(tree_for pkms-org)"
reject_forbidden pkms-org "$org_tree" pkms pkms-db pkms-task pkms-web

db_tree="$(tree_for pkms-db)"
reject_forbidden pkms-db "$db_tree" pkms pkms-task pkms-web

rag_tree="$(tree_for pkms-rag)"
require_present pkms-rag pkms-org "$rag_tree"
reject_forbidden pkms-rag "$rag_tree" pkms pkms-db pkms-task pkms-web

task_tree="$(tree_for pkms-task)"
reject_forbidden pkms-task "$task_tree" pkms pkms-db pkms-web

web_tree="$(tree_for pkms-web)"
reject_forbidden pkms-web "$web_tree" pkms pkms-db pkms-task

printf 'Crate dependency boundaries OK\n'
