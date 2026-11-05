#!/usr/bin/env bash
# The mechanical part of closing out a batch: `just closeout [base]`.
# build, lint, lint-deps, test, lint-os, then mutants-batch over base..HEAD
# (base defaults to the merge-base with main). Every gate runs, so one report
# shows everything that is red; each prints its own terse line (a pass is one
# `ok` line, a failure its FAIL line and extract), then a summary line. Exits
# non-zero when any gate failed.
#
# Environment: CLOSEOUT_GATES (space-separated recipe names; default the six
# above; the self-test's seam) and the mutants-batch variables.
set -uo pipefail
top=$(git rev-parse --show-toplevel)
base=${1:-}
read -ra gates <<< "${CLOSEOUT_GATES:-build lint lint-deps test lint-os mutants-batch}"
start=$(date +%s)
failed=()
for gate in "${gates[@]}"; do
    args=()
    [ "$gate" = mutants-batch ] && [ -n "$base" ] && args=("$base")
    if ! just --justfile "$top/justfile" "$gate" "${args[@]}" 2>&1; then
        failed+=("$gate")
    fi
done
secs=$(($(date +%s) - start))
if [ "${#failed[@]}" -eq 0 ]; then
    printf 'ok   closeout (%d gates) %ds\n' "${#gates[@]}" "$secs"
    exit 0
fi
printf 'FAIL closeout: %d of %d gates failed: %s (logs: target/logs/)\n' "${#failed[@]}" "${#gates[@]}" "${failed[*]}" >&2
exit 1
