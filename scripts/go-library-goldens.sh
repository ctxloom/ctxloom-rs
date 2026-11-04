#!/usr/bin/env bash
# Regenerate the Go-parity goldens for the library read side (S1):
# crates/root/bin/ctxloom/fixtures/golden/<id>.stdout and <id>.exit, from
# Go ctxloom's own output on the same fixtures.
#
#   scripts/go-library-goldens.sh [GO_CTXLOOM_BINARY] [id ...]
#
# The binary is built from Go ctxloom's release/0.7 (860336c4e, which is
# ca14d6046 or later) with a stamp of the shape the binary insists on:
#
#   go build -ldflags "-X github.com/ctxloom/ctxloom/internal/shared/version.Version=v0.7.0-860336c4e-20261103T000000" -o ctxloom ./cmd/ctxloom
#
# Each line of fixtures/parity.tsv is run in a fresh copy of its fixture
# (fixtures/<fixture>/project, with fixtures/<fixture>/home as the home), by
# CTXLOOM_ROOT, with a throwaway home. The project's path in the output is
# written as {PROJECT} and the home's as {HOME}.
#
# ONE normalization beyond the paths: Go always adds ctxloom's own companion
# loadout (the fragment ctxloom+companion:ctxloom#fragments/isolation-axes,
# whose text names the MCP tool agent_run, and the `ctxloom` MCP server).
# Those texts are the owner's (D24) and wait for the owner's rewrite once the
# MCP cut list is known, so this port does not ship them yet: the goldens are
# Go's output with that companion's contribution removed (fixtures/
# strip_self_companion.py says exactly how).
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
fx=$here/crates/root/bin/ctxloom/fixtures
go_bin=${1:-${GO_CTXLOOM:-ctxloom}}
shift || true
only=("$@")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$fx/golden"

declare -A prepared
prepare() { # fixture -> project dir, home dir
    local f=$1
    if [ -z "${prepared[$f]:-}" ]; then
        mkdir -p "$work/$f"
        cp -a "$fx/$f/project" "$work/$f/project"
        if [ -d "$fx/$f/home" ]; then cp -a "$fx/$f/home" "$work/$f/home"; else mkdir -p "$work/$f/home"; fi
        prepared[$f]=1
    fi
}

while IFS=$'\t' read -r id fixture rest; do
    case "$id" in ''|'#'*) continue ;; esac
    if [ ${#only[@]} -gt 0 ]; then
        keep=0; for o in "${only[@]}"; do [ "$o" = "$id" ] && keep=1; done
        [ $keep = 1 ] || continue
    fi
    prepare "$fixture"
    proj=$work/$fixture/project; home=$work/$fixture/home
    envs=(); args=()
    for word in $rest; do
        if [ ${#args[@]} -eq 0 ] && [[ "$word" =~ ^[A-Z_][A-Z0-9_]*= ]]; then envs+=("$word"); else args+=("$word"); fi
    done
    set +e
    out=$(cd "$proj" && env -i PATH="$PATH" HOME="$home" CTXLOOM_ROOT="$proj" CLAUDE_CODE_OAUTH_TOKEN=sk-ant-oat01-fixture "${envs[@]}" "$go_bin" "${args[@]}" 2>/dev/null)
    code=$?
    set -e
    printf '%s' "$out" | python3 "$fx/strip_self_companion.py" "$proj" "$home" "$id" > "$fx/golden/$id.stdout"
    echo "$code" > "$fx/golden/$id.exit"
    echo "ok $id (exit $code)"
done < "$fx/parity.tsv"
