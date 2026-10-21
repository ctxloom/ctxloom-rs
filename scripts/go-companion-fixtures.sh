#!/usr/bin/env bash
# Rewrite the fixtures that hold Go taskloom's companion text, from the Go
# source itself:
#
#   crates/adapters/driving/tasks-mcp/fixtures/go/instructions.txt
#       the MCP server's instructions (cmd/taskloom/mcp.go,
#       mcpServerInstructions)
#   crates/adapters/driving/tasks-mcp/fixtures/go/tools.json
#       every tool as Go's tools/list answers it: name, description, input
#       and output schema (cmd/taskloom/mcp_tools.go, registerTaskTools)
#   crates/adapters/driving/tasks-cli/fixtures/go/loadout.yaml
#       the loadout document (cmd/taskloom/loadout.yaml)
#
# The parity tests compare what Rust taskloom serves with these, byte for
# byte, after applying the human's reviewed factual changes
# (fixtures/reviewed.rs in tasks-mcp and tasks-cli), which this leaves alone. Go's own code produces them: scripts/go-companion/
# companion_dump_test.go, laid into the Go checkout's cmd/taskloom with
# `go test -overlay`, so the checkout is not written. GO_CTXLOOM names that
# checkout (default ../../ctxloom/ctxloom/main from here, which resolves
# from the primary checkout and from ~/workspace/worktrees/<name> alike).
set -euo pipefail

top=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
go_checkout=${GO_CTXLOOM:-$top/../../ctxloom/ctxloom/main}
harness=$top/scripts/go-companion/companion_dump_test.go
mcp_fixtures=$top/crates/adapters/driving/tasks-mcp/fixtures/go
cli_fixtures=$top/crates/adapters/driving/tasks-cli/fixtures/go

if [ ! -d "$go_checkout/cmd/taskloom" ]; then
    echo "go-companion-fixtures: no Go ctxloom checkout at $go_checkout (set GO_CTXLOOM)" >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

printf '{"Replace":{"%s/cmd/taskloom/zz_companion_dump_test.go":"%s"}}' \
    "$go_checkout" "$harness" > "$work/overlay.json"
mkdir "$work/out"
(cd "$go_checkout" && GO_COMPANION_OUT=$work/out \
    go test -count=1 -overlay "$work/overlay.json" -run '^TestDumpCompanion$' ./cmd/taskloom/ >"$work/go-test.log") \
    || { cat "$work/go-test.log" >&2; exit 1; }

mkdir -p "$mcp_fixtures" "$cli_fixtures"
cp "$work/out/instructions.txt" "$work/out/capabilities.json" "$work/out/tools.json" \
    "$work/out/resources.json" "$work/out/tag-schema.json" "$work/out/tag-vocabulary.json" \
    "$mcp_fixtures/"
cp "$work/out/loadout.yaml" "$cli_fixtures/"
echo "go-companion-fixtures: wrote $mcp_fixtures and $cli_fixtures from $go_checkout ($(cd "$go_checkout" && git rev-parse --short HEAD))"
