#!/usr/bin/env bash
# Self-test of mutants-batch.sh against a fixture repository and a stub gate
# (the seam MUTANTS_GATE) that writes the logs `just mutants-staged` writes.
# Checks: the batch diff is applied to the index at the base; the verdict is
# terse (pass, FAIL with capped survivors, timeout); the default base is the
# merge-base with main; and the worktree and what the run left running are
# gone afterwards.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'pkill -KILL -f -- "$tmp" 2>/dev/null || true; rm -rf "$tmp"' EXIT
fail() { echo "FAIL mutants-batch.selftest: $*" >&2; exit 1; }

repo=$tmp/repo
git init -q -b main "$repo"
git -C "$repo" config user.email t@example.com
git -C "$repo" config user.name t
echo base > "$repo/README"
git -C "$repo" add . && git -C "$repo" commit -qm base
git -C "$repo" checkout -q -b feature
mkdir -p "$repo/crates/x/src"
echo 'pub fn f() {}' > "$repo/crates/x/src/lib.rs"
git -C "$repo" add . && git -C "$repo" commit -qm feature
echo 'pub fn g() {}' >> "$repo/crates/x/src/lib.rs"
git -C "$repo" commit -qam feature2
main_sha=$(git -C "$repo" rev-parse --short=7 main)
head_sha=$(git -C "$repo" rev-parse --short=7 HEAD)

# The stub gate: MODE says what it does. It sees the batch applied (and not
# committed) in its worktree and leaves a stray process behind, as a mutant
# that broke a test's stop condition does.
cat > "$tmp/gate.sh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[ "$(git diff --cached --name-only HEAD)" = crates/x/src/lib.rs ] || { echo "diff not applied: $(git diff --cached --name-only HEAD)"; exit 9; }
grep -q 'pub fn g' crates/x/src/lib.rs || { echo "head content missing"; exit 9; }
setsid nohup sleep 300 > /dev/null 2>&1 < /dev/null &
mkdir -p target/logs/mutants-staged/mutants.out
case "$MODE" in
pass)
    { echo "mutants-staged: kill rate by tier:"; echo "  pass  crates/core/domain: 4/4 killed (100%), threshold 90%"; } > target/logs/mutants-staged.log
    echo "ok   mutants-staged (staged diff) 1.0s" ;;
fail)
    { echo "mutants-staged: kill rate by tier:"; echo "  FAIL  crates/core/domain: 1/5 killed (20%), threshold 90%"; } > target/logs/mutants-staged.log
    echo "below the kill-rate threshold: crates/core/domain: 1/5" > target/logs/mutants-staged/verdict.txt
    for i in $(seq 1 20); do echo "src/lib.rs:$i: replace f -> ()" ; done > target/logs/mutants-staged/mutants.out/missed.txt
    exit 1 ;;
hang) sleep 300 ;;
esac
STUB
chmod +x "$tmp/gate.sh"
run() { (cd "$repo" && MUTANTS_GATE="$tmp/gate.sh" MUTANTS_POLL=0.2 "$@" "$here/mutants-batch.sh" $ARGS); }
leftover() {
    [ -z "$(git -C "$repo" worktree list | grep -v "^$repo ")" ] || fail "worktree left: $(git -C "$repo" worktree list)"
    [ ! -e "$tmp/.repo.mutants-batch" ] || fail "worktree directory left"
    # The stray process runs in the throwaway's cwd.
    for p in $(pgrep -x sleep || true); do
        [ "$(readlink "/proc/$p/cwd" 2>/dev/null | grep -c "$tmp")" = 0 ] || fail "stray process $p left running"
    done
}

# Pass, default base (merge-base with main) and default head.
ARGS=""
out=$(MODE=pass run env) || fail "pass run exited non-zero: $out"
grep -q "^ok   mutants-batch ($main_sha..$head_sha) " <<< "$out" || fail "pass line wrong: $out"
grep -q 'pass  crates/core/domain: 4/4' <<< "$out" || fail "tier line missing: $out"
[ "$(wc -l <<< "$out")" -le 3 ] || fail "pass output not terse: $out"
leftover

# Failure: non-zero, summary, tiers, survivors capped, log path.
ARGS="main HEAD"
rc=0; out=$(MODE=fail MUTANTS_SURVIVORS_MAX=5 run env 2>&1) || rc=$?
[ "$rc" -ne 0 ] || fail "failing gate exited 0"
grep -q '^FAIL mutants-batch: below the kill-rate threshold: crates/core/domain: 1/5 (full log: target/logs/mutants-batch/mutants-staged.log)' <<< "$out" || fail "FAIL line wrong: $out"
grep -q 'surviving mutants (20, first 5 shown)' <<< "$out" || fail "survivor header wrong: $out"
grep -q '\.\.\. and 15 more' <<< "$out" || fail "survivor cap note missing: $out"
[ "$(grep -c 'replace f' <<< "$out")" -eq 5 ] || fail "survivors not capped to 5: $out"
[ -s "$repo/target/logs/mutants-batch/mutants-staged/mutants.out/missed.txt" ] || fail "logs not kept"
leftover

# Timeout: 124, and the hung run is killed.
rc=0; out=$(MODE=hang MUTANTS_TIMEOUT=2 run env 2>&1) || rc=$?
[ "$rc" -eq 124 ] || fail "timeout exit was $rc: $out"
grep -q '^FAIL mutants-batch: timed out after 2s' <<< "$out" || fail "timeout line wrong: $out"
leftover

# An empty batch (base == head) applies nothing: the stub's diff check fails it.
ARGS="HEAD HEAD"
rc=0; MODE=pass run env >/dev/null 2>&1 || rc=$?
[ "$rc" -ne 0 ] || fail "an empty batch applied a diff"
leftover

# Bad refs.
ARGS="nonesuch"
rc=0; out=$(MODE=pass run env 2>&1) || rc=$?
[ "$rc" -ne 0 ] && grep -q 'is not a commit' <<< "$out" || fail "bad base accepted: $out"

echo "ok   mutants-batch.selftest"
