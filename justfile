TOP := `git rev-parse --show-toplevel`

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
build:
    cargo build --manifest-path {{TOP}}/Cargo.toml --workspace

test:
    cargo test --manifest-path {{TOP}}/Cargo.toml --workspace

# Clippy, warnings denied. Clippy reads only the nearest clippy.toml, so the
# core's repeats the root's nesting threshold; refuse to lint if they differ.
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    root=$(grep '^excessive-nesting-threshold' {{TOP}}/clippy.toml)
    core=$(grep '^excessive-nesting-threshold' {{TOP}}/crates/core/clippy.toml)
    if [ "$root" != "$core" ]; then
        echo "clippy.toml ($root) and crates/core/clippy.toml ($core) disagree on nesting" >&2
        exit 1
    fi
    cargo clippy --manifest-path {{TOP}}/Cargo.toml --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --manifest-path {{TOP}}/Cargo.toml --all

fmt-check:
    cargo fmt --manifest-path {{TOP}}/Cargo.toml --all -- --check

# Dependency policy: licenses, sources, bans and RustSec advisories (deny.toml).
# Fetches the advisory database, so it needs the network.
lint-deps:
    cargo deny --manifest-path {{TOP}}/Cargo.toml check

# Scan the STAGED changes for secrets: lefthook's pre-commit step. Rules and
# allowlists are .gitleaks.toml; output is redacted. A missing binary fails
# rather than skipping: a scan that passes by not running looked at nothing.
secrets-scan-staged:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v gitleaks >/dev/null 2>&1; then
        echo "gitleaks is not on PATH; the secret scan cannot pass by not running." >&2
        exit 1
    fi
    if ! gitleaks git --pre-commit --staged --redact --verbose --no-banner --config {{TOP}}/.gitleaks.toml {{TOP}}; then
        echo "gitleaks found a secret in the staged changes (redacted above)." >&2
        echo "A real credential: unstage it and rotate it. A test fixture: end its line with 'gitleaks:allow'." >&2
        exit 1
    fi

# Test what the commit changes: lefthook's pre-commit step. Runs the tests of
# every crate with a staged change and of the workspace crates depending on
# them (a change can break a dependent's tests); a staged workspace-wide file
# (`workspace_wide`) tests the whole workspace. It judges the STAGED tree,
# built by `_staged-tree`, so unstaged edits cannot pass or fail it.
test-staged:
    #!/usr/bin/env bash
    set -euo pipefail
    read -ra wide <<< "{{workspace_wide}}"
    while IFS= read -r -d '' path; do
        [[ $path == crates/* ]] && exec just --justfile {{TOP}}/justfile _staged-tree _test-affected
        for pattern in "${wide[@]}"; do
            [[ $path == $pattern ]] && exec just --justfile {{TOP}}/justfile _staged-tree _test-affected
        done
    done < <(git -C {{TOP}} diff --cached --name-only --no-renames -z HEAD)
    echo "test-staged: nothing tested: no staged change is in a crate or a workspace-wide file."

# Inside the staged tree: map the staged paths to crates through cargo
# metadata, close over workspace dependents, and test them.
_test-affected:
    #!/usr/bin/env bash
    set -euo pipefail
    root={{TOP}}
    read -ra wide <<< "{{workspace_wide}}"
    meta=$(cargo metadata --format-version 1 --no-deps --offline --manifest-path "$root/Cargo.toml")
    declare -A dir_of=() selected=()
    while IFS=$'\t' read -r name dir; do dir_of[$name]=$dir; done < <(jq -r --arg root "$root/" \
        '.packages[] | "\(.name)\t\(.manifest_path | ltrimstr($root) | rtrimstr("/Cargo.toml"))"' <<< "$meta")
    whole=
    while IFS= read -r -d '' path; do
        for pattern in "${wide[@]}"; do [[ $path == $pattern ]] && whole="$path"; done
        owner=
        for name in "${!dir_of[@]}"; do
            [[ $path == "${dir_of[$name]}"/* ]] && owner=$name
        done
        if [ -n "$owner" ]; then
            selected[$owner]=1
        elif [[ $path == crates/* && ( $path == *.rs || $path == */Cargo.toml ) ]]; then
            # Crate source no current crate owns: a crate moved or removed.
            whole="$path"
        fi
    done < <(git diff --cached --name-only --no-renames -z HEAD)
    if [ -n "$whole" ]; then
        echo "test-staged: $whole is staged, so the whole workspace is tested."
        exec cargo test --manifest-path "$root/Cargo.toml" --workspace
    fi
    if [ "${#selected[@]}" -eq 0 ]; then
        echo "test-staged: nothing tested: the staged change is in no crate's sources or manifest."
        exit 0
    fi
    changed=$(printf '%s ' "${!selected[@]}")
    # Dependents, transitively: every workspace crate depending (normal, dev or
    # build) on a selected one is selected too, until nothing changes.
    mapfile -t edges < <(jq -r '[.packages[].name] as $ws | .packages[] as $p
        | $p.dependencies[] | select(.path != null and (.name as $n | any($ws[]; . == $n)))
        | "\(.name)\t\($p.name)"' <<< "$meta")
    grew=1
    while [ -n "$grew" ]; do
        grew=
        for edge in "${edges[@]}"; do
            IFS=$'\t' read -r dep dependent <<< "$edge"
            if [ -n "${selected[$dep]:-}" ] && [ -z "${selected[$dependent]:-}" ]; then
                selected[$dependent]=1
                grew=1
            fi
        done
    done
    packages=()
    for name in "${!selected[@]}"; do packages+=(--package "$name"); done
    echo "test-staged: changed: ${changed% }; testing with dependents: ${!selected[*]}"
    exec cargo test --manifest-path "$root/Cargo.toml" "${packages[@]}"

# Mutation-test what the commit changes: lefthook's pre-commit step, last
# because it is the slowest. `cargo mutants --in-diff` over the staged diff,
# in the STAGED tree (`_staged-tree`), judged by `mutation_thresholds`; the
# commit is refused with the surviving mutants listed. Config (the wire
# exclusion) is .cargo/mutants.toml. A missing cargo-mutants (`cargo install
# cargo-mutants --locked`) fails rather than skipping: a mutation run that
# passes by not running tested nothing.
mutants-staged:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! cargo mutants --version >/dev/null 2>&1; then
        echo "cargo-mutants is not installed; the mutation gate cannot pass by not running." >&2
        echo "Install it: cargo install cargo-mutants --locked" >&2
        exit 1
    fi
    if [ -z "$(git -C {{TOP}} diff --cached --name-only HEAD -- 'crates/*.rs' ':(exclude)*.test.rs')" ]; then
        echo "mutants-staged: no mutants: the staged change touches no Rust source under crates/."
        exit 0
    fi
    exec just --justfile {{TOP}}/justfile _staged-tree _mutants-affected

# Inside the staged tree: mutate the staged diff in place (the tree is
# throwaway) and judge each tier's kill rate. Killed = caught or timed out (a
# mutant the tests could not finish under changed behaviour); unviable mutants
# (did not build) count neither way.
_mutants-affected:
    #!/usr/bin/env bash
    set -euo pipefail
    root={{TOP}}
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    git diff --cached --no-color --no-ext-diff --no-renames --src-prefix=a/ --dst-prefix=b/ HEAD -- '*.rs' > "$work/staged.patch"
    rc=0
    cargo mutants --dir "$root" --in-place --in-diff "$work/staged.patch" --output "$work" --colors never || rc=$?
    case "$rc" in
        0|2|3) ;;
        4)
            echo "mutants-staged: the staged tree's tests fail unmutated (cargo-mutants baseline); fix them first." >&2
            exit 1 ;;
        *)
            echo "mutants-staged: cargo mutants failed before a verdict (exit $rc)." >&2
            exit 1 ;;
    esac
    outcomes="$work/mutants.out/outcomes.json"
    judged='[.outcomes[] | select(.scenario | type == "object") | {name: .scenario.Mutant.name, file: .scenario.Mutant.file, summary}]'
    if [ ! -f "$outcomes" ] || [ "$(jq "$judged | length" "$outcomes")" -eq 0 ]; then
        echo "mutants-staged: no mutants: the staged diff changes no mutable code."
        exit 0
    fi
    read -ra tiers <<< "{{mutation_thresholds}}"
    # Each mutant's tier: the longest threshold prefix its file is under.
    tiered=$(jq --arg tiers "${tiers[*]}" "$judged"' | ($tiers | split(" ") | map(split("=") | {prefix: .[0], min: (.[1] | tonumber)})) as $t
        | map(. as $m | . + {tier: ([$t[] | . as $tier | select($m.file | startswith($tier.prefix + "/"))] | max_by(.prefix | length))})' "$outcomes")
    untiered=$(jq -r '.[] | select(.tier == null) | .file' <<< "$tiered" | sort -u)
    if [ -n "$untiered" ]; then
        echo "mutants-staged: no threshold in mutation_thresholds (justfile) for:" >&2
        echo "$untiered" >&2
        exit 1
    fi
    report=$(jq -r 'group_by(.tier.prefix)[] | (map(select(.summary == "CaughtMutant" or .summary == "Timeout")) | length) as $killed
        | (map(select(.summary == "MissedMutant")) | length) as $missed | ($killed + $missed) as $viable
        | .[0].tier as $tier | (if $viable == 0 then 100 else ($killed * 100 / $viable | floor) end) as $rate
        | "\(if $killed * 100 >= $tier.min * $viable then "pass" else "FAIL" end)  \($tier.prefix): \($killed)/\($viable) killed (\($rate)%), threshold \($tier.min)%"
            + (map(select(.summary == "Unviable")) | length | if . > 0 then "; \(.) unviable, not counted" else "" end)
            + (map(select(.summary == "Timeout")) | length | if . > 0 then "; \(.) timed out, counted killed" else "" end)' <<< "$tiered")
    echo "mutants-staged: kill rate by tier:"
    sed 's/^/  /' <<< "$report"
    survivors=$(jq -r '.[] | select(.summary == "MissedMutant") | .name' <<< "$tiered")
    if [ -n "$survivors" ]; then
        echo "mutants-staged: surviving mutants (no test failed with the change applied):"
        sed 's/^/  /' <<< "$survivors"
    fi
    if grep -q '^FAIL' <<< "$report"; then
        echo "mutants-staged: a tier is below its kill-rate threshold: add or sharpen the test that would" >&2
        echo "kill each survivor above, stage it, and commit again." >&2
        exit 1
    fi

# Run a recipe of the STAGED tree: HEAD checked out into a throwaway git
# worktree with `git diff --cached` applied on top, so the recipe sees what the
# commit will contain and nothing else (no unstaged edit, no untracked file),
# and nothing here writes the working tree, the index or the stash. Taken from
# Go ctxloom's scripts/hook-mutation-staged.
#
# The worktree is a SIBLING of this checkout, not under target/: the root
# Cargo.toml reaches annals and angzarr-build by relative path (../../), which
# resolves the same only from a directory at the same depth. Its name is
# fixed and the run holds a lock, so builds reuse one target dir
# (target/staged) across commits.
[positional-arguments]
_staged-tree +recipe:
    #!/usr/bin/env bash
    set -euo pipefail
    top={{TOP}}
    # git exports GIT_INDEX_FILE (and, under some commits, GIT_DIR and friends)
    # into a hook. Reading the staged change needs it: under `git commit -a` it
    # is git's temporary index, holding exactly what will be committed. Every
    # git touching the throwaway worktree must not inherit it, or it writes the
    # real index. git names its own repository-locating variables.
    scrub=(env)
    while IFS= read -r var; do scrub+=(-u "$var"); done < <(git rev-parse --local-env-vars)
    mkdir -p "$top/target"
    exec 9> "$top/target/.staged-tree.lock"
    flock 9
    tree="$(dirname "$top")/.$(basename "$top").staged"
    scratch=$(mktemp -d)
    remove_tree() {
        if [ -e "$tree" ]; then
            "${scrub[@]}" git -C "$top" worktree remove --force "$tree" >/dev/null 2>&1 || rm -rf "$tree"
        fi
        "${scrub[@]}" git -C "$top" worktree prune
    }
    trap 'remove_tree; rm -rf "$scratch"' EXIT
    remove_tree
    git diff --cached --binary --full-index --no-color --no-ext-diff --no-textconv \
        --no-renames --src-prefix=a/ --dst-prefix=b/ HEAD > "$scratch/staged.patch"
    # --no-checkout, then read-tree: a checkout would run post-checkout hooks
    # for a tree nobody works in.
    "${scrub[@]}" git -C "$top" worktree add --quiet --detach --no-checkout "$tree" HEAD
    "${scrub[@]}" git -C "$tree" read-tree -u --reset HEAD
    # --index, so the recipe's `git diff --cached HEAD` is the staged change.
    if [ -s "$scratch/staged.patch" ]; then
        "${scrub[@]}" git -C "$tree" apply --index --binary --whitespace=nowarn "$scratch/staged.patch"
    fi
    cd "$tree"
    "${scrub[@]}" "CARGO_TARGET_DIR=$top/target/staged" just --justfile "$tree/justfile" "$@"

# Wire lefthook's git hooks into .git/hooks.
hooks-install:
    lefthook install
