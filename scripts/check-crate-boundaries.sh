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
domain_packages=(pkms-org pkms-db pkms-rag pkms-task pkms-web)
leaf_packages=(pkms-tokens)

for package in "${domain_packages[@]}" "${leaf_packages[@]}"; do
  require_present pkms "$package" "$pkms_tree"
done

# Domain crates may depend on pkms-org, but not on the umbrella crate or peer
# domain crates. Checking the all-feature cargo tree rejects indirect forbidden
# dependencies as well as direct manifest edges.
for package in "${domain_packages[@]}"; do
  package_tree="$(tree_for "$package")"
  forbidden=(pkms)
  for candidate in "${domain_packages[@]}"; do
    if [[ "$candidate" != "$package" && "$candidate" != pkms-org ]]; then
      forbidden+=("$candidate")
    fi
  done
  reject_forbidden "$package" "$package_tree" "${forbidden[@]}"
done

# Leaf utility crates may be shared by domain crates, but must not acquire an
# edge back into the umbrella or any domain package.
for package in "${leaf_packages[@]}"; do
  package_tree="$(tree_for "$package")"
  reject_forbidden "$package" "$package_tree" pkms "${domain_packages[@]}"
done

printf 'Crate dependency boundaries OK\n'
