# Ported from probes/capability_context_approaches.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   capability_context_approaches.feature with its steps (gherkin_lint.test.rs).

@live @probe-p1-approach-sweep
@pending
Feature: Context-approach sweep — the same task, delivered by each mechanism the engine declares

  ctxloom can hand an agent its context four different ways, and until this
  feature existed it had only ever been observed doing it ONE way per engine:
  whichever way that engine defaults to. `agent.ApproachSystemPrompt` (claude's
  --append-system-prompt-file) was claimed by the descriptors and proven by
  nobody — row 4 of the capability inventory. (Row 5, the hook-carried route,
  was proven here and then retired by onectx with the approach itself: no hook
  carries context any more.)

  This is that gap closed. Each cell runs the JSON hello-world task from
  engine_isolation_matrix.feature — byte-identical prompt, byte-identical
  bundle, a nonce planted in the agent's own composed context — with ONE line
  added to the agent binding:

    surfaces:
      context: <approach>

  so the run pins the delivery mechanism instead of taking the engine's own
  default. Everything else is held constant. A red here is therefore a finding
  about that MECHANISM on that engine, and about nothing else.

  WHAT THESE CELLS CANNOT SEPARATE, LEARNED THE HARD WAY. A nonce-echo cell asks
  a model a question, and a model's answer is downstream of EVERY channel at
  once — so it can never, by itself, attribute one. This probe recorded a false
  finding on exactly that mistake (a green hook cell credited to a hook that
  ctxloom's own stderr said had never been written), and S4's hook-firing probe
  found the same class from the other side by catching an engine answering a
  nonce probe after searching the workspace with rg.

  Two corrections came out of it, and both are load-bearing here:

    - the verdict now reads ctxloom's REPORT, not only the model's answer:
      approachPinHonoured refuses a run that degraded off the pin (a sibling
      check for a hook-pinned cell whose hook was never written retired with
      the hook approach);
    - each cell's registry row states whether it is SIDE-CHANNEL-CONTROLLED —
      whether the nonce bytes were out of reach of a file search. Only claude's
      system-prompt cell is: its delivery writes out of cwd, and its negative
      control was the hook cell beside it (since retired with the hook
      approach), which saw the identical project tree with the identical tools
      and came back with nothing. Every other context
      delivery in the ladder lands IN the working directory (only claude's
      system-prompt route has an out-of-cwd realization), so those cells prove the bytes reached the
      workspace and the model produced them — not which route the model read.

  WHICH APPROACHES EXIST IS THE ENGINE'S ANSWER, NOT OURS. The cells below are
  exactly the (engine, approach) pairs the engines' own `ApproachTable`s
  declare: claude-code carries all three for its context surface. A pair an
  engine does not declare has NO Examples row here — a scenario for a capability
  the engine says it does not have would skip forever and read as coverage — and
  the absence is written down in the probe registry
  (capability_probe_registry.go) as gated-out with the declaring symbol, so it
  can never read as forgotten. The Given step re-asks production the same
  question through operations.ResolveAgentSurfaces before spending a turn, so
  the registry and the engine cannot quietly drift apart.

  THE DEFAULT IS ALREADY COVERED, AND THAT IS WHY THESE CELLS LOOK REDUNDANT
  AND ARE NOT. claude's default at a shared-cwd launch is the system-prompt
  scratch file, so some cells below pin what
  their engine would have chosen anyway. They still earn their turn: what P0
  measured was the engine's default, and a default can move — the pin is what
  makes "this mechanism works" a claim that survives ctxloom changing its mind
  about preference (see approach.go's own note that no single table order serves
  every caller).

  THE FALSE GREEN THIS FEATURE IS BUILT AROUND. A preference ctxloom cannot
  honour is a WARNING, not a failure: the run degrades to the engine's default
  delivery and carries on. That is the right product behaviour and it is lethal
  to a naive probe — the nonce still arrives, the JSON is still perfect, the
  cell still exits 0, and it has proved nothing at all about the approach in its
  own name. So the verdict reads stderr for production's degrade markers BEFORE
  it reads stdout, and a degraded run is a CONTEXT-DELIVERY failure: the context
  came through a channel this cell did not select.

  WHAT THESE CELLS STILL CANNOT SEE. Absence of the degrade warning proves the
  pin survived to the backend's surface selection. It is not a positive sighting
  of which writer ran — no ctxloom surface reports the resolved per-surface
  approach (`run --dry-run` shows the assembled context and the target file, not
  the mechanism). That is stated here rather than smoothed over, and it is recorded as this
  probe's deferred work.

  THE ASSERTION IS P0'S, UNCHANGED AND UNLOOSENED. stdout, whitespace-trimmed
  and nothing else stripped, must parse as JSON and equal {"hello": "<the
  nonce>"} exactly. Fences, a preamble or terminal decoration are RED, and the
  red is the finding. Anything that must be tolerated gets its own tagged
  examples block and its own measured evidence — never a weaker matcher.

  ADDRESSING ONE CELL. Every Examples block carries the probe, the engine, both
  isolation axes and its VARIANT as tags, so
  `ACCEPTANCE_TAGS="@live && @probe-p1-approach-sweep && @claude-code && @host
  && @ws-none && @var-system-prompt"` selects exactly one cell. The variant tag is not
  decoration: two of these cells share an engine and both axes and differ only
  by the mechanism, so without it they are not separately addressable — and the
  minted-harp ledger keys on it too, which is what stops them from being handed
  the same nonce.

  Scenario Outline: A <engine> run under runtime <runtime> and workspace <workspace>, with context pinned to <approach>, returns exactly the JSON it was asked for
    Given the approach sweep targets "<engine>" under runtime "<runtime>" and workspace "<workspace>", pinning context delivery to "<approach>" for cell variant "<variant>"
    When it runs the JSON hello-world task in one turn under the pinned approach
    Then the run's output is exactly the expected JSON object, delivered by the pinned approach

    # claude's system-prompt route: the framed context goes to an out-of-cwd
    # scratch file consumed via --append-system-prompt-file. The only engine
    # that has it (agent.ApproachSystemPrompt is claude-only), and the approach
    # that touches least of the user's environment — no project file, no process
    # of ours, nothing to race.
    #
    # THE LADDER'S ONE SIDE-CHANNEL-CONTROLLED CONTEXT CELL. The delivery puts
    # no nonce bytes in the workspace, and the fixture bytes that ARE in the tree
    # were ruled out by the hook cell that stood beside it until onectx retired
    # the hook approach: same tree, same tools, same prompt, no nonce in the
    # answer. An engine reading the fixture off disk would have passed both.
    @claude-code @host @ws-none @var-system-prompt
    Examples:
      | engine      | runtime | workspace | approach      | variant       |
      | claude-code | host    | none      | system-prompt | system-prompt |

    # THE CONTAINER CELLS, and they ask a question no other probe answers.
    # P0 proves a nonce planted in DEFAULT composed context survives both axes.
    # These ask whether the PINNED system-prompt route does — and that is not
    # the same claim, because this delivery writes an out-of-cwd scratch file
    # (appendFlagDelivery, consumed via --append-system-prompt-file) rather than
    # a file in the tree the container already mounts. If the scratch file is
    # written host-side, a container cell CANNOT see it, and the cell reds as a
    # CONTEXT-DELIVERY failure. That red would be a real product finding about
    # containerized context delivery, not a fixture fault — which is why these
    # rows are worth their turns either way.
    #
    # BOTH workspace values, for the reason P6 measured the hard way: its
    # host/worktree cell failed where both-off and both-on passed. The defect
    # lives where exactly ONE boundary is on, so a container row without its
    # worktree partner rebuilds the blind spot it was added to remove.
    @claude-code @container-rootless @ws-none @var-system-prompt
    Examples:
      | engine      | runtime            | workspace | approach      | variant       |
      | claude-code | container-rootless | none      | system-prompt | system-prompt |

    @claude-code @container-rootless @ws-worktree @var-system-prompt
    Examples:
      | engine      | runtime            | workspace | approach      | variant       |
      | claude-code | container-rootless | worktree  | system-prompt | system-prompt |

    # claude's native-file route (CLAUDE.md marker-merge), on the WORKTREE axis.
    # The axis is the point: ApproachFile into a shared cwd is a race, and an isolated cell is the conversion that makes the same write
    # safe — claude is the one engine with a race-safe shared-cwd realization to
    # convert away from (claude.NewSurfaces' SharedRealization), so this is
    # where the native write is worth measuring.
    @claude-code @host @ws-worktree @var-file-shared
    Examples:
      | engine      | runtime | workspace | approach    | variant            |
      | claude-code | host    | worktree  | file | file-shared |
