#!/usr/bin/env bash
# The ltk rules of .ltk/config.yaml against sample commands: `ltk check` says
# what each would get. Every denied command must name its just recipe in the
# suggestion. Usage: ltk-rules-check.sh [config]   (default .ltk/config.yaml)
set -euo pipefail
top=$(git rev-parse --show-toplevel)
config=${1:-$top/.ltk/config.yaml}
command -v ltk >/dev/null || { echo "FAIL ltk-rules-check: ltk is not on PATH" >&2; exit 1; }
command -v jq >/dev/null || { echo "FAIL ltk-rules-check: jq is not on PATH" >&2; exit 1; }
n=0 bad=0
t() { # allow|deny <command> [text the suggestion must contain]
    local want=$1 cmd=$2 sug=${3:-} out got s
    n=$((n + 1))
    out=$(ltk check --config "$config" --command "$cmd" --format json 2>&1) || { echo "FAIL ltk-rules-check: ltk check failed for '$cmd': $out" >&2; exit 1; }
    got=$(jq -r .decision <<< "$out")
    s=$(jq -r '.suggestion // ""' <<< "$out")
    if [ "$got" != "$want" ]; then
        echo "  wrong: '$cmd' is ${got}ed, want ${want}ed" >&2; bad=$((bad + 1))
    elif [ "$want" = deny ] && [[ $s != *"$sug"* ]]; then
        echo "  wrong: '$cmd' suggests '$s', want it to contain '$sug'" >&2; bad=$((bad + 1))
    fi
}

# Allowed: the recipes, git without a skip, read-only cargo, the sanctioned wrappers.
t allow 'just test'
t allow 'just test-pkg ctxloom-ports'
t allow 'just mutants-batch main'
t allow 'just closeout'
t allow 'just clean-target'
t allow 'git commit -m x'
t allow 'git commit -am "fix n and --no-verify in text"'
t allow 'git commit --amend --no-edit'
t allow 'git push origin queue/x'
t allow 'git status'
t allow 'cargo metadata --format-version 1 --no-deps'
t allow 'cargo tree -p ctxloom-ports'
t allow 'cargo install cargo-nextest --locked'
t allow 'cargo --version'
t allow 'rustc --version'
t allow "setsid nohup bash -c 'just mutants-staged' > /tmp/x.log 2>&1 &"
t allow 'lefthook version'

# Denied: the raw tools, each pointing at its recipe.
t deny 'cargo build --workspace' 'just build'
t deny 'cargo check -p x' 'just build'
t deny 'cargo b' 'just build'
t deny 'cargo test -p x' 'just test'
t deny 'cargo t' 'just test'
t deny 'cargo nextest run' 'just test'
t deny 'cargo-nextest nextest run' 'just test'
t deny 'cargo +nightly test' 'just test'
t deny 'cargo clippy --all-targets' 'just lint'
t deny 'cargo fmt --all' 'just fmt'
t deny 'rustfmt crates/x/src/lib.rs' 'just fmt'
t deny 'rustc crates/x/src/lib.rs' 'just build'
t deny 'cargo deny check' 'just lint-deps'
t deny 'cargo-deny check' 'just lint-deps'
t deny 'cargo clean' 'just clean-target'
t deny 'cargo run -- --help' 'just run'
t deny 'cargo bench' 'ask the owner'
t deny 'cargo doc --open' 'ask the owner'
t deny 'cargo llvm-cov --html' 'ask the owner'
t deny 'lefthook run pre-commit' 'just hooks-install'
t deny 'gitleaks detect' 'just secrets-scan-staged'

# Mutation testing: every raw spelling, including through wrappers.
t deny 'cargo mutants' 'just mutants-staged'
t deny 'cargo mutants --in-diff x.patch' 'just mutants-batch'
t deny 'cargo-mutants mutants --list' 'just mutants-staged'
t deny 'cargo +nightly mutants' 'just mutants-staged'
t deny "bash -c 'cargo mutants --in-diff p'" 'just mutants-staged'
t deny 'setsid nohup cargo mutants --in-place > log 2>&1 &' 'just mutants-staged'
t deny 'env RUST_LOG=x cargo mutants' 'just mutants-staged'
t deny '/home/u/.cargo/bin/cargo-mutants' 'just mutants-staged'

# Skipping the hooks. (ltk cannot see a bare `LEFTHOOK=0 git commit` assignment
# prefix, `export`, or what `sudo` runs; only `env` is unwrapped.)
t deny 'git commit --no-verify -m x' 'without --no-verify'
t deny 'git commit -n -m x' 'without -n'
t deny 'git commit -nm x' 'without -n'
t deny 'git commit -an -m x' 'without -n'
t deny 'git push --no-verify' 'without --no-verify'
t deny 'git merge --no-verify x' 'without --no-verify'
t deny 'git -c core.hooksPath=/dev/null commit -m x' 'hooks-install'
t deny 'git config core.hooksPath /dev/null' 'hooks-install'
t deny 'env LEFTHOOK=0 git commit -m x' 'just'
t deny 'env SKIP=tests git commit -m x' 'just'

if [ "$bad" -ne 0 ]; then
    echo "FAIL ltk-rules-check: $bad of $n samples wrong" >&2
    exit 1
fi
echo "ok   ltk-rules-check ($n samples)"
