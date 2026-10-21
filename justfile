# The pre-commit gates and their recipes (fmt, fmt-check, fmt-staged, test,
# test-staged, mutants-staged, lint-deps, secrets-scan-staged and the `_run`
# machinery and extractors) are the shared gates repository's gates.just,
# imported through `.gates`; this file gives them ctxloom-rs's parameters and
# keeps its own recipes (`build`, `lint`). `just gates-link` makes the link.
# cargo-deny's duplicate-version warnings stay in target/logs/lint-deps.log,
# not silenced in deny.toml: one (annals-diagnostic) marks the local annals
# checkout still to be pinned.

TOP := `git rev-parse --show-toplevel`

# Where the shared gates repository is checked out, relative to this
# repository's main checkout (so every worktree of it finds the same one):
# the one place its path is stated.
gates_dir := "../../gates"

import? '.gates/gates.just'

# The staged tree is a sibling of the checkout, not under target/: the root
# Cargo.toml reaches annals and angzarr-build by relative path (../../),
# which resolves the same only from a directory at the same depth.
staged_tree := "../.{name}.staged"

# No feature is turned on by what a commit changes.
feature_rules := ""
features_prepare := ""

# Mutation kill-rate thresholds, percent, by crate path: the one place they
# are stated. A mutant is judged by the longest prefix its file matches; one
# matching none fails the gate, so a new tier gets a threshold before it gets
# code. core/wire is not here: .cargo/mutants.toml excludes it (generated).
# root/bin at 0 reports its survivors and refuses nothing: the composition root
# holds no logic, and the binary's wiring is shown by running it, not by unit
# tests.
mutation_thresholds := "crates/core/domain=90 crates/core/read-model=90 crates/core/ports=90 crates/core/application=90 crates/adapters=90 crates/root/bin=0"

# Staged paths outside any crate that can change what every crate builds or
# tests (bash globs, `*` crossing `/`): one of these staged tests the whole
# workspace.
workspace_wide := "Cargo.toml Cargo.lock rust-toolchain.toml proto/* .cargo/config.toml .cargo/config"

# Build every crate. Needs protoc on PATH and access to github.com (angzarr-proto).
[no-exit-message]
build:
    #!/usr/bin/env bash
    set -euo pipefail
    terse=(-q --message-format=short)
    [ "${VERBOSE:-}" = 1 ] && terse=()
    exec just --justfile {{TOP}}/justfile _run build workspace cargo \
        cargo build "${terse[@]}" --manifest-path {{TOP}}/Cargo.toml --workspace

# Clippy, warnings denied, including the core's purity rules
# (crates/core/clippy.toml): lefthook's `clippy` step. Clippy reads only the
# nearest clippy.toml, so the core's repeats the root's nesting threshold;
# refuse to lint if they differ.
[no-exit-message]
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    root=$(grep '^excessive-nesting-threshold' {{TOP}}/clippy.toml)
    core=$(grep '^excessive-nesting-threshold' {{TOP}}/crates/core/clippy.toml)
    if [ "$root" != "$core" ]; then
        echo "FAIL lint: clippy.toml ($root) and crates/core/clippy.toml ($core) disagree on nesting" >&2
        exit 1
    fi
    terse=(-q --message-format=short)
    [ "${VERBOSE:-}" = 1 ] && terse=()
    exec just --justfile {{TOP}}/justfile _run lint workspace cargo \
        cargo clippy "${terse[@]}" --manifest-path {{TOP}}/Cargo.toml --workspace --all-targets -- -D warnings

# The bootstrap, which must run without the shared gates, so it uses nothing
# from them.

# Link .gates to the shared gates checkout at `gates_dir`, failing with where
# to clone it when it is not there.
[no-exit-message]
gates-link:
    #!/usr/bin/env bash
    set -euo pipefail
    main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
    target=$(realpath -m "$main/{{gates_dir}}")
    if [ ! -f "$target/gates.just" ] || [ ! -f "$target/lefthook.yml" ]; then
        echo "FAIL gates: no shared gates checkout at $target (gates_dir, justfile)." >&2
        echo "Clone the gates repository there: git clone <gates repository> $target" >&2
        exit 1
    fi
    ln -sfn "$target" .gates

# lefthook's own pre-commit command (lefthook.yml): fail while .gates does not reach the
# shared gates, because lefthook then runs no gate at all. A missing link is
# made, and the commit still refused: the gates were not loaded for it.
[no-exit-message]
gates-check:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f .gates/gates.just ] && [ -f .gates/lefthook.yml ]; then
        echo "ok   gates (.gates: $(readlink -f .gates))"
        exit 0
    fi
    just gates-link
    echo "FAIL gates: .gates was missing, so no gate ran; it is linked now: commit again." >&2
    exit 1

# Wire lefthook's git hooks into the repository's .git/hooks, after linking
# the shared gates. Worktrees share .git/hooks, so lefthook install is once
# per clone; `just gates-link` is once per checkout (`gates-check` does it
# when a commit finds the link missing). Needs lefthook (1.13.6) and
# gitleaks (v8), both by `go install`; cargo-nextest, cargo-deny and
# cargo-mutants by `cargo install <name> --locked`; jq and flock.
hooks-install: gates-link
    lefthook install
