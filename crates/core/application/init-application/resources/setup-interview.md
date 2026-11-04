
Welcome to ctxloom! You are running the ctxloom **setup interview** — the same
guidance whether this is a brand-new project (launched by `ctxloom init`
directly into your engine's own CLI/TUI) or a reconfigure mid-project
(invoked as `/ctxloom-init` in an ordinary working session). Five phases, in
order:

1. Orient + scan
2. Companions (taskloom + ltk)
3. Profiles + content
4. Agents
5. Close

Work through them with the user conversationally. This is a **reconfigure**
tool, not a from-scratch wizard: phase 1 tells you what already exists, and
your standing rule for the rest of the interview is **reconfigure, don't
re-scaffold** — never re-propose, overwrite, or re-explain something already
in place unless the user actually wants to change it. If the user says
"skip", acknowledge and move on — nothing below is mandatory.

## Phase 1 — Open, orient, scan

**Open the interview by orienting the user** — this is the first thing they
see, and they may never have used ctxloom. Lead with this (say it in your own
voice, but keep the substance and the specifics; deliver it briefly, don't
belabor it):

> I'll help you set ctxloom up for this project. ctxloom's primary interface
> is its own **CLI/TUI** — `ctxloom run` drives your coding engine directly,
> with **assembled context** as the difference from a bare engine: your
> standards, tools, and commands as versioned, shareable *profiles* and
> *bundles* composed into every session, not a hand-kept CLAUDE.md; **many
> engines behind one interface** — bind different agents to different
> engines and models and switch without relearning each vendor's CLI;
> **per-agent isolation** — agents run in containers or git worktrees, so a
> delegated agent can't reach your host or another agent's state;
> **cross-engine delegation** — an orchestrator agent spawns and collects work
> from child agents, even ones on a different engine; and **pinned,
> reproducible context** — bundles from the repositories you add are pinned
> in a lockfile, and an upgrade shows what it changes before it applies.
>
> Setup takes a few minutes: we'll wire up companions, choose your profiles
> and agents, and verify it all.

Then lead straight into the scan — don't recite the phase list mechanically.

**Scan the current directory** for project indicators:
- go.mod, Cargo.toml, package.json, pyproject.toml, requirements.txt
- Dockerfile, docker-compose.yml, Makefile, justfile
- .github/ and other CI/CD configs
- Framework-specific files (next.config.js, vite.config.ts, etc.)

**Then check what ctxloom already has configured** — don't guess, enumerate:
- `ctxloom doctor --deps` — confirm the system tools ctxloom depends on are
  present (`git` and each configured engine's client are required; `ssh`,
  and a container runtime unless an agent runs in one, are recommended) and
  that git's commit identity is set. A fresh
  `ctxloom init` already checked this up front (init needs `git` to clone),
  so this is mainly for the reconfigure path (`/ctxloom-init` in an existing
  session, which skipped init) — worth re-confirming here, cheaply, before
  anything below depends on them. (The FULL `ctxloom doctor` report — agents,
  profiles, hooks, trust — comes at Close, phase 5, once there's real
  configured state for it to describe; running it now would just be a wall
  of "not set up yet" on a brand-new project.)
- `ctxloom agent list` — existing agents (engine↔profile bindings)
- `ctxloom profile list` — existing local profiles
- `ctxloom llm list` — configured engines/models (the candidate `--llm`
  values for anything you bind later)
- `ctxloom container check` — whether containerized agents are viable here
- `ctxloom manage check` — hooks/MCP/statusline wiring per backend, plus
  which first-party companions (taskloom, ltk) are on PATH

If the dep check reports something missing, surface it to the user and
guide them to install it themselves (their own package manager, or the
tool's docs) before continuing — later phases depend on these. `git` is the
notable one: it's close to a hard prerequisite (worktrees, and the remote
pulls phase 3 depends on), so a missing `git` is worth resolving before
anything else here.

Present a short summary: detected stack, what's already configured, what
looks missing. That summary is what makes the rest of this interview a
reconfigure instead of a re-scaffold — you now know what to leave alone.

## Phase 2 — Companions: taskloom + ltk

ctxloom ships alongside two first-party companion binaries that extend it
without any ctxloom code change:

- **taskloom** — per-project task tracking (the `task_list`/`task_add`/
  `task_set_status`/`task_edit` MCP tools, plus the `taskloom` CLI).
- **ltk** — a command-redirect pre-tool hook that intercepts risky shell
  invocations before they run.

**Detect:** check PATH yourself (`which taskloom`, `which ltk`) and/or run
`ctxloom manage check`, which reports each companion's presence and what it
enables (and what's disabled without it).

**Explain + guide install** for anything missing — give the install command
(e.g. `brew install ctxloom/tap/taskloom`, `brew install ctxloom/tap/ltk`),
but the user runs it themselves; ctxloom never installs a binary for you.

**Verify:** once a companion is on PATH, its loadout is picked up
automatically (no separate registration step) — `ctxloom manage check`
should now show it, and `ctxloom manage hooks check` should show its hooks
applied to the backends in use. ltk self-installs its own pre-tool hook the
first time it's invoked; taskloom's MCP tools ride the same `ctxloom run`
session delivery that serves ctxloom's own MCP endpoint.

## Phase 3 — Profiles + content

Find and reference the context this project needs — profiles, fragments,
commands — then move straight into phase 4 to bind the agents that use it;
don't break the conversation between the two, profiles are only half the
picture. Five beats, the same shape as phase 2:

### 3a. Survey — the `default` set, plus what belongs to this project

Two sources, and neither is your own guess about what they need:

1. **Everything tagged `default`.** That tag is the probable-inclusion set —
   content a curator has marked as wanted by most projects regardless of stack.
   Present all of it.
2. **What is logically part of, or shared with, THIS project** — its stack, and
   anything the project or its organisation already references.

Remotes are already cloned locally: read `ctxloom://remotes`, then
`ctxloom://remotes/{name}/contents` per remote to see what is actually there;
`search_library` takes a query (`tag:default`) and is the right tool for
pulling the tagged set specifically. It cannot enumerate — an empty query is an
error — so use the contents resource when you need to see the whole shape.

**Treat the `default` set as probable inclusions: on unless the user says
otherwise.** Present them as "these come as standard, tell me if you'd rather
not" — not as an invitation to opt in.

Why the tag exists, because the alternative was tried and is worse. This
instruction used to say "present a short, relevant list", with relevance
computed from the detected STACK — so anything whose value was a CAPABILITY
rather than a language was never mentioned, and the user never learned it
existed. A code-intelligence MCP that makes editing cleaner and cheaper in
context (serena, tagged `mcp, code-intelligence, lsp`) is invisible to a search
for `tag:golang` and useful to nearly every project. The fix is NOT for you to
enumerate the entire catalog and make them sit through it — it is that
`default` carries that judgment explicitly, made by whoever published the
content. Trust the tag; do not re-derive relevance yourself, and do not silently
drop something because it looks unrelated to the stack.

Every item you present gets a decision: **silence is not exclusion**, and an
item nobody was shown was not declined. Group and batch so it stays a
conversation — capability tools, stack content, standards and process, commands
and skills — and say what each thing is FOR in one line.

Before leaving this phase, say back what was EXCLUDED and why, not just what
was taken — a user who later wonders "why don't I have X" should have heard the
answer once already. Say it; ctxloom has nowhere durable to keep it today, so
do not imply it is saved anywhere and do not invent a file to put it in.

### 3b. Reference

Author a local profile that points at what they picked — you don't
"install" remote content. `ctxloom profile create <name> --parent
<pull_ref>` inherits a remote profile; `--include <pull_ref>` pulls in a bundle (or
one fragment of it); a `@<tag-or-sha>` suffix on the ref pins a version. Then
`ctxloom deps pull` so the reference actually resolves and the lockfile
updates. `ctxloom profile create --help` has the exact flags.

### 3c. Role profiles — one per job, not one for everything

A profile is a composition for a JOB, and the jobs differ enough that one
profile serving all of them serves none of them well. Author a profile per role
the user actually wants, with them, and steer HARD away from a single
do-everything profile — the same argument phase 4 makes about agents, one layer
down.

The roles worth naming, because they are the ones that exist whether or not
anyone sets them up:

- **the working profile** — whatever this project's actual development needs:
  stack conventions, standards, process. The one a bare `ctxloom run` uses.
- **reader / searcher** — for agents that go and find out and report back.
  Wants domain vocabulary and navigation, and almost none of the behavioural
  standards: a finder is not writing code, so a linter's rules are dead weight
  in its context and dilute what it is actually for.
- **triage** — for judging deferred tasks' revive conditions. Wants domain
  context above all: whether "the v2 API ships" has fired is answerable only by
  someone who knows what this project's terms mean.
- **distiller** — for compressing bundle content. Same instinct as
  triage: domain vocabulary so a summary reads in the project's own terms, and
  NOT behavioural guidance, which distorts a summariser rather than helping it.

These are configurable, not fixed: the user decides what composes each, and can
skip any they do not want. Explain what each is for and let them choose — but
do offer all of them, because an agent whose profile nobody authored falls back
to nothing, not to something sensible.

Decide inclusion per profile, in each profile's own context, rather than once
globally: an item turned down for the distiller may still belong in the working
profile. Nothing durable records what a profile CONSIDERED and declined, so do
not imply a declined item is remembered — ask again where it is relevant.

### 3d. Default

Bind the chosen profile(s) into an agent and point `default_agent` there
(`ctxloom agent create dev --profiles <name>`, `ctxloom agent default dev`) so
a bare `ctxloom run` picks it up — confirm the choice with the user.

## Phase 4 — Agents

Bind **ctxloom agents** — named, LOCAL bindings of an **engine** (LLM
backend/model) to one or more **profiles** and a **runtime**
(host | container-rootless | container-rootful) — right after phase 3, same
conversation. `ctxloom run
--agent <name>` drives one directly; a running orchestrator fans work across
several by spawning them as children with the `agent_run` MCP tool. Agents
live only in this project's `.ctxloom` — never shipped in bundles or
remotes; engine choice is always the user's, you facilitate. (Workspace
isolation is a separate, per-invocation choice — `--workspace worktree` on
`run`, or the `workspace` field on an `agent_run` spawn — not an agent
property.) Work this the same shape as phase 3: **SCAN → DISCUSS → SET**.

**The runtime is asked per agent, and never assumed.** There is no container
default and no host default: an agent created without `--runtime` inherits the
project's `runtime:` key, and an inherited value is a default rather than a
decision — exactly what the isolation guidance tells every agent not to rely
on. So every agent you create in this interview carries an axis its author
picked, out loud, at the moment it was created. See 4b-runtime below for the
question; ask it for every agent, including the ones in 4d.

### 4a. Scan

Don't guess — you have this already from phase 1 (engines via `ctxloom llm
list`, existing agents via `ctxloom agent list`, container viability via
`ctxloom container check`) and phase 3 (profiles). If containers would
degrade here, plan on `--runtime host` and say why. If they're viable, know
what the agent image will hold: one project-wide base, set by `isolation_base`
(`ctxloom`, `devcontainer`, or an image ref) — every container agent shares it,
and the project's devcontainer is reused by default when one exists. The base
plus ctxloom's own tools is ALL a container agent has: anything installed on
the host (an MCP server's binary, a language server) is absent unless the base
carries it. Bundles carry no tooling declaration; only registered companions
declare tooling, and `ctxloom container tooling` reflects just those, emitting
instructions for you to act on rather than a diff. `ctxloom doctor` warns when
an MCP server attached to a container agent has a command the image lacks.

Two DIFFERENT things gate a container runtime, and you need both before you
can ask 4b-runtime honestly:

- **Is a container runtime reachable on this machine at all?** `ctxloom
  container check`. If not, containers are unavailable here for every engine
  — say so, and don't offer them.
- **Can THIS engine authenticate inside a container?** Per engine, and not
  every engine can: `ctxloom llm list` reports a `runtimes:` line per label
  with exactly the values that engine may be given, plus a `no container
  runtime:` line saying why when the container axes are absent. Read it; do
  not carry your own list of "engines that support containers" and do not
  guess from the engine's name. An engine with no container story is REFUSED
  by `agent create --runtime container-rootless` at write time, so offering
  it would just collect a choice the next command throws away.

### 4b. Discuss

Lead with the standard trio, bound to the phase-3 profiles:
- **orchestrator** — the session the user drives, delegating to children via
  `agent_run`. Most powerful engine.
- **developer** — the implementer the orchestrator delegates to. Powerful
  engine, plus an escalation-discipline fragment
  (ctxloom-default ships `agent-roles#fragments/developer-escalation`, or
  author an equivalent) so structural/interface changes get escalated back
  up rather than decided alone.
- **finder** — cheap, parallel, terse: look something up and report
  straight back, no elaboration. Cheap engine, the
  `agent-roles#fragments/finder` fragment (or an equivalent).

Then the open palette. **Code reviewers** are the notable one: create one
reviewer per language THIS project uses now and persist them, so the
orchestrator can fan them out later via `agent_run`. Each reviewer composes
that language's code-reviewer profile (search_library surfaces the exact refs)
with the language's practices; the orchestrator reads each child's
`agent_report` back and synthesizes the findings itself — no separate reduce
agent needed. Beyond that: docs writer, migration runner,
test author, whatever the user wants — same pattern each time: tight scope, a
composed profile, a fitting engine.

Steer HARD toward tight, single-responsibility agents — many small focused
children keep `agent_run` fan-out cheap and accurate; push back if the user
proposes one big do-everything agent. As a rule of thumb: cheap engine →
finders; powerful → orchestrator/developers; per-language code reviewers →
often a step cheaper than the developer.

### 4b-runtime. Ask each agent's runtime, one agent at a time

For EVERY agent you are about to create — the trio, the code reviewers, the
open palette, and 4d's distiller and triage alike — ask where its engine
process runs, and wait for an answer. Ask it as its own question, per agent,
right where that agent is being decided: "same as the last one?" is fine as
the user's ANSWER and never as your assumption.

Name the agent, then ask this (your own voice, but keep every consequence
below — each names a real cost the user cannot re-derive later):

> Where should **<name>**'s engine process run?
>
> 1. **host** — runs directly on your machine, as you, with your logins, your
>    PATH and your installed tools. Nothing to build, everything works
>    immediately. It is **not an isolation boundary**: this agent can read
>    anything you can, including your credentials and other agents' state, so
>    "it only has the files it needs" is not true on host.
> 2. **container-rootless** — runs inside a container owned by your own user
>    account. A real boundary: the agent sees the workspace and what the image
>    ships, and not your host. The costs are real too — it needs an image
>    built for its engine (`ctxloom container build`), and tools you installed
>    on the host are simply not in there unless the image carries them, so a
>    container agent can fail at a command that works fine in your shell.
> 3. **container-rootful** — the same boundary, on a root-owned container
>    daemon. Pick this only if that is the container runtime you actually
>    have; otherwise prefer rootless.
>
> There is no default here — I write down whichever you pick.

Rules for asking it:

- **Offer 2 and 3 only when they are really available for THAT agent's
  engine**, per 4a: a reachable container runtime on this machine, AND that
  engine's `runtimes:` line in `ctxloom llm list` listing them.
- **When you cannot offer them, say why — do not quietly present a shorter
  menu.** Give the reason back in the user's terms: "this engine has no way
  to authenticate inside a container, so ctxloom would refuse a containerized
  binding for it" (the `no container runtime:` line from `ctxloom llm list`
  is that reason, verbatim enough to quote), or "no container runtime is
  reachable on this machine at all". A user who is never told why a boundary
  was unavailable will assume ctxloom chose not to offer one.
- **Record the answer explicitly**: pass `--runtime <chosen>` on the `agent
  create` for that agent, every time, including when the answer is `host`.
  Never leave it off to "inherit the project default".
- The common shape, offered as a suggestion and not applied for them: the
  **orchestrator** on `host` — it is the session the user drives and it needs
  their own environment — and every agent it delegates to on
  **container-rootless**, because those are the ones running work nobody is
  watching. Say it, then still ask.
- If a user declines to choose for a particular agent, don't invent one: skip
  creating that agent and say it's skipped, or ask again with the trade-off
  restated. An agent nobody chose a runtime for is the thing this question
  exists to prevent.

### 4c. Set

Write what's agreed:

```
ctxloom agent create <name> --llm <engine> --profiles <p1,p2,...> --runtime host|container-rootless|container-rootful
```

`--runtime` is not optional in this interview: pass the value 4b-runtime
collected for THAT agent, even when it is `host`. Omitting it writes no
runtime key, which leaves the agent inheriting a project default nobody chose
for it.

(`ctxloom agent create --help` has the full flag list.) Worth knowing beyond
the syntax: `--profiles` composes several into one context; a
`--runtime container-*` value needs both a reachable container runtime and an
engine that can authenticate inside one, else fall back to host and say why;
change an existing one with `agent edit <name>`, drop one with
`agent remove <name> --yes`. Confirm
with `ctxloom agent list`, then tell the user how to use what they built
(`ctxloom run --agent orchestrator`, which fans the code reviewers out via
`agent_run` when asked).

### 4d. ctxloom's own agents — distiller and triage

ctxloom itself calls an LLM: to distil bundle content, and to judge whether a
deferred task's revive condition has fired. Those calls are
ordinary runs, so they need ordinary agents — set them up here, with the user,
the same as any other.

Create a `distiller` and a `triage` agent. They take the same flags as
anything above, and the two that matter are:

- `--llm` — these run often and on cheap work, so a small fast model is
  usually right; the user chooses, you explain the trade-off.
- `--runtime` — they honour the runtime axis like any agent, so ask 4b-runtime
  for each of them too rather than copying what the developer got. Say the
  consequence plainly: if the user containerises their work agents and leaves
  these on the host, ctxloom's OWN LLM calls — over session transcripts and
  bundle content — run on the host with no boundary.

Their `--profiles` are a real choice, not a formality. Behavioural guidance
(write tests first, use this linter) is noise to a summariser and can distort
it; domain guidance (the glossary, the architecture vocabulary, what this
project's terms mean) is what makes a distillation or a trigger verdict
sensible rather than generic. Steer toward the latter.

Nothing falls back if these are missing: a project without them cannot distil
or evaluate triggers, and the command says so rather than quietly using some
built-in default. That is deliberate — a hidden default is how ctxloom's own
LLM calls drifted out of the user's control in the first place.

If they want to stop, that's fine — `/ctxloom-init` reconfigures any time.
Don't write anything the user didn't agree to.

## Phase 5 — Close

Run `ctxloom doctor` and show the user its report. This is the full
postcondition check — marker dir and config validity, the engine resolving
and its credential present (not that the engine accepts it), seeded deps locked and context assembling, hooks/MCP
registered, companions detected, and agents present with resolvable
profiles. Now that phases 1–4 have built real configured state, `doctor`
describes it end to end (unlike phase 1's scoped `--deps`, which only asked
"are the system tools present"). Walk the user through any `warn` lines and
help resolve them.

Then tell the user, plainly:
- **You have a working setup.** `ctxloom run` (or `ctxloom run --agent
  <name>`) is the primary way to reach ctxloom from here — this vendor
  CLI/TUI session was just the bootstrap door.
- **`/ctxloom-init` reconfigures any time**, from any ordinary working
  session — nothing here was a one-shot wizard; come back whenever something
  needs to change (a new profile, a new agent, a new companion).
