#!/usr/bin/env bash
# Mutation-test a batch of commits: `just mutants-batch [base] [head]`.
#
# The close-out ceremony in one command. base defaults to the merge-base of
# `main` and head, head to HEAD. In a throwaway detached worktree at base the
# batch's diff (base..head) is applied to the index, and `just mutants-staged`
# is run there in a session of its own (setsid nohup, so a harness that
# backgrounds this script loses nothing), polled until it writes its `exit N`
# line. Only the terse verdict is printed:
#
#   ok   mutants-batch (<base>..<head>) 412.0s          (or)
#   FAIL mutants-batch: <summary> (full log: target/logs/mutants-batch/mutants-staged.log)
#     pass  crates/core/domain: 12/12 killed (100%), threshold 90%
#     surviving mutants (3, first 15 shown): ...
#
# The run's logs are copied to target/logs/mutants-batch/ before the worktree
# goes. The worktree and everything the run left running are always cleaned
# (trap), whatever ends this script. Exit: 0 on a pass, the gate's code (1 if
# it died or its code was 0 with no verdict) on a failure, 124 on a timeout.
#
# Environment: MUTANTS_TIMEOUT seconds to wait (default 3600), MUTANTS_POLL
# seconds between polls (default 5), MUTANTS_SURVIVORS_MAX survivors listed
# (default 15), MUTANTS_GATE the command run in the worktree (default
# `just mutants-staged`; the self-test's seam).
set -euo pipefail

timeout=${MUTANTS_TIMEOUT:-3600}
poll=${MUTANTS_POLL:-5}
cap=${MUTANTS_SURVIVORS_MAX:-15}
read -ra gate <<< "${MUTANTS_GATE:-just mutants-staged}"

g() { env -u GIT_INDEX_FILE -u GIT_DIR -u GIT_WORK_TREE git "$@"; }

top=$(g rev-parse --show-toplevel)
head=${2:-HEAD}
[ -n "$head" ] || head=HEAD
head_sha=$(g -C "$top" rev-parse --verify --quiet "$head^{commit}") || { echo "FAIL mutants-batch: $head is not a commit" >&2; exit 1; }
if [ -n "${1:-}" ]; then
    base_sha=$(g -C "$top" rev-parse --verify --quiet "$1^{commit}") || { echo "FAIL mutants-batch: $1 is not a commit" >&2; exit 1; }
else
    for trunk in main origin/main; do
        g -C "$top" rev-parse --verify --quiet "$trunk^{commit}" >/dev/null && break
        trunk=
    done
    [ -n "$trunk" ] || { echo "FAIL mutants-batch: no base given and no main to take the merge-base with" >&2; exit 1; }
    base_sha=$(g -C "$top" merge-base "$trunk" "$head_sha") || { echo "FAIL mutants-batch: no merge-base of $trunk and $head" >&2; exit 1; }
fi
scope="${base_sha:0:7}..${head_sha:0:7}"

logs=$top/target/logs/mutants-batch
mkdir -p "$top/target" "$logs"
exec 8> "$top/target/.mutants-batch.lock"
flock 8

wt=$(dirname "$top")/.$(basename "$top").mutants-batch
inner=$(dirname "$wt")/.$(basename "$wt").staged
work=$(mktemp -d)
runlog=$work/run.log
pidfile=$work/session
: > "$runlog"

stop_session() {
    local sid
    sid=$(cat "$pidfile" 2>/dev/null) || sid=
    [ -n "$sid" ] || return 0
    # TERM first: the gate's own traps reap the cargo-mutants session they made.
    pkill -TERM -s "$sid" 2>/dev/null || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do pgrep -s "$sid" >/dev/null 2>&1 || break; sleep 1; done
    pkill -KILL -s "$sid" 2>/dev/null || true
    # What the gate started in sessions of its own: whatever runs in, or was
    # started with a path of, the throwaway trees.
    local proc cwd
    for proc in /proc/[0-9]*; do
        cwd=$(readlink "$proc/cwd" 2>/dev/null) || continue
        case "$cwd/" in "$wt"/* | "$inner"/*) kill -KILL "${proc#/proc/}" 2>/dev/null || true ;; esac
    done
    pkill -KILL -f -- "$wt" 2>/dev/null || true
    pkill -KILL -f -- "$inner" 2>/dev/null || true
}
remove_tree() {
    local t
    for t in "$inner" "$wt"; do
        [ -e "$t" ] || continue
        g -C "$t" reset --hard -q 2>/dev/null || true
        g -C "$t" clean -fdq -e target 2>/dev/null || true
        g -C "$top" worktree remove "$t" >/dev/null 2>&1 \
            || g -C "$top" worktree remove --force "$t" >/dev/null 2>&1 || rm -rf "$t"
    done
    g -C "$top" worktree prune
}
cleanup() {
    trap '' INT TERM HUP
    stop_session
    remove_tree
    rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 130' INT TERM HUP

remove_tree
g -C "$top" worktree add --quiet --detach --no-checkout "$wt" "$base_sha"
g -C "$wt" read-tree -u --reset HEAD
g -C "$top" diff --binary --full-index --no-color --no-ext-diff --no-textconv --no-renames \
    --src-prefix=a/ --dst-prefix=b/ "$base_sha" "$head_sha" > "$work/batch.patch"
if [ -s "$work/batch.patch" ]; then
    g -C "$wt" apply --index --binary --whitespace=nowarn "$work/batch.patch"
fi

start=$(date +%s)
setsid nohup bash -c 'echo $$ > "$1"; cd "$2"; shift 2; "$@"; rc=$?; echo "exit $rc"' \
    mutants-batch "$pidfile" "$wt" "${gate[@]}" >> "$runlog" 2>&1 8>&- 9>&- < /dev/null &

rc=
timed_out=0
while :; do
    if line=$(grep -x 'exit [0-9]*' "$runlog" | tail -1) && [ -n "$line" ]; then
        rc=${line#exit }
        break
    fi
    sid=$(cat "$pidfile" 2>/dev/null || true)
    if [ -n "$sid" ] && ! kill -0 "$sid" 2>/dev/null && ! grep -qx 'exit [0-9]*' "$runlog"; then
        rc=1
        echo "the run died before writing its exit line" >> "$runlog"
        break
    fi
    if [ $(($(date +%s) - start)) -ge "$timeout" ]; then
        timed_out=1
        rc=124
        break
    fi
    sleep "$poll"
done
secs=$(($(date +%s) - start))

# Keep the run's logs; the worktree they were written in is about to go.
rm -rf "$logs"
mkdir -p "$logs"
cp "$runlog" "$logs/run.log"
[ -f "$wt/target/logs/mutants-staged.log" ] && cp "$wt/target/logs/mutants-staged.log" "$logs/mutants-staged.log"
[ -d "$wt/target/logs/mutants-staged" ] && cp -r "$wt/target/logs/mutants-staged" "$logs/mutants-staged"
log_rel=target/logs/mutants-batch/mutants-staged.log
[ -f "$logs/mutants-staged.log" ] || log_rel=target/logs/mutants-batch/run.log

tiers=
if [ -f "$logs/mutants-staged.log" ]; then
    tiers=$(sed -n '/kill rate by tier:/,$p' "$logs/mutants-staged.log" | grep -E '^ +(pass|FAIL) ' || true)
fi

if [ "$rc" -eq 0 ]; then
    printf 'ok   mutants-batch (%s) %d.0s\n' "$scope" "$secs"
    if [ -n "$tiers" ]; then
        printf '%s\n' "$tiers"
    else
        grep -E '^(ok  |mutants-staged: no mutants)' "$runlog" | tail -1 | sed 's/^/  /' || true
    fi
    exit 0
fi

if [ "$timed_out" -eq 1 ]; then
    summary="timed out after ${timeout}s"
elif [ -s "$logs/mutants-staged/verdict.txt" ]; then
    summary=$(head -1 "$logs/mutants-staged/verdict.txt")
else
    summary="mutants-staged failed (exit $rc)"
fi
printf 'FAIL mutants-batch: %s (full log: %s)\n' "$summary" "$log_rel" >&2
[ -n "$tiers" ] && printf '%s\n' "$tiers" >&2
missed=$logs/mutants-staged/mutants.out/missed.txt
if [ -s "$missed" ]; then
    n=$(grep -c . "$missed")
    echo "  surviving mutants ($n, first $((n < cap ? n : cap)) shown):" >&2
    head -n "$cap" "$missed" | sed 's/^ */    /' >&2
    [ "$n" -gt "$cap" ] && echo "    ... and $((n - cap)) more in target/logs/mutants-batch/mutants-staged/mutants.out/missed.txt" >&2
fi
[ "$rc" -eq 124 ] && exit 124
exit "$rc"
