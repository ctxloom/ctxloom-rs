#!/usr/bin/env bash
# The OS and filesystem rule's sync check: fail when a clippy.toml disagrees
# with the canonical list (templates/clippy-disallowed.toml). An entry is its
# `path` and whether it is `allow-invalid` (the reason text is documentation,
# not part of the rule).
#
# Usage: clippy-sync-check.sh [--exact] <clippy.toml> [template]
#   default: every template entry must be in the clippy.toml (a consumer may
#            disallow more of its own);
#   --exact: the two sets must be equal (this repository's own clippy.toml).
#   template defaults to templates/clippy-disallowed.toml beside this script's
#   repository; a consumer passes its checkout of the template.
set -euo pipefail
exact=0
if [ "${1:-}" = --exact ]; then exact=1; shift; fi
target=${1:?usage: clippy-sync-check.sh [--exact] <clippy.toml> [template]}
template=${2:-$(cd "$(dirname "$0")/.." && pwd)/templates/clippy-disallowed.toml}
for f in "$target" "$template"; do
  [ -f "$f" ] || { echo "FAIL lint-clippy-sync: no such file: $f" >&2; exit 1; }
done

entries() { # an entry per line: the path, marked when allow-invalid
  grep -E '^[[:space:]]*\{[[:space:]]*path = "' "$1" \
    | sed -E 's/^[[:space:]]*\{[[:space:]]*path = "([^"]*)".*allow-invalid = true.*/\1 allow-invalid/; t; s/^[[:space:]]*\{[[:space:]]*path = "([^"]*)".*/\1/' \
    | sort -u
}
missing=$(comm -13 <(entries "$target") <(entries "$template") | tr '\n' ';')
extra=$(comm -23 <(entries "$target") <(entries "$template") | tr '\n' ';')
if [ -n "$missing" ]; then
  echo "FAIL lint-clippy-sync: $target lacks (or differs on) entries of $template: $missing" >&2
  exit 1
fi
if [ "$exact" = 1 ] && [ -n "$extra" ]; then
  echo "FAIL lint-clippy-sync: $target has entries $template lacks: $extra" >&2
  exit 1
fi
echo "ok   lint-clippy-sync ($target)"
