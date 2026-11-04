## Isolation: specify both axes

Creating, configuring, or delegating to a ctxloom agent (`ctxloom agent
create`, `ctxloom agent edit`, `run --agent`, `agent_run`)? Set both axes
explicitly — never rely on the default:

- **runtime** (`host` | `container-rootless` | `container-rootful`,
  the agent binding's `runtime:`) isolates the PROCESS. There is
  deliberately no "any container" value: rootless and rootful differ
  in UID mapping, so a workload can genuinely require one.
- **workspace** (`none`|`worktree`, per-invocation `--workspace` /
  `agent_run`'s `workspace`) isolates the FILES.

An ownership mismatch is FATAL, never a substitution. Asking for
`container-rootful` where only rootless is reachable is a fatal
KindIsolation finding (exit 3), not a quiet downgrade to the other
mode — and `--degraded` falls back to the HOST, never to the other
ownership mode.

They're independent: `container-rootless` can still mount the
workspace at the SAME absolute path as the live project (process
isolated, edits still land where the editor already looks); `worktree`
still runs the engine on the host (the editor goes blind to that tree
by design — results return via the delegated-agent merge flow, not
live edits). Picking one says nothing about the other.

Unspecified means `host`+`none` — isolated on NEITHER axis. That's a
default, not a decision. Host runtime is not a security boundary
between agents: the coordinator credential is readable in the process
environment of any same-uid process, and that token IS identity, so a
host-runtime agent can read another host-runtime agent's credential and
speak as that agent. Containers are the actual boundary: they make
isolation a property of the runtime, not a request to the engine —
some vendor CLIs ignore env-var isolation hints and write
credentials/state to a global path regardless.

A bad or missing agent name silently degrades to `host`+`none` with only
a stderr warning, discarding the runtime and permissions you asked for —
confirm the name resolves before trusting the isolation you requested.
