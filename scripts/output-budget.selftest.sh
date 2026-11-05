#!/usr/bin/env bash
# The output budget of the gates, checked on a fixture workspace: a pass is at
# most 2 lines, a failure is its FAIL line plus at most 40 lines of extract
# (and a cap note), and a failure names failing tests only, never passing ones.
# The gates are gates/gates.just (or $GATES_JUST, to measure another version).
# MEASURE=1 also prints each case's lines and bytes.
set -euo pipefail
# Under a git hook, git exports variables naming the real repository; the
# fixtures' own git commands must not inherit them.
while IFS= read -r v; do unset "$v"; done < <(git rev-parse --local-env-vars)
here=$(cd "$(dirname "$0")" && pwd)
gates=${GATES_JUST:-$here/../gates/gates.just}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail() { echo "FAIL output-budget.selftest: $*" >&2; [ "${BUDGET:-}" = off ] || exit 1; }

fx=$tmp/fx
mkdir -p "$fx/gates" "$fx/crates/a/src"
cp "$gates" "$fx/gates/gates.just"
cat > "$fx/Cargo.toml" <<'T'
[workspace]
members = ["crates/a"]
resolver = "2"
T
cat > "$fx/crates/a/Cargo.toml" <<'T'
[package]
name = "a"
version = "0.1.0"
edition = "2021"
T
cat > "$fx/justfile" <<'J'
TOP := `git rev-parse --show-toplevel`
import 'gates/gates.just'
staged_tree := "../.{name}.staged"
feature_rules := ""
features_prepare := ""
mutation_thresholds := "crates=90"
workspace_wide := "Cargo.toml"
build:
    @just --justfile {{TOP}}/justfile _run build workspace cargo cargo build -q --message-format=short --manifest-path {{TOP}}/Cargo.toml --workspace
lint:
    @just --justfile {{TOP}}/justfile _run lint workspace cargo cargo clippy -q --message-format=short --manifest-path {{TOP}}/Cargo.toml --workspace --all-targets -- -D warnings
J
git -C "$fx" init -q
export CARGO_TARGET_DIR=$tmp/target CARGO_TERM_COLOR=never
lib() { printf '%s\n' "$1" > "$fx/crates/a/src/lib.rs"; }
good_tests=$(for i in $(seq 1 30); do printf '    #[test]\n    fn passing_test_%d() {}\n' "$i"; done)
tests_mod() { printf '#[cfg(test)]\nmod tests {\n%s\n%s\n}\n' "$good_tests" "$1"; }
bad_tests='    #[test]
    fn failing_one() { assert_eq!(1, 2, "boom one"); }
    #[test]
    fn failing_two() { assert_eq!(vec![1; 30], vec![2; 30], "boom two"); }'
needless=$(for i in $(seq 1 30); do printf 'pub fn f%d() -> u32 { return 1; }\n' "$i"; done)
badtypes=$(for i in $(seq 1 30); do printf 'pub fn g%d() { let _x: u32 = "s"; }\n' "$i"; done)

row() { # name recipe expect-pass
    local name=$1 rc=0 out lines bytes
    shift
    out=$(cd "$fx" && just "$@" 2>&1) || rc=$?
    lines=$(grep -c '' <<< "$out" || true); bytes=${#out}
    [ "${MEASURE:-}" != 2 ] || printf "%s\n" "$out"
    [ -z "${MEASURE:-}" ] || printf '%-34s exit %-3s %4d lines %6d bytes\n' "$name" "$rc" "$lines" "$bytes"
    OUT=$out RC=$rc LINES=$lines
}
pass_case() { # name recipe...
    row "$@"
    [ "$RC" -eq 0 ] || fail "$1 should pass: $OUT"
    [ "$LINES" -le 2 ] || fail "$1: a pass printed $LINES lines (budget 2): $OUT"
    grep -q '^ok ' <<< "$OUT" || fail "$1: no ok line: $OUT"
}
fail_case() {
    row "$@"
    [ "$RC" -ne 0 ] || fail "$1 should fail"
    # FAIL line + at most 40 extract lines + the cap note.
    [ "$LINES" -le 42 ] || fail "$1: a failure printed $LINES lines (budget 42): $OUT"
    head -1 <<< "$OUT" | grep -q '^FAIL [a-z-]*: .*(full log: target/logs/[a-z-]*\.log)$' || fail "$1: no FAIL line first: $OUT"
}

lib "pub fn f() -> u32 { 1 }
$(tests_mod '')"
rustfmt --edition 2021 "$fx/crates/a/src/lib.rs"
(cd "$fx" && git add -A && git -c user.email=t@e -c user.name=t commit -qm init)
pass_case "build (pass)" build
pass_case "lint (pass)" lint
pass_case "test (pass)" test
pass_case "fmt-check (pass)" fmt-check

lib "$badtypes"; fail_case "build (30 type errors)" build
[ "$(grep -c 'error' <<< "$OUT")" -le 24 ] || fail "build: more than 20 diagnostics listed"
grep -q 'more, see the log' <<< "$OUT" || fail "build: no cap note: $OUT"
lib "$needless"; fail_case "lint (30 clippy errors)" lint
grep -q 'more, see the log' <<< "$OUT" || fail "lint: no cap note: $OUT"
lib "pub fn f() -> u32 { 1 }
$(tests_mod "$bad_tests")"; fail_case "test (2 failing, 30 passing)" test
grep -q 'failing_one' <<< "$OUT" && grep -q 'failing_two' <<< "$OUT" || fail "test: failing names missing: $OUT"
! grep -q 'passing_test' <<< "$OUT" || fail "test: a passing test is named: $OUT"
lib "pub fn   f( )->u32{1}"; fail_case "fmt-check (1 file)" fmt-check
echo "ok   output-budget.selftest"
