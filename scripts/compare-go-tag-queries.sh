#!/usr/bin/env bash
# Rewrite Go taskloom's answers to the tag-query parity cases:
#
#   scripts/compare-go-tag-queries.sh
#
# reads crates/core/application/tasks-application/fixtures/tag-queries/cases.tsv
# and writes go.tsv beside it: Go's filterTasks (tagma's Go port) over each
# case. tasks-application's parity test holds tagma-core to it. Go's code is
# run as Go's own: scripts/go-tagquery/tagquery_dump_test.go, laid into the
# Go checkout's internal/shared/tasks with `go test -overlay`, so the
# checkout is not written. GO_CTXLOOM names that checkout (default
# ../../ctxloom/ctxloom/main from here, which resolves from the primary
# checkout and from ~/workspace/worktrees/<name> alike).
set -euo pipefail

top=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
go_checkout=${GO_CTXLOOM:-$top/../../ctxloom/ctxloom/main}
harness=$top/scripts/go-tagquery/tagquery_dump_test.go
fixtures=$top/crates/core/application/tasks-application/fixtures/tag-queries

if [ ! -d "$go_checkout/internal/shared/tasks" ]; then
    echo "compare-go-tag-queries: no Go ctxloom checkout at $go_checkout (set GO_CTXLOOM)" >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
printf '{"Replace":{"%s/internal/shared/tasks/zz_tagquery_dump_test.go":"%s"}}' \
    "$go_checkout" "$harness" > "$work/overlay.json"
(cd "$go_checkout" && GO_TAGQUERY_CASES=$fixtures/cases.tsv GO_TAGQUERY_OUT=$fixtures/go.tsv \
    go test -count=1 -overlay "$work/overlay.json" -run '^TestDumpTagQueries$' ./internal/shared/tasks/ >/dev/null)
echo "wrote $fixtures/go.tsv"
