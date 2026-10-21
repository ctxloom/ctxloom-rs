#!/usr/bin/env bash
# Compare `taskloom import` with Go taskloom's fold, task by task.
#
#   scripts/compare-go-import.sh [LOGS]   compare over the Go logs in LOGS
#                                         (default ~/.ctxloom/tasks)
#   scripts/compare-go-import.sh --fixtures
#                                         rewrite the fixtures' Go fold,
#                                         crates/root/bin/taskloom/fixtures/go-fold.json
#
# The logs are only read: they are copied into a temporary directory first,
# and the importer runs in a temporary home (the ignored test
# `test_the_import_agrees_with_gos_fold_of_real_logs` copies them again).
# Go's fold is Go's own code: scripts/go-fold/fold_dump_test.go, laid into
# the Go checkout's internal/shared/tasks with `go test -overlay`, so the
# checkout is not written. GO_CTXLOOM names that checkout (default
# ../../ctxloom/ctxloom/main from here, which resolves from the primary
# checkout and from ~/workspace/worktrees/<name> alike).
set -euo pipefail

top=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
go_checkout=${GO_CTXLOOM:-$top/../../ctxloom/ctxloom/main}
harness=$top/scripts/go-fold/fold_dump_test.go
fixtures=$top/crates/root/bin/taskloom/fixtures

if [ ! -d "$go_checkout/internal/shared/tasks" ]; then
    echo "compare-go-import: no Go ctxloom checkout at $go_checkout (set GO_CTXLOOM)" >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# go_fold LOGS OUT: Go's fold of every log in LOGS, as JSON in OUT.
go_fold() {
    printf '{"Replace":{"%s/internal/shared/tasks/zz_fold_dump_test.go":"%s"}}' \
        "$go_checkout" "$harness" > "$work/overlay.json"
    (cd "$go_checkout" && GO_FOLD_LOGS=$1 GO_FOLD_OUT=$2 \
        go test -count=1 -overlay "$work/overlay.json" -run '^TestDumpFold$' ./internal/shared/tasks/ >/dev/null)
}

if [ "${1:-}" = "--fixtures" ]; then
    go_fold "$fixtures/go-logs" "$fixtures/go-fold.json"
    echo "wrote $fixtures/go-fold.json"
    exit 0
fi

logs=${1:-$HOME/.ctxloom/tasks}
mkdir "$work/logs"
cp "$logs"/*.jsonl "$work/logs/"
chmod -R u+w "$work/logs"
go_fold "$work/logs" "$work/go-fold.json"
TASKLOOM_IMPORT_LOGS=$work/logs TASKLOOM_GO_FOLD=$work/go-fold.json \
    cargo test --quiet --manifest-path "$top/Cargo.toml" -p taskloom --bin taskloom -- \
    --ignored --nocapture --exact tests::import::test_the_import_agrees_with_gos_fold_of_real_logs
