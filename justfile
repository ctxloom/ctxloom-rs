# The pre-commit gates and their recipes (fmt, fmt-check, fmt-staged, test,
# test-staged, mutants-staged, lint-deps, secrets-scan-staged and the `_run`
# machinery and extractors) are gates/gates.just, imported below; this file
# gives them ctxloom-rs's parameters and keeps its own recipes (`build`,
# `lint`, `test-pkg`).
# cargo-deny's duplicate-version warnings stay in target/logs/lint-deps.log,
# not silenced in deny.toml.

TOP := `git rev-parse --show-toplevel`

import 'gates/gates.just'
import 'gates/os-gates.just'

# The staged tree is a sibling of the checkout, not under target/: cargo
# reads .cargo/config.toml from every parent directory, so under the checkout
# it would pick up a local-deps override (.cargo/local-deps.toml) and gate the
# sibling checkouts instead of the commit's pinned dependencies.
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
mutation_thresholds := "crates/core/domain=90 crates/core/read-model=90 crates/core/ports=90 crates/core/application=90 crates/adapters=90 crates/root/lib=0 crates/root/bin=0"

# Staged paths outside any crate that can change what every crate builds or
# tests (bash globs, `*` crossing `/`): one of these staged tests the whole
# workspace.
workspace_wide := "Cargo.toml Cargo.lock rust-toolchain.toml proto/* .cargo/config.toml .cargo/config"

# The cargo arguments `check-windows` (gates/os-gates.just) compiles for a
# Windows target: driven adapters that use the platform seams, without C
# dependencies (those need a mingw C compiler).
windows_packages := "-p content-fs -p config-files -p cache-files -p surface-files -p remote-git -p liveness -p session-files -p deps-files -p editor-process"

# Where this machine's checkout of ctxloom-lib's templates/clippy-disallowed.toml
# is, the canonical OS and filesystem list `lint-clippy-sync` compares
# clippy.toml with. A sibling checkout of the repository by default; set
# CTXLOOM_LIB_TEMPLATE to the file's path elsewhere (a worktree).
clippy_template := env("CTXLOOM_LIB_TEMPLATE", TOP + "/../ctxloom-lib/templates/clippy-disallowed.toml")

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
# (crates/core/clippy.toml), after `lint-os`: lefthook's `clippy` step. Clippy
# reads only the nearest clippy.toml, so the core's repeats the root's nesting
# threshold and every method the root's disallows (the file I/O the
# file-system seam holds); refuse to lint if they differ.
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
    paths() { grep -o 'path = "[^"]*"' "$1" | sort -u; }
    missing=$(comm -23 <(paths {{TOP}}/clippy.toml) <(paths {{TOP}}/crates/core/clippy.toml) | tr '\n' ' ')
    if [ -n "$missing" ]; then
        echo "FAIL lint: crates/core/clippy.toml does not disallow what clippy.toml does: $missing" >&2
        exit 1
    fi
    {{TOP}}/scripts/os-cfg-check.sh
    terse=(-q --message-format=short)
    [ "${VERBOSE:-}" = 1 ] && terse=()
    exec just --justfile {{TOP}}/justfile _run lint workspace cargo \
        cargo clippy "${terse[@]}" --manifest-path {{TOP}}/Cargo.toml --workspace --all-targets -- -D warnings

# Wire lefthook's git hooks into the repository's .git/hooks. Worktrees share
# .git/hooks, so this is once per clone, and again whenever lefthook.yml or
# gates/lefthook.yml names a hook not installed yet. Needs lefthook (1.13.6)
# and gitleaks (v8), both by `go install`; cargo-nextest, cargo-deny and
# cargo-mutants by `cargo install <name> --locked`; jq, flock, setsid, ps
# and pkill.
hooks-install:
    lefthook install

# The tests of the named crates only, by cargo-nextest (the gates' `_tests`):
# the quick loop on one crate. `just test` runs every crate's.
[no-exit-message]
test-pkg +packages: (_require-nextest "test-pkg")
    @just --justfile '{{TOP}}/justfile' _run test-pkg "{{packages}}" tests just --justfile '{{TOP}}/justfile' _tests {{packages}}
