---
tags:
  - domain
  - architecture
---
# ctxloom-rs: architecture vocabulary

ctxloom-rs is the Rust rewrite of ctxloom: hexagonal architecture around
CQRS/event sourcing, built on **annals** (event-sourcing kernel, store
contract, `annals_diagnostic`) and **angzarr** (`angzarr-proto` wire types,
`angzarr-build` code generation from `proto/`).

- **Ring / role**: every crate lives at `crates/<ring>/<role>/<crate>`; the
  path is the architecture. Rings: `core`, `adapters`, `root`.
- `core/wire`: generated prost types. `core/domain`: **aggregates**
  (decide, apply), the write side. `core/read-model`: **projectors**, the
  query side. `core/ports`: traits the core needs. `core/application`:
  command and query handlers.
- `adapters/driven`: port implementations (stores, clock, git).
  `adapters/driving`: CLI, MCP, gRPC — translation only. `root/bin`: the
  composition root.
- **Aggregate vs. read model**: entities are aggregates; anything listed is a
  projector's read model. The write side never reads a projection.
- **Store**: `annals-store-sql` (SQLite), one database per project at
  `~/.ctxloom/projects/<project-id>/state.db`; held to annals' store contract.
- **Diagnostic**: every error carries a stable crate-prefixed code, args and an
  en-US Fluent message.
- **Tasks domain**: the first domain ported; tasks can be deferred with a
  revive condition (`deferred_since`), listed by the task-list projector.
