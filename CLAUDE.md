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

`crates/core/clippy.toml` holds the core's purity rules (no I/O, clock, env,
threads or hash-ordered collections) and reaches every core crate, because
clippy reads the nearest `clippy.toml`. A crate's own `clippy.toml` would
replace it, so none has one. It repeats the root `clippy.toml`'s settings,
and `just lint` refuses to run if they disagree.

## Architecture: hexagonal around CQRS/ES (checked in review)

Dependencies point inward; a review refuses a change that breaks these:

- The core depends on nothing outside it. A domain or read model links only
  annals, annals-diagnostic, prost and angzarr-proto: no I/O, async runtime
  or clock in decide, apply or a projection.
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
never generated output.

## Stores are held to annals' store contract

A store adapter runs `annals::store_contract!` (sync) or
`annals::async_store_contract!` (async) in its tests: the same checks
`InMemoryStore` passes. Domain and projector tests run through annals'
kernel over `InMemoryStore`, as the daemon runs over SQLite.

## Errors are localizable: a code, args, and an en-US message

Every error and rejection implements `annals_diagnostic::Diagnostic` with an
English `Display` fallback. A new variant needs a stable code (prefixed with
the crate's name), an en-US message in `locales/en-US/<crate>.ftl` using
exactly its args, and a sample in the crate's `diagnostic_samples()`. The
crate's catalog test checks messages and samples both ways.

## Use `just`

`just test`, `just lint` (clippy, warnings denied) and `just lint-deps`
(cargo-deny) are the checks; lefthook's pre-commit runs fmt, gitleaks,
clippy and cargo-deny. Mutation testing (`cargo mutants`) holds new logic to
its tier: domain and read models 85% or more, adapters 50% or more.
