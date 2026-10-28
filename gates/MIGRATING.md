# Migrating ctxloom-rs onto the shared gates

ctxloom-rs (`/home/babbitt/workspace/ctxloom/ctxloom-rs`) carries its own
copy of every gate recipe. This replaces that copy with `gates.just` and
`lefthook.yml` from this repository, keeping only ctxloom-rs's parameters,
its own recipes and its tool configs. annals (branch `precommit-hooks`) is
the worked example: its `justfile` and `lefthook.yml` have the shape
below.

These steps were written against ctxloom-rs at `3ecc90c`. Check its
justfile and lefthook.yml against the lists here before applying: anything
changed in a recipe being deleted since then must be carried into
`gates.just` first, or it is lost.

## 1. The link

This repository is checked out at `/home/babbitt/workspace/gates`, which
is `../../gates` from ctxloom-rs's main checkout. Add `/.gates` to
ctxloom-rs's `.gitignore`.

## 2. justfile: delete these recipes (now imported)

`fmt`, `fmt-check`, `fmt-staged`, `_fmt-index`, `_members`,
`lint-deps`, `secrets-scan-staged`, `test`, `test-staged`,
`_test-affected`, `_tests`, `_require-nextest`, `mutants-staged`,
`_mutants-affected`, `_mutants-judge`, `_staged-tree`, `_run`,
`_extract-cargo`, `_extract-fmt`, `_extract-rustfmt`,
`_extract-tests`, `_extract-deny`, `_extract-secrets`,
`_extract-mutants`, and the comment block above `build` describing the
terse output (gates.just has it). Also delete `hooks-install`; step 4
replaces it.

## 3. justfile: keep these, and add the parameters

Keep `TOP`, `mutation_thresholds` and `workspace_wide` as they are, and
the recipes `build` and `lint` (its nesting-threshold check and clippy).
They call `_run` and the extractors by `just --justfile {{TOP}}/justfile
...`, which now resolve to the imported ones. A recipe of ctxloom-rs must
not name a variable or a recipe dependency from gates.just (`gates_root`,
`(_require-nextest ...)`): without the link the justfile would not parse,
and the bootstrap could not run. Calling an imported recipe inside a body,
as `build` and `lint` do, is fine.

Add, after `TOP`:

```just
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
```

`staged_tree` reproduces the current `"$(dirname "$top")/.$(basename
"$top").staged"`. Every one of these variables must be defined: gates.just
names them all.

## 4. justfile: the bootstrap

Append annals' bootstrap verbatim (it names nothing but `gates_dir`):

```just
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
# cargo-mutants by `cargo install <name> --locked`; jq, flock, setsid, ps
# and pkill.
hooks-install: gates-link
    lefthook install
```

## 5. lefthook.yml

Replace the whole file (its `output` and its `pre-commit` commands are in
this repository's lefthook.yml, with the same names, priorities and `run`
lines; `clippy` still runs ctxloom-rs's own `just lint`) by:

```yaml
# Git hooks, managed by lefthook (`just hooks-install`). The gates are the
# shared gates repository's lefthook.yml, reached through the `.gates` link
# (`just gates-link`); ctxloom-rs's parameters for them are in its justfile.
extends:
  - .gates/lefthook.yml

pre-commit:
  commands:
    gates:
      # lefthook skips an extends file it cannot read, so without this a
      # missing link would let the commit through with no gate run. Its
      # place in the order does not matter: whenever the link is broken it
      # is the only command lefthook has.
      run: just gates-check
```

Keep the comment on `clippy` (the core's purity rules) as a comment on
`lint` in the justfile; the shared file's comment says only that it is the
consumer's `lint`.

## 6. What stays as it is

`deny.toml`, `.gitleaks.toml`, `.cargo/mutants.toml` (its
`exclude_globs` for the generated wire code and `test_workspace = false`),
`clippy.toml` and `crates/core/clippy.toml`.

## 7. What behaves differently

- `fmt-staged` is given the staged `*.rs` files by lefthook
  (`{staged_files}`) instead of listing them, and takes each file's edition
  from its owning package rather than `[workspace.package]` (the same
  edition there today). It names a partially staged file as
  `partially staged: <file> is committed formatted; its working copy is left
  as it was`; its extractor is `_extract-fmt-staged` (was
  `_extract-rustfmt`).
- `fmt` and `fmt-check` format the workspace's own packages
  (`--package` each), never annals through its path dependency.
- lefthook's `output` adds `execution_out`: each gate's one pass line is
  shown, not only failures.
- `mutants-staged` runs `cargo mutants --workspace`; ctxloom-rs's root is
  a virtual manifest, so that is what it mutated already. Its early exit
  looks at any staged `*.rs` but `*.test.rs`, not only under `crates/`.
- `test-staged` builds the staged tree for any staged `*.rs` or
  `Cargo.toml`, any path under a package, or a `workspace_wide` path; an
  unowned `*.rs` or `Cargo.toml` anywhere tests the whole workspace.
- The staged tree gets its own `.gates` link, to the same checkout.

## 8. Install and verify

```sh
just gates-link && lefthook install   # or: just hooks-install
just fmt-check && just lint && just lint-deps && just test
```

Then commit through the hook (never `--no-verify`) and see the six pass
lines and `ok   gates (.gates: /home/babbitt/workspace/gates)`. Check the
refusal once: `rm .gates`, try a commit, and see `FAIL gates: .gates was
missing`; the link is back, so the next commit goes through. Update
ctxloom-rs's CLAUDE.md hook section to say the gates are shared, as annals'
CLAUDE.md does (*The pre-commit gates are shared*).

# Picking up red tests on branches

The gates now allow a commit with failing tests on any branch but the
default one (warned `RED  tests: ... cannot merge into <default> until they
pass`), refuse it on the default branch, and add three hooks keeping red
tests out of it: pre-merge-commit (a merge commit is tested as a commit is),
pre-push (a push to the remote's default branch runs the whole workspace's
tests on the pushed commit) and post-merge (a fast-forward of the default
branch to local work, which no hook can refuse, is tested just after and the
way back named). Nothing in a consumer's justfile changes. Each clone runs,
once:

```sh
lefthook install          # or: just hooks-install
```

lefthook installs a hook only for the hooks its config names when it is run,
and does not notice new ones in an `extends` file, so until then only
pre-commit's new rule applies. With a remote, record its default branch
(`git remote set-head origin --auto`): without origin/HEAD the default
branch is the first of init.defaultBranch, main and master that is a branch.
