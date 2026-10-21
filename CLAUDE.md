# Introduction
Note that this is a port of ctxloom into rust from golang, piece by piece, with architect review of each piece as its ported.

We're starting with the highest risk pieces first and the smallest complete units that can derisk early.

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

## Layout: a crate's directory is its role

Every crate lives at `crates/<ring>/<role>/<crate>`. The ring and role are
the architecture; there is no other table of crates.

- `core/wire`: generated prost types.
- `core/domain`: aggregates (decide, apply): the write side.
- `core/read-model`: projectors: the query side.
- `core/ports`: traits the core needs from outside.
- `core/application`: command and query handlers.
- `adapters/driven`: port implementations (stores, clock, git).
- `adapters/driving`: CLI, MCP, gRPC: translation only.
- `root/bin`: binaries, the composition root.

ctxloom-rs produces several binaries, each a crate under
`root/bin` that composes the same core, application and adapters. Below
`root/bin`, a crate is named for its domain or role, never for a binary.

`crates/core/clippy.toml` holds the core's purity rules (no I/O, clock, env,
threads or hash-ordered collections) and reaches every core crate, because
clippy reads the nearest `clippy.toml`. A crate's own `clippy.toml` would
replace it, so none has one. It repeats the root `clippy.toml`'s settings,
and `just lint` refuses to run if they disagree.

## Architecture: hexagonal around CQRS/ES (checked in review)

Dependencies point inward; a review refuses a change that breaks these:

- The core depends on nothing outside it. A domain or read model links only
  annals, annals-diagnostic, prost, angzarr-proto, and tagma's tagma-core
  (the tag grammar and query engine) and tagma-proto (its `Tag` message): no
  I/O, async runtime or clock in decide, apply or a projection.
- A driven adapter sees only ports (and wire types), never a use case. A
  driving adapter sees only the application layer, never a domain's
  internals. Adapters never depend on each other.
- A domain never depends on a read model (the write side reads no
  projection); a read model never depends on the application layer (a query
  issues no command).
- Entities are aggregates; anything listed is a projector's read model,
  never an aggregate. A UI watches one stream per watch; a list's cursor is
  its projector's own sequence.

## Generated code is never committed

Wire and domain types are generated into `OUT_DIR` by `angzarr-build` from
`proto/`. A core crate's `build.rs` is one call (`angzarr_build::wire` or
`angzarr_build::domain`): angzarr-build reads the build environment itself,
so the core's clippy rules hold in build scripts too. Change the `.proto`,
never generated output. A tag is tagma's message, `tagma.v1.Tag`, imported
from tagma-proto's `PROTO_INCLUDE`; the generated domain layer gives it a
domain `Tag` that converts to `tagma_core::Tag` and spells itself
canonically.

## Storage is annals-store-sql, in annals for now

The SQL store is `annals-store-sql` (feature `sqlite`), in the annals
repository: backend-neutral statements, with each backend's execution and
migrations behind a feature. It lives in annals until angzarr core's older
SQL storage is brought onto it, and then moves to angzarr core. ctxloom
writes no SQL of its own; a project's database is that crate's SQLite store,
opened at `~/.ctxloom/projects/<project-id>/state.db`.

Any store is held to annals' store contract (`annals::store_contract!` or
`annals::async_store_contract!`), the checks `InMemoryStore` passes. Domain
and projector tests run through annals' kernel over `InMemoryStore`, as the
daemon runs over SQLite.

## Errors are localizable: a code, args, and an en-US message

Every error and rejection implements `annals_diagnostic::Diagnostic` with an
English `Display` fallback. A new variant needs a stable code (prefixed with
the crate's name), an en-US message in `locales/en-US/<crate>.ftl` using
exactly its args, and a sample in the crate's `diagnostic_samples()`. The
crate's catalog test checks messages and samples both ways.

Each crate with diagnostics exports its embedded en-US catalog as `EN_US`
(its name and its `.ftl`). The composition root assembles the binary's
catalog from every crate's `EN_US` (presentation's `Catalog`), and the
driving adapters render with it: a diagnostic's catalog message in text,
its `{code, args, message}` envelope in JSON. A new crate's `EN_US` joins
the binary's catalog list, and its samples the binary's every-diagnostic
catalog test.

## Use `just`

`just build`, `just test` (cargo-nextest, required: a missing nextest fails
the gate), `just lint` (clippy, warnings denied) and `just lint-deps`
(cargo-deny) are the checks. Every gate is terse: a pass prints one `ok
<gate> (<scope>) <duration>` line; a failure prints `FAIL <gate>: <summary>
(full log: target/logs/<gate>.log)` and only the relevant extract (failing
tests' output, short clippy messages, the files rustfmt would change, deny's
errors, the mutation verdict and survivors). Read the log for the rest;
`VERBOSE=1 just <gate>` shows the full output. `just fmt` and `just
fmt-check` cover this workspace's crates only, never the path dependencies
(annals, angzarr-build).

## The pre-commit gates are shared; ctxloom-rs gives their parameters

The gates and their recipes live in the shared gates repository
(`gates.just`, `lefthook.yml`), checked out at `gates_dir` (justfile,
`../../gates`) relative to ctxloom-rs's main checkout, so every worktree
finds the same one. The justfile imports `.gates/gates.just` and
lefthook.yml extends `.gates/lefthook.yml`; `.gates` is a gitignored link
made by `just gates-link`, once per checkout (worktrees included). `just
hooks-install` links and installs the hooks, once per clone: every worktree
shares them. A commit without the link is refused by `just gates-check`
(which makes the link, so the next commit goes through); a commit with no
gates checkout at `gates_dir` is refused, saying where to clone it. Change a
gate there, not here; ctxloom-rs keeps only its parameters
(`mutation_thresholds`, `workspace_wide`, `staged_tree`, `feature_rules`,
`features_prepare`), its `build` and `lint`, `deny.toml`, `.gitleaks.toml`
and `.cargo/mutants.toml`.

lefthook runs the gates one at a time, stopping at the first failure, and
shows each gate's pass line and any failure's extract. A failing gate is a
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
6. **mutants**: `just mutants-staged` mutation-tests the staged diff
   (`cargo mutants --in-diff`), holding new logic to its tier: every
   hand-written tier (domain, read models, ports, application and adapters)
   90% or more; the composition root is listed but not held (the justfile's
   `mutation_thresholds` has every tier); the generated wire code is
   excluded (`.cargo/mutants.toml`). A mutant judged equivalent or
   untestable gets `#[mutants::skip]` on its function, with a one-line
   comment above it saying why (in the core, as `#[cfg_attr(test,
   mutants::skip)]` with `mutants` a dev-dependency, so the core links
   nothing new); never an exclude in `.cargo/mutants.toml` or a lowered
   threshold. The last run's mutants.out (each mutant's log, outcomes.json)
   is kept in target/logs/mutants-staged/.
