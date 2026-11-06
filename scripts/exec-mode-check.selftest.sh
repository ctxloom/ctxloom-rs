#!/usr/bin/env bash
# Self-test of exec-mode-check.sh against fixture trees: each way a test gives
# a file an executable mode is flagged; a clean tree, a mask comparison, a
# directory, a quoted literal, non-test code and a listed exception pass; a
# stale exception fails.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
check="$here/exec-mode-check.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fixture() { # name, test-file content, [allow], [file name]
  local d="$work/$1" name=${4:-lib.test.rs}
  mkdir -p "$d/crates/a/b/x/src" "$d/scripts"
  printf '%s\n' "$2" > "$d/crates/a/b/x/src/$name"
  [ -n "${3:-}" ] && printf '%s\n' "$3" > "$d/scripts/exec-mode-allow.txt"
  echo "$d"
}
expect() { # code, dir, label
  local got=0
  "$check" "$2" >/dev/null 2>&1 || got=$?
  if [ "$got" != "$1" ]; then echo "selftest FAIL: $3 (exit $got, wanted $1)" >&2; exit 1; fi
}

expect 0 "$(fixture clean 'fn f() { fs.write_atomic(p, b, Perms::Exact(Mode::FILE)); }')" clean
expect 1 "$(fixture exact 'fn f() { fs.write_atomic(p, b, Perms::Exact(Mode::from_bits(0o755))); }')" "Perms::Exact(0o755)"
expect 1 "$(fixture owner-only 'fn f() { set_mode(p, 0o700); }')" "0o700"
expect 1 "$(fixture group 'fn f() { set_mode(p, 0o010); }')" "0o010"
expect 1 "$(fixture other 'fn f() { set_mode(p, 0o001); }')" "0o001"
expect 1 "$(fixture variable 'fn f() {
    let mode = Mode::from_bits(0o775);
}')" "0o775 on its own line"
expect 1 "$(fixture multiline 'fn f() {
    fs.write_atomic(
        p,
        b,
        Perms::Exact(Mode::from_bits(0o755)),
    ).unwrap();
}')" "a call over several lines"
d="$work/tests-dir"
mkdir -p "$d/crates/a/b/x/tests" "$d/scripts"
printf 'fn f() { set_mode(p, 0o755); }\n' > "$d/crates/a/b/x/tests/it.rs"
expect 1 "$d" "a file under tests/"
expect 0 "$(fixture plain-mode 'fn f() { set_mode(p, 0o644); set_mode(q, 0o600); }')" "modes with no execute bit"
expect 0 "$(fixture mask 'fn f() { assert!(mode & 0o111 != 0); }')" "a mask compared"
expect 0 "$(fixture dir 'fn f() { fs.create_dir_all(p, Mode::from_bits(0o755)).unwrap(); }')" "a directory"
expect 0 "$(fixture dir-multiline 'fn f() {
    fs.create_dir_all(
        p,
        Mode::from_bits(0o755),
    ).unwrap();
}')" "a directory over several lines"
expect 1 "$(fixture dir-then-file 'fn f() {
    fs.create_dir_all(d, Mode::DIR).unwrap();
    fs.write_atomic(p, b, Perms::Exact(Mode::from_bits(0o755))).unwrap();
}')" "a file written after a directory made"
expect 0 "$(fixture assertion 'fn f() { assert_eq!(mode_of(p), 0o755); }')" "an assertion of a mode"
expect 0 "$(fixture assertion-multiline 'fn f() {
    assert_eq!(
        mode_of(p),
        0o700,
        "message"
    );
}')" "an assertion of a mode over several lines"
expect 1 "$(fixture after-assertion 'fn f() {
    assert!(ok);
    set_mode(p, 0o755);
}')" "a mode given after an assertion"
expect 0 "$(fixture quoted 'fn f() { let x = ["0o7", "0o755"]; }')" "a literal in quotes"
expect 0 "$(fixture production 'fn f() { set_mode(p, 0o755); }' '' lib.rs)" "non-test code"
expect 0 "$(fixture allowed 'fn f() { set_mode(p, 0o755); }' 'crates/a/b/x/src/lib.test.rs | set_mode(p, 0o755) | the execute bit is the point')" allowed
expect 1 "$(fixture stale 'fn f() {}' 'crates/a/b/x/src/lib.test.rs | set_mode(p, 0o755) | gone')" "stale exception"
out=$("$check" "$(fixture message 'fn f() { set_mode(p, 0o755); }')" 2>&1 || true)
case $out in
  *"lib.test.rs:1:"*install_program*) ;;
  *) echo "selftest FAIL: the failure does not name the line and the helper: $out" >&2; exit 1 ;;
esac
echo "ok   lint-fixtures selftest"
