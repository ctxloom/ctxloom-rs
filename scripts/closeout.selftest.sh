#!/usr/bin/env bash
# Self-test of closeout.sh: a fixture repository whose justfile has stub gates.
# Every gate runs even after one fails; the summary names the failures; the
# exit code follows; mutants-batch gets the base argument.
set -euo pipefail
# Under a git hook, git exports variables naming the real repository; the
# fixtures' own git commands must not inherit them.
while IFS= read -r v; do unset "$v"; done < <(git rev-parse --local-env-vars)
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail() { echo "FAIL closeout.selftest: $*" >&2; exit 1; }
git init -q "$tmp/r"
cat > "$tmp/r/justfile" <<'J'
one:
    @echo "ok   one (stub) 0.1s"
two:
    @echo "FAIL two: broken (full log: x)" >&2; exit 3
three:
    @echo "ok   three (stub) 0.1s"
mutants-batch base="":
    @echo "ok   mutants-batch (base=[{{base}}]) 0.1s"
J
run() { (cd "$tmp/r" && "$@" "$here/closeout.sh" $ARGS 2>&1); }

ARGS="" rc=0
out=$(CLOSEOUT_GATES="one two three mutants-batch" run env) || rc=$?
[ "$rc" -eq 1 ] || fail "exit was $rc"
grep -q '^ok   three' <<< "$out" || fail "a gate after the failing one did not run: $out"
grep -q '^FAIL closeout: 1 of 4 gates failed: two ' <<< "$out" || fail "summary wrong: $out"
grep -q 'base=\[\]' <<< "$out" || fail "empty base not passed: $out"

ARGS="abc123" rc=0
out=$(CLOSEOUT_GATES="one three mutants-batch" run env) || rc=$?
[ "$rc" -eq 0 ] || fail "green run exited $rc: $out"
grep -q 'base=\[abc123\]' <<< "$out" || fail "base not passed to mutants-batch: $out"
grep -q '^ok   closeout (3 gates) ' <<< "$out" || fail "summary wrong: $out"
echo "ok   closeout.selftest"
