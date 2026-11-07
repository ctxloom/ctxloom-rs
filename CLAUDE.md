# Introduction

ctxloom-rs is the Rust ctxloom: a port of the Go ctxloom onto angzarr-embedded,
one user journey at a time, with architect review of each milestone. The plan
(`ctxloom-rs-port.plan.md`, §10) orders the milestones by journey
(`crates/root/bin/ctxloom/features/journeys/`); each makes its journey's
scenarios pass, with the cli contracts it touches. A milestone starts as red
tests, committed red on `port/<milestone>`, reviewed, then implemented green.

The features are the spec, and are written first: a journey tells a user's
story with the commands they type; a command's contract lives in its
`features/cli/` file. `docs/plans/ctxloom-features.md` ("Specs") and
`docs/reviews/2026-11-06-vocabulary.md` (names) are authoritative, and where
they disagree with older design in the port plan, they win.

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
home. The red tests of a milestone are committed red on its branch
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
- `root/lib`: the composition root's wiring as a library (`ctxloom-wiring`).
- `root/bin`: the `ctxloom` binary, the composition root.

Slice S0 made `ctxloom-store-sqlite` (driven), `ctxloom-cli` (driving),
`ctxloom-wiring` (wiring) and the `ctxloom` binary. Slice S1 (library read side)
adds:

- `core/read-model/library` (references, bundles, profiles, composite
  selection, the premise index, the layered config);
- `core/ports/ctxloom-ports` (the content and config ports);
- `core/application/library-application` (every library query);
- `adapters/driven/{content-fs, config-files}` and `adapters/driving/library-cli`.

The whole command line is parsed by `ctxloom-cli`; a crate that owns more
nouns joins it through its `Extension` seam (`ctxloom-wiring` wires `library-cli`
in).

Divergence from Go: a config whose `schema_version` is newer, older or unreadable is refused (exit 2, D15 strict decode); Go warns and continues. A `non_tty_format` of yaml, toml or markdown is refused (D28).

Slice S2 (library write side) adds, in the S1 crates, a write half: `library::write`
(pure planning: the config section merge with sorted keys, item/envelope/mcp/
skill/profile files, the scaffold, the loadout, the skill zip codec),
`library-application::write` (create, edit and remove, previews unless `--yes`,
export/import, the editor round trips), `ContentRepo::apply`/`claim`,
`UserFiles`, `Scaffold`, `Editor` and `LoadoutProbe` ports, and the driven
crates `editor-process` and `companion-probe` beside the writes of `content-fs`
and `config-files`. Files are the truth: a command plans the files, checks the
tree still reads, then hands the changes to the port; a refused write has
written nothing. A config write replaces the section it owns whole, never
heals a file that does not parse. `loadout --format yaml` is the one yaml
output; its prose is withheld until the owner rules (D24, D8).
Skill zips are written and read by `library::write::archive` over the `zip`
crate (deflate only, default features off) and crc32fast.

Slice S4 (sessions) built the following. Its store design is superseded: a
session's state goes through angzarr-embedded with in-memory storage: the
engine driver's I/O is event streams there, ctxloom reacts to the events, and
the driver writes the session's directory, `.ctxloom/sessions/<harp>/` in the
project (session home, metadata, records, worktree), from them. That directory
is the durable record; there is no project-level session store, claim stream
or index (list and search walk the directory; a
name is unique because it is the directory's name), and nothing is imported
from Go. M1 reshapes it.

- `core/wire/sessions-proto`, from `proto/ctxloom/sessions/v1`;
- `core/domain/sessions` (hand-written markup, as taskloom's `tasks`: the
  session aggregate and the `session-names` claim aggregate);
- `core/read-model/session-index` (the rows, state, title, search, retention);
- `core/ports/session-ports` (a project's store, `Liveness`, a session's
  files, Go's session directories);
- `core/application/sessions-application` (list, search, show, keep, remove,
  bind, the transcripts; Go's `session.yaml` schema 1 to 3 read through
  angzarr-yaml; the importer's types);
- `adapters/driven/{liveness, session-files}` and, behind `ProjectStores`,
  `ctxloom-store-sqlite`;
- `adapters/driving/{session-cli, hook-cli}` (`hook` is hidden: `session-bind`
  and `hud`).

A session's state is its lock's verdict, never its end stamp; its title is the
first prompt of its transcript; `hud` renders and records nothing. The
kernel's retry is `angzarr-embedded-runner`'s (the embedded pin). A read catches
up fresh each time; nothing caches a project's index. Rows and refusals say what was not
done and the command that would, in the message, so the json envelope carries
it too. A crate's catalog is `locales/en-US/messages.ftl`, ids the full code.

Names and ids are fixed by the owner's rulings (2026-11-03), and imported
streams are rooted on them, so they never change:

- a project's namespace is `UUIDv5(URL namespace, "ctxloom:project:" + project
  id)` (`sessions::project_namespace`);
- a name's claim stream root is `UUIDv5(project namespace, harp text)`
  (`sessions::claim_root`, domain `session-names`);
- an imported session's stable id is `UUIDv5(project namespace, harp text)`
  (`sessions_application::imported_id`). Go's `run_ends[]` and `end_reason`
  are NOT imported (they return with S10).

A rename claims the new name, moves the output folder, records the name and
frees the old one; a removed session's name stays claimed. The refusal code is
`sessions-names-taken` (the crate-prefix rule).

Deliberate divergence from Go: `session remove` takes `--even-if-live`, as its
own refusal text already told the user to use.

`crates/core/clippy.toml` holds the core's purity rules (no I/O, clock, env,
threads or hash-ordered collections) and reaches every core crate, because
clippy reads the nearest `clippy.toml`. A crate's own `clippy.toml` would
replace it, so none has one. It repeats the root `clippy.toml`'s settings,
and `just lint` refuses to run if they disagree.

## Architecture: hexagonal around CQRS/ES (checked by `just lint-arch`, and in review)

Dependencies point inward; a review refuses a change that breaks these, and
`just lint-arch` (part of `just lint`) holds every edge between workspace
crates to `arch-rules.toml`: a table of which role (`crates/<ring>/<role>`)
may depend on which, with the known departures listed as exceptions, each with
its reason and a fix (debt; a stale one fails). The lint is ctxloom-lib's
`arch-lint`, installed at the pinned commit into `~/.cache/ctxloom-arch-lint`.
A dev-dependency finding is a warning.

The rules:

- The core depends on nothing outside it: no I/O, async runtime or clock in
  decide, apply or a projection.
  Pure computation crates are allowed: `surface-ports` links serde_json (with
  `arbitrary_precision`, so a number keeps the digits it was written in) for
  its `Json` value's reader and writer.
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
a full commit. The angzarr pins are one line (angzarr-embedded 9db9299,
core dd59566, angzarr-cli b47c357, angzarr-platform 7b448a8); taskloom's
crates (71bfc28) and ctxloom-lib's (c66dd6b) are built on it, so moving one
moves all. The lock holds one of every angzarr crate. The platform crates (`os`, `file-system`, `file-lock`) are
angzarr-platform's, and `angzarr-presentation` and `angzarr-diagnostic` are
angzarr core's, all `Apache-2.0 OR BSD-3-Clause`; this workspace is
`AGPL-3.0-only`.

The driving edge's shared shims (`cli-support`: `Call<G>`, `Failure`, the
notices printers, `report_failure`, the argument readers) are
ctxloom-lib's (`github.com/ctxloom/ctxloom-lib`), a git dependency pinned by
full commit over https. Its angzarr pins are this workspace's, and the lock
holds one `cli-support` (the same test). The application's own global flags
(`library_application::Flags`) ride in the call as `Call<Flags>`'s `globals`;
ctxloom-lib itself knows no ctxloom word (its vocabulary check).

Every adapter that runs a program (`remote-git`, `worktree-git`,
`companion-probe`, `container-runtime`, `taskloom-cli-client`) runs it
through ctxloom-lib's `run-within`: the program under a deadline, its whole
process tree killed when the deadline passes (not only the program: a git
hook or a shell's `sleep` would otherwise live on holding the pipes), its
output bounded per stream, and the wait for that output bounded too
(`drain_grace`, two seconds), so a descendant that outlives a program that
exited cannot hang the caller. The program's environment is exactly the one
the adapter gives; its tests need `run-within`'s `test-support`, which waits
out ETXTBSY. Limits (os::process's): a descendant that calls `setsid` or
`setpgid` escapes the kill, and the program runs in a process group of its
own, so a Ctrl-C at the terminal does not reach it: a caller that must stop
a run on Ctrl-C hears `Os::stop_signal` and ends the tree itself.
All of ctxloom-lib's crates are pinned at one rev (`cli-support`,
`process-env`, `project-dir`, `run-within`), and so is each family above.

## Co-developing a sibling or ctxloom-lib

To change a sibling (angzarr, angzarr-platform, ...) or ctxloom-lib and this
workspace together, `cp .cargo/local-deps.toml .cargo/config.toml`: a
gitignored `[patch]` that CI, a fresh clone and the pre-commit gates never
read. For ctxloom-lib it is
`[patch."https://github.com/ctxloom/ctxloom-lib"]` with
`cli-support = { path = "../ctxloom-lib/crates/adapters/driving/cli-support" }`
(one line per ctxloom-lib crate used; the sample is in local-deps.toml).
Restore Cargo.lock before committing, and land the change by pushing the
sibling (ctxloom-lib's gates and mutation first) and bumping its rev.

## Storage is angzarr-store-sqlite

A project's store is angzarr-store-sqlite's `SqliteStore` (through
angzarr-embedded's `sqlite` feature), opened at
`~/.ctxloom/projects/<project-id>/ctxloom.db` by `ctxloom-store-sqlite` (the
file name, the placement and the port, over ctxloom-lib's `sqlite-store-opener`,
which refuses a store from before the embedded store, `ctxloom.db.pre-embedded`
naming its backup), beside taskloom's `state.db` and never the same file (D26). All file I/O goes
through the `file-system` seam, so a test under a temporary home touches
nothing else. ctxloom writes no SQL of its own.

## A test's fake program is `fixture_program::install_program`

A test that needs a program on a path (an editor, a `git`, a runtime, a
companion) calls ctxloom-lib's `fixture_program::install_program(fs, path,
body)` (a dev-dependency) and runs what it returns. Writing a script with an
executable mode and exec'ing it is racy in a multithreaded test process (a
thread's fork holds a copy of the writer: "text file busy", ETXTBSY); the
program is instead a link to a committed shim that `sh` runs the script
beside it through, so no file the test wrote is exec'd. No crate keeps a
helper of its own for this (a thin wrapper that only shapes the call is fine).
`just lint-fixtures` (`scripts/exec-mode-check.sh`, part of `just lint`) fails
a test line that gives a file an executable mode, unless
`scripts/exec-mode-allow.txt` lists it with why (a mode that is data: the bit
git commits, a listing reports, a PATH search wants). Its self-test is
`scripts/exec-mode-check.selftest.sh` (`just selftest`). The script and the
`gates/fixture-gates.just` recipe are copies of ctxloom-lib's.

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

## Remotes and dependencies (slice S3)

Crates: `deps` and `pin-changes` (core/read-model: addresses, references,
registry, lock, constraints, the pull/upgrade/reconcile decisions, the pin
disclosure), `deps-ports` (`Remotes`, `DepsFiles`, `RemoteLibrary`),
`deps-application`, `remote-git` (the git CLI with `GIT_*` scrubbed, clones
under `.ctxloom/cache/repos`, read from commits only, never a checkout),
`deps-files` (remotes.yaml, lock.yaml and the installed trees under
`.ctxloom/cache/bundles/<segments>/<bundle>/<sha>/`; it stands where the plan
named `content-fs` for these files), `deps-cli`. `ctxloom-wiring`'s `Commands`
composes them with the library: it merges `bundle push|move` into the
library's `bundle` noun and owns `remote` and `deps`.

- Evidence rule: content is removed upstream only on a not-found from a
  repository proven readable this run; an unreachable remote keeps everything.
- Lock schema 3 has no timestamps and rewrites byte-identically at unchanged
  pins; a corrupt lock is refused, never overwritten; an empty lock never
  replaces a populated one. Registry schema 1: unstamped is refused
  ("schema_version 0"), unknown keys survive a rewrite.
- Installed remote bundles join the library's catalog as `<remote>/<bundle>`
  and by canonical reference (`Catalog::origins`).
- Go parity kept: `.git` stays in canonical references; remote profiles may
  name only their own repository. Divergences: the default is cleared when
  its remote is removed (Go keeps it); an unstamped registry is refused;
  `materialize` is `context assemble`.
- Not built (no crate admitted): semver ranges (exact versions only), the
  forge API (`forge-github`, `remote discover`, `bundle push --pr`). The
  withheld red tests for them are `#[ignore]`d in `bundle_remote.test.rs`.
  angzarr-yaml cannot read Go's `? key` form for lock keys over 128 bytes.

## Use `just`

`just --list` shows the public recipes by group. The checks are `just build`,
`just test` (cargo-nextest, required: a missing nextest fails the gate),
`just lint` (clippy, warnings denied), `just lint-deps` (cargo-deny) and
`just lint-os`; `just test-pkg <crate>...` is the quick loop, `just fmt` and
`just fmt-check` cover this workspace's crates only, `just run` runs the
binary, `just clean-target` is the only way to drop `target/`, and `just
selftest` checks the scripts. Raw `cargo build|test|clippy|fmt|deny|clean|
run|mutants`, `nextest`, `rustc`, `lefthook run`, `gitleaks`, and any
`--no-verify` or `LEFTHOOK=`/`SKIP=` skip of the hooks are blocked by ltk
(`.ltk/config.yaml`, checked by `scripts/ltk-rules-check.sh`); the recipe is
named in the block message. Every gate is terse, because its output lands in
an agent's context: a pass prints one `ok <gate> (<scope>) <duration>` line
and nothing else; a failure prints `FAIL <gate>: <summary> (full log:
target/logs/<gate>.log)` and at most 40 lines of extract (only failing tests,
the first 20 diagnostics, survivors capped, each with a `+K more` tail).
`scripts/output-budget.selftest.sh` holds that budget. Read the log for the
rest; `VERBOSE=1 just <gate>` shows the full output.

## The pre-commit gates are in gates/; the justfile gives their parameters

The gates and their recipes are `gates/gates.just` and
`gates/lefthook.yml`: the justfile imports the one and lefthook.yml extends
the other. The justfile keeps ctxloom-rs's parameters for them
(`mutation_thresholds`, `workspace_wide`, `staged_tree`, `feature_rules`,
`features_prepare`) and its own recipes (`build`, `lint`, `test-pkg`, `run`,
`clean-target`, `mutants-batch`, `closeout`, `selftest`); their tool
configs are `deny.toml`, `.gitleaks.toml` and `.cargo/mutants.toml`. `just
hooks-install` installs the hooks, once per clone (every worktree shares
them), and again after a change to the gates names a new hook
(`gates/MIGRATING.md`). They came from the shared gates repository that
taskloom and annals also copied; a change here does not reach them.

lefthook runs the gates one at a time, stopping at the first failure, and
shows each gate's pass line and any failure's extract. Gates 1 to 5 run in
pre-commit (the private recipes `fmt-staged`, `secrets-scan-staged`,
`test-commit`, ...; `just --list` hides them, they run by name). Mutation (6)
runs in no git hook and is not run by hand: the `closeout` skill runs `just
closeout`, whose `mutants-batch` mutation-tests the whole batch (base to HEAD,
the base defaulting to the merge-base with `main`) in a throwaway worktree, in
the background of the recipe's own session, and prints the terse per-tier
verdict. It is REQUIRED before a batch of work reaches the default branch; no
hook enforces it (a fast-forward of `main` runs none), so merging without it
is how 280 survivors reached `main` once (2026-11-04). Nothing runs
cargo-mutants but the `mutants-staged` gate recipe, which `mutants-batch`
drives. A failing gate is a finding: fix the cause and commit again; never
`--no-verify`, and never weaken a gate to get through.

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
6. **mutants** (no hook; at close-out, `just mutants-batch`): `mutants-staged` mutation-tests the staged diff
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
