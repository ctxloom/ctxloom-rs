#!/usr/bin/env bash
# Rewrite the fixtures that hold what Go taskloom's `manage` does to an
# agent's MCP configuration, by running Go's own binary on each case:
#
#   crates/adapters/driven/agent-configs/fixtures/go-manage/<case>/
#       before.json        the configuration before (authored; absent for
#                          a case with no file)
#       scope              "project" for the project scope (.mcp.json under
#                          --dir); absent for the user's (~/.claude.json)
#       install.json       the file after `manage install` (absent if none)
#       install.out        `manage install`'s exit code, then its stderr
#       uninstall.json     the file after `manage uninstall` run on
#                          install.json (or before.json when the install
#                          failed) in a home with no application records:
#                          what Go does without the memory Rust lacks
#       uninstall.out      its exit code and stderr
#       record.yaml        the application record Go's install left
#                          (absent if none), its target as $TARGET
#       recorded.json      the file after `manage uninstall` run in the
#                          install's own home, that record there
#       check.out          `manage check` on before.json: exit code, stdout
#
# Every run is in a fresh temporary HOME, with a PATH holding no taskloom,
# so nothing real is read or written. Paths are written as $HOME and $DIR,
# a corrupt file's backup stamp as <unix>. The Rust adapter's parity tests
# hold its output to these byte for byte. GO_CTXLOOM names the Go checkout
# (default ../../ctxloom/ctxloom/main from here).
set -euo pipefail
shopt -s dotglob

top=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
go_checkout=${GO_CTXLOOM:-$top/../../ctxloom/ctxloom/main}
fixtures=$top/crates/adapters/driven/agent-configs/fixtures/go-manage

if [ ! -d "$go_checkout/cmd/taskloom" ]; then
    echo "go-manage-fixtures: no Go ctxloom checkout at $go_checkout (set GO_CTXLOOM)" >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
(cd "$go_checkout" && go build -o "$work/taskloom" ./cmd/taskloom)
mkdir "$work/bin"

# go_manage <home> <dir> <args...>: Go's taskloom in <home>, run in <dir>.
go_manage() {
    local home=$1 dir=$2
    shift 2
    (cd "$dir" && env -i HOME="$home" PATH="$work/bin:/usr/bin:/bin" "$work/taskloom" manage "$@")
}

# normalize <home> <dir>: the paths and stamps of one run, made portable.
normalize() {
    sed -e "s#$1#\$HOME#g" -e "s#$2#\$DIR#g" -e 's#\.corrupt-[0-9][0-9]*#.corrupt-<unix>#g'
}

# fresh: a new home and project directory; prints "<home> <dir>".
fresh() {
    local t
    t=$(mktemp -d "$work/case.XXXXXX")
    mkdir "$t/home" "$t/dir"
    echo "$t/home $t/dir"
}

for case_dir in "$fixtures"/*/; do
    case_dir=${case_dir%/}
    rm -f "$case_dir"/record.yaml "$case_dir"/install.json "$case_dir"/install.out "$case_dir"/uninstall.json \
        "$case_dir"/uninstall.out "$case_dir"/recorded.json "$case_dir"/check.out
    scope=(); config=.claude.json; where=home
    if [ -f "$case_dir/scope" ]; then
        scope=(--project --dir .); config=.mcp.json; where=dir
    fi
    engine=()
    [ -f "$case_dir/before.json" ] || engine=(--engine claude-code)

    read -r home dir < <(fresh)
    target=$home/$config; [ "$where" = dir ] && target=$dir/$config
    [ -f "$case_dir/before.json" ] && cp "$case_dir/before.json" "$target"
    code=0; go_manage "$home" "$dir" install "${engine[@]}" "${scope[@]}" 2> "$work/err" || code=$?
    { echo "$code"; normalize "$home" "$dir" < "$work/err"; } > "$case_dir/install.out"
    installed=$case_dir/before.json
    if [ "$code" = 0 ] && [ -f "$target" ]; then
        cp "$target" "$case_dir/install.json"
        installed=$case_dir/install.json
        for record in "$home"/.ctxloom/records/taskloom/*.hew-record.yaml; do
            [ -f "$record" ] || continue
            sed -e "s#^  - target: .*#  - target: \$TARGET#" "$record" | normalize "$home" "$dir" > "$case_dir/record.yaml"
        done
        go_manage "$home" "$dir" uninstall "${engine[@]}" "${scope[@]}" 2> /dev/null
        [ -f "$target" ] && cp "$target" "$case_dir/recorded.json"
    fi

    if [ -f "$installed" ]; then
        read -r home dir < <(fresh)
        target=$home/$config; [ "$where" = dir ] && target=$dir/$config
        cp "$installed" "$target"
        code=0; go_manage "$home" "$dir" uninstall "${engine[@]}" "${scope[@]}" 2> "$work/err" || code=$?
        { echo "$code"; normalize "$home" "$dir" < "$work/err"; } > "$case_dir/uninstall.out"
        [ -f "$target" ] && cp "$target" "$case_dir/uninstall.json"
    fi

    read -r home dir < <(fresh)
    target=$home/$config; [ "$where" = dir ] && target=$dir/$config
    [ -f "$case_dir/before.json" ] && cp "$case_dir/before.json" "$target"
    code=0; go_manage "$home" "$dir" check --dir . > "$work/out" 2>&1 || code=$?
    { echo "$code"; normalize "$home" "$dir" < "$work/out"; } > "$case_dir/check.out"
done
echo "go-manage-fixtures: wrote $(ls -d "$fixtures"/*/ | wc -l) cases under $fixtures"
