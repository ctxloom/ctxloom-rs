#!/usr/bin/env bash
# What differs between operating systems lives in angzarr-platform's os crate
# alone. Fail on a cfg (or cfg_attr) naming an OS, `cfg!(`, `std::os::` or
# `split_paths` anywhere under crates/, tests included, and on a crate with
# target-specific dependencies. The exceptions (the seam) are the allowlist,
# scripts/os-cfg-allow.txt (one per line: `<file> | <text on the line> |
# <why>`), so each carries its reason and a new one is seen in review; an
# entry that no longer matches anything fails too, so the list stays true.
#
# Usage: os-cfg-check.sh [root]   (root defaults to the repository)
set -euo pipefail
top=${1:-$(git rev-parse --show-toplevel)}
cd "$top"
allow_file=scripts/os-cfg-allow.txt
pattern='cfg(_attr)?\((all\(|any\(|not\()*[^)]*\b(unix|windows|target_os|target_family)\b|cfg!\(|std::os::|split_paths'

files=() fragments=()
if [ -f "$allow_file" ]; then
  while IFS= read -r entry; do
    [[ -z ${entry//[[:space:]]/} || $entry == \#* ]] && continue
    IFS='|' read -r file fragment _ <<< "$entry"
    file=$(echo "$file" | xargs)
    fragment=${fragment#"${fragment%%[![:space:]]*}"}
    fragment=${fragment%"${fragment##*[![:space:]]}"}
    if [ -z "$file" ] || [ -z "$fragment" ]; then
      echo "FAIL lint-os: $allow_file: an entry needs a file and the text on its line: $entry" >&2
      exit 1
    fi
    files+=("$file") fragments+=("$fragment")
  done < "$allow_file"
fi
used=()
for _ in "${files[@]+"${files[@]}"}"; do used+=(0); done

found=0
if [ -d crates ]; then
  while IFS= read -r hit; do
    file=${hit%%:*}
    rest=${hit#*:}
    line=${rest#*:}
    allowed=0
    for i in "${!files[@]}"; do
      if [[ $file == "${files[$i]}" && $line == *"${fragments[$i]}"* ]]; then
        allowed=1 used[i]=1
      fi
    done
    [ "$allowed" = 1 ] && continue
    echo "$hit"
    found=$((found + 1))
  done < <(grep -rnE --include='*.rs' "$pattern" crates || true)

  # A manifest at any depth (crates/<ring>/<role>/<crate>/Cargo.toml, or the
  # core's nesting).
  while IFS= read -r hit; do
    echo "$hit"
    found=$((found + 1))
  done < <(find crates -name Cargo.toml -not -path '*/target/*' -print0 | xargs -0 -r grep -nE '^\[target\.' || true)
fi

stale=0
for i in "${!files[@]}"; do
  if [ "${used[$i]}" = 0 ]; then
    echo "$allow_file: matches nothing now: ${files[$i]} | ${fragments[$i]}" >&2
    stale=$((stale + 1))
  fi
done

if [ "$found" -gt 0 ]; then
  echo "FAIL lint-os: $found OS-specific lines: move them onto angzarr-platform's os crate (its Flavor or Os), or list an allowed one in $allow_file with why" >&2
  exit 1
fi
if [ "$stale" -gt 0 ]; then
  echo "FAIL lint-os: $stale entries of $allow_file match nothing: remove them" >&2
  exit 1
fi
echo "ok   lint-os (workspace)"
