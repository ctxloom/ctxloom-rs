# Introduction

ctxloom-rs is the Rust ctxloom: a port of the Go ctxloom onto angzarr-embedded,
slice by slice, with architect review of each slice as it is ported. The plan
(`ctxloom-rs-port.plan.md`) is the source of the order and the decisions; a
slice starts as red tests, committed red on `port/<slice>`, reviewed, then
implemented green.

The Rust taskloom is a separate repository. ctxloom reaches it as git
dependencies pinned by commit (`harp-names` and the `ports` it implements,
and later `tasks-lib`, linked in process), never by a path and never as a
copy: this repository holds only ctxloom.

# Working rules

## Tests go in `*.test.rs` files beside the code they test

A source file `foo.rs` keeps its tests in `foo.test.rs` in the same
directory, included at the end of `foo.rs`:

```rust
#[cfg(test)]
#[path = "foo.test.rs"]
mod tests;
```

Not inline `mod tests { ... }`, and not a `tests/` directory. The tests stay
beside the code they test, but out of the source file, so reading the code
does not mean reading its tests too.

Acceptance tests drive the real binary: `crates/root/bin/ctxloom/src/*.test.rs`
build it once (with its `test-support` feature, which turns on the hidden
`ctxloom test ...` commands) and run it as a separate process in a temporary
home. The red tests of a slice are committed red on its branch
(`just test-commit` allows that off the default branch, never on it).

## Layout: a crate's directory is its role

Every crate lives at `crates/<ring>/<role>/<crate>`. The ring and role are
the architecture; there is no other table of crates.

- `core/wire`: generated prost types.
- `core/domain`: aggregates (decide, apply): the write side.
- `core/read-model`: projectors: the query side.
- `core/ports`: traits the core needs from outside.
- `core/application`: command and query handlers.
- `adapters/driven`: port implementations (stores, clock, git).
- `adapters/driving`: the CLI (and later gRPC, MCP): translation only.
- `root/lib`: the composition root's wiring as a library (`ctxloom-lib`).
- `root/bin`: the `ctxloom` binary, the composition root.

Today: slice S0's `ctxloom-store-sqlite` (driven), `ctxloom-cli` (driving),
`ctxloom-lib` (wiring) and the `ctxloom` binary; and slice S1's library read
side: `library` (read model: references, bundles, profiles, composite
selection, the premise index, the layered config), `ctxloom-ports` (the
content and config ports), `library-application` (every library query),
`content-fs` and `config-files` (driven), and `library-cli` (driving). The
whole command line is parsed by `ctxloom-cli`; a crate that owns more nouns
joins it through its `Extension` seam (`ctxloom-lib` wires `library-cli`
in).

Divergence from Go: a config whose `schema_version` is newer, older or unreadable is refused (exit 2, D15 strict decode); Go warns and continues.

`crates/core/clippy.toml` holds the core's purity rules (no I/O, clock, env,
threads or hash-ordered collections) and reaches every core crate, because
clippy reads the nearest `clippy.toml`. A crate's own `clippy.toml` would
replace it, so none has one. It repeats the root `clippy.toml`'s settings,
and `just lint` refuses to run if they disagree.

## Architecture: hexagonal around CQRS/ES (checked in review)

Dependencies point inward; a review refuses a change that breaks these:

- The core depends on nothing outside it: no I/O, async runtime or clock in
  decide, apply or a projection.
- A driven adapter sees only ports (and wire types), never a use case. A
  driving adapter sees only the application layer, never a domain's
  internals. Adapters never depend on each other, except that driving
  adapters share angzarr core's `angzarr-presentation`, and a driven adapter
  may use the platform crates (`os`, `file-system`, `file-lock`).
- A domain never depends on a read model; a read model never depends on the
  application layer.
- Entities are aggregates; anything listed is a projector's read model,
  never an aggregate. There is no commit position, fork point or global
  position anywhere (a test greps the sources for them).

## The pins: one of each

The lock holds ONE `angzarr-embedded`, `angzarr-proto`, `angzarr-ports` and
`angzarr-diagnostic` (a test reads Cargo.lock). Every pin in `Cargo.toml` is
a full commit. taskloom's `ports` is built on angzarr core 4d7ef357, so the
angzarr-embedded pin is the rev built on that same core (9bd1d2c); moving to
a newer embedded (and `angzarr-embedded-runner`) waits for taskloom to
re-pin. The platform crates (`os`, `file-system`, `file-lock`) are
angzarr-platform's, and `angzarr-presentation` and `angzarr-diagnostic` are
angzarr core's, all `Apache-2.0 OR BSD-3-Clause`; this workspace is
`AGPL-3.0-only`.

To change a sibling and this workspace together, `cp .cargo/local-deps.toml
.cargo/config.toml` and add a `[patch]` for it: a gitignored override that CI,
a fresh clone and the pre-commit gates never read. Restore Cargo.lock before
committing, and land the change by pushing the sibling and bumping its rev.

## Storage is angzarr-store-sqlite

A project's store is angzarr-store-sqlite's `SqliteStore` (through
angzarr-embedded's `sqlite` feature), opened at
`~/.ctxloom/projects/<project-id>/ctxloom.db` by `ctxloom-store-sqlite`,
beside taskloom's `state.db` and never the same file (D26). All file I/O goes
through the `file-system` seam, so a test under a temporary home touches
nothing else. ctxloom writes no SQL of its own.

## Errors are localizable: a code, args, and an en-US message

Every error implements `angzarr_diagnostic::Diagnostic`: a stable code
prefixed with the crate's name (`code!`), named args, and an en-US message in
`locales/en-US/messages.ftl` whose id is the full code and which uses exactly
the args. An id ending `-fix` is the remedy of the code before it, printed as
`  fix: ...` in text. A third party's text (the OS's, SQLite's) is the arg
`ext_err_txt`, verbatim. Each crate with diagnostics exports `EN_US` (its name
and its `.ftl`) and, under `test-support`, `diagnostic_samples()`; the
binary's `catalog()` lists every crate's `EN_US`.

## The command line

`--format` is `json` or `text` (json off a terminal); yaml, toml and markdown
are refused like any unknown format, exit 2. Errors are `ctxloom: <message>`
with an optional `  fix:` line in text, and the envelope `{code, args,
message}` on stderr in json. Exit codes: 0, 1 failed, 2 refused, 3 fatal
findings. `-h` works at every depth, there is no `help` command, an unknown
verb says "Did you mean" and lists the verbs, `-v` is accepted everywhere
and changes no output.

## Use `just`

`just build`, `just test` (cargo-nextest, required: a missing nextest fails
the gate), `just lint` (clippy, warnings denied) and `just lint-deps`
(cargo-deny) are the checks. Every gate is terse: a pass prints one `ok
<gate> (<scope>) <duration>` line; a failure prints `FAIL <gate>: <summary>
(full log: target/logs/<gate>.log)` and only the relevant extract. Read the
log for the rest; `VERBOSE=1 just <gate>` shows the full output. `just fmt`
and `just fmt-check` cover this workspace's crates only.

## The pre-commit gates are in gates/; the justfile gives their parameters

The gates and their recipes are `gates/gates.just` and
`gates/lefthook.yml`: the justfile imports the one and lefthook.yml extends
the other. The justfile keeps ctxloom-rs's parameters for them
(`mutation_thresholds`, `workspace_wide`, `staged_tree`, `feature_rules`,
`features_prepare`) and its own `build`, `lint` and `test-pkg`; their tool
configs are `deny.toml`, `.gitleaks.toml` and `.cargo/mutants.toml`. `just
hooks-install` installs the hooks, once per clone (every worktree shares
them), and again after a change to the gates names a new hook
(`gates/MIGRATING.md`). They came from the shared gates repository that
taskloom and annals also copied; a change here does not reach them.

lefthook runs the gates one at a time, stopping at the first failure, and
shows each gate's pass line and any failure's extract. Gates 1 to 5 run in
pre-commit; mutation (6) runs in no git hook: it runs by hand while tests
are written and judged. A failing gate is a
finding: fix the cause and commit again; never `--no-verify`, and never
weaken a gate to get through.

1. **fmt** formats the staged `*.rs` files in the index (`just
   fmt-staged`) rather than refusing an unformatted commit. A fully staged
   file's working copy gets the same content; a partially staged file's
   working copy is left exactly as it was, and named (`partially staged:
   <file> is committed formatted; its working copy is left as it was`).
   `just fmt-check` is the check for CI and manual runs.
2. **secrets**: gitleaks over the staged changes (`.gitleaks.toml`). A fake
   secret in a test ends its line with `gitleaks:allow`.
3. **clippy**: `just lint`, including the core's purity rules
   (`crates/core/clippy.toml`).
4. **deps**: `just lint-deps`, cargo-deny (`deny.toml`).
5. **tests**: `just test-staged` tests the changed crates and their
   dependents in the staged tree (HEAD plus the staged diff, a sibling
   worktree of the checkout); a staged `workspace_wide` file tests the whole
   workspace.
6. **mutants** (no hook; by hand): `just mutants-staged` mutation-tests the staged diff
   (`cargo mutants --in-diff`), holding new logic to its tier: every
   hand-written tier (domain, read models, ports, application and adapters)
   90% or more; the composition root is listed but not held (the justfile's
   `mutation_thresholds` has every tier); the generated wire code is
   excluded (`.cargo/mutants.toml`). Each mutant runs only its own crate's
   tests (`test_workspace = false`), under `cargo test`: a dependent's
   tests, the binary's acceptance tests included, kill nothing, so a crate
   kills its own mutants. A mutant judged equivalent or untestable gets
   `#[cfg_attr(test, mutants::skip)]` on its function, with `mutants` a
   dev-dependency (`mutants.workspace = true`, so nothing new is linked)
   and a one-line comment above it saying why; never an exclude in
   `.cargo/mutants.toml` or a lowered threshold. cargo mutants runs in a
   session of its own (`setsid`), and whatever it leaves running (a
   process a test spawned whose stop a mutant broke) is killed and named
   in the gate's log; the verdict does not change. The last run's mutants.out (each
   mutant's log, outcomes.json) is kept in target/logs/mutants-staged/. Red
   tests fail it on every branch (no mutant can be judged against failing
   tests), as does a run that found mutants and tested fewer: a mutation
   run that judged nothing has shown nothing.
