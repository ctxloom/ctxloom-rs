## Isolation: specify both axes

Creating, configuring, or delegating to a ctxloom agent (`ctxloom agent
create`, `ctxloom agent edit`, `run --agent`, `agent_run`)? Set both axes
explicitly; never rely on the default.

- **runtime** (`host` | `container-rootless` | `container-rootful`, the
  binding's `runtime:`) isolates the PROCESS. There is no "any container"
  value: rootless and rootful differ in UID mapping, so a workload can
  genuinely require one.
- **workspace** (`none` | `worktree`, per-invocation `--workspace` /
  `agent_run`'s `workspace`) isolates the FILES.

An ownership mismatch is FATAL, never a substitution: `container-rootful`
where only rootless is reachable is a fatal KindIsolation finding (exit 3).
`--degraded` falls back to the HOST, never to the other ownership mode.

The axes are independent. `container-rootless` can mount the workspace at the
live project's absolute path (process isolated, edits land where the editor
looks); `worktree` still runs the engine on the host (the editor is blind to
that tree by design; results return via the delegated-agent merge flow).

Unspecified is `host`+`none`: isolated on NEITHER axis — a default, not a
decision. Host runtime is no boundary between agents: the coordinator
credential is readable in the environment of any same-uid process, and that
token IS identity, so one host-runtime agent can speak as another. Containers
are the boundary: isolation becomes a property of the runtime, not a request
to the engine — some vendor CLIs ignore env-var isolation hints and write
credentials/state to a global path regardless.

A bad or missing agent name silently degrades to `host`+`none` with only a
stderr warning, discarding the runtime and permissions you asked for: confirm
the name resolves before trusting the isolation you requested.
