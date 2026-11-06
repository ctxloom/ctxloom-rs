#!/usr/bin/env bash
# A test does not write an executable file to run it: exec'ing a file a test
# has just written fails with "text file busy" (ETXTBSY) whenever another
# thread's fork holds a copy of the writer. A fake program is
# `fixture_program::install_program` (ctxloom-lib's fixture-program crate),
# which never execs a file the test wrote.
#
# Fail on an executable octal mode (any of the owner, group or other execute
# bits: 0o755, 0o700, 0o111...) written in a test: a `*.test.rs` file or a
# `.rs` file under a `tests/` directory of crates/. Not flagged: a mask being
# compared (`mode & 0o111`), a literal in quotes, an assertion (it reads a
# mode, it does not give one), and a statement that makes a directory
# (`create_dir_all`, `make_temp_dir`, `Mode::DIR`). A statement is
# the lines up to the next `;`, `{` or `}`. A mode that is the point of a test
# (the execute bit of a file git commits or a listing reports, a mode that
# is data) is an exception in scripts/exec-mode-allow.txt (one per line:
# `<file> | <text on the line> | <why>`), so each carries its reason and a new
# one is seen in review; an entry that no longer matches anything fails too.
#
# Usage: exec-mode-check.sh [root]   (root defaults to the repository)
set -euo pipefail
top=${1:-$(git rev-parse --show-toplevel)}
cd "$top"
allow_file=scripts/exec-mode-allow.txt

files=() fragments=()
if [ -f "$allow_file" ]; then
  while IFS= read -r entry; do
    [[ -z ${entry//[[:space:]]/} || $entry == \#* ]] && continue
    IFS='|' read -r file fragment _ <<< "$entry"
    file=$(echo "$file" | xargs)
    fragment=${fragment#"${fragment%%[![:space:]]*}"}
    fragment=${fragment%"${fragment##*[![:space:]]}"}
    if [ -z "$file" ] || [ -z "$fragment" ]; then
      echo "FAIL lint-fixtures: $allow_file: an entry needs a file and the text on its line: $entry" >&2
      exit 1
    fi
    files+=("$file") fragments+=("$fragment")
  done < "$allow_file"
fi
used=()
for _ in "${files[@]+"${files[@]}"}"; do used+=(0); done

scan() {
  awk '
    function flush(   i, joined) {
      joined = ""
      for (i = 1; i <= n; i++) joined = joined text[i] "\n"
      if (joined !~ /create_dir_all|make_temp_dir|Mode::DIR|PRIVATE_DIR|assert/) {
        for (i = 1; i <= n; i++) {
          if (text[i] ~ /(^|[^"0-9A-Za-z_])0o[0-7_]*[1357][0-7_]?[0-7_]?([^0-7_]|$)/ && text[i] !~ /& *0o/)
            print FILENAME ":" num[i] ":" text[i]
        }
      }
      n = 0
    }
    FNR == 1 { n = 0 }
    { n++; text[n] = $0; num[n] = FNR }
    /[;{}][[:space:]]*$/ { flush() }
  ' "$@"
}

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
  done < <(find crates \( -name '*.test.rs' -o \( -name '*.rs' -path '*/tests/*' \) \) -not -path '*/target/*' -print0 \
    | sort -z | xargs -0 -r bash -c "$(declare -f scan); scan \"\$@\"" _)
fi

stale=0
for i in "${!files[@]}"; do
  if [ "${used[$i]}" = 0 ]; then
    echo "$allow_file: matches nothing now: ${files[$i]} | ${fragments[$i]}" >&2
    stale=$((stale + 1))
  fi
done

if [ "$found" -gt 0 ]; then
  echo "FAIL lint-fixtures: $found test lines give a file an executable mode: a fake program to run is fixture_program::install_program (ctxloom-lib's fixture-program crate), never a written 0o755 file (ETXTBSY); or, if the mode is the point of the test, list the line in $allow_file with why" >&2
  exit 1
fi
if [ "$stale" -gt 0 ]; then
  echo "FAIL lint-fixtures: $stale entries of $allow_file match nothing: remove them" >&2
  exit 1
fi
echo "ok   lint-fixtures (workspace)"
