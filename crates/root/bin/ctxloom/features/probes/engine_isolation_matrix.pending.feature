# Ported from probes/engine_isolation_matrix.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   engine_isolation_matrix.feature with its steps (gherkin_lint.test.rs).

@live
@pending
Feature: Claude × runtime × workspace matrix — the simplest round trip, on every axis pairing

  Before anyone can trust what ctxloom does with an engine, one question has to
  be answerable for every axis pairing we ship: does a run come back with the
  answer at all? Not "did the flag parse", not "did the container launch", not
  "was the credential isolated" — did the engine we launched, in the sandbox we
  put it in, with the context we composed, return the thing it was asked for.

  CLAUDE IS THE ENGINE THIS FEATURE DRIVES NOW. The matrix is built on two axes
  that apply to any engine ctxloom ships, and claude is the one currently wired
  active on all six combinations of them. Wire another engine in the same way
  claude is wired: give it an Examples block per combination of the two axes,
  and a registry cell recording what each is expected to do.

  THE TWO AXES, NAMED PLAINLY, because the table below crosses them and a
  reader should not have to already know which is which to read it. `runtime`
  (host | container-rootless | container-rootful) is the CONTAINERIZATION axis
  — it isolates the PROCESS the engine runs in. `workspace` (none | worktree)
  is the ISOLATION axis — it isolates the FILES the engine sees. They are
  INDEPENDENT: a container run can still mount the workspace at the exact
  absolute path the live project lives at, and a worktree run still executes
  the engine on the host, outside any container — picking one says nothing
  about the other.

  RUNTIME'S THREE VALUES, NOT TWO. `container` was never a fourth thing beside
  `host` — it split into `container-rootless` and `container-rootful` because
  they differ in the property this whole floor cares about: who owns the
  container runtime daemon, and therefore which uid a run's writes land as.
  That is a DIMENSION of the containerization axis, not a second axis and not
  a separate suite — there is deliberately no bare `container` value here, and
  every container row below names its ownership explicitly. A host running
  both dimensions of a suite would be unusual; a host running both ownership
  modes of a container daemon is rarer still — a reachable rootless docker
  daemon means rootful docker is NOT reachable on that same box, and vice
  versa (containercell.probeDocker asserts the exclusivity from one `docker
  info`) — so the matrix is asymmetric by ENVIRONMENT, never by design: each
  runner covers what it has, and no single runner is expected to cover every
  cell below.

  Both column names stay `runtime` and `workspace` throughout this suite: they
  are the product's own spelling, matching config.yaml and the CLI flags.

  Nothing else in this suite answers that. j002300_cross_engine_delegation.feature
  proves DELEGATION, and only on the host axis with workspace none.
  isolation_probe.feature proves the ISOLATION GUARANTEES — what a run wrote and
  where, and whether a credential leaked — and deliberately asserts nothing
  about what the engine said. j002200_isolation.feature and
  j002400_container.feature prove the axis MACHINERY against the mock, with no
  vendor engine involved at all. This feature is the floor underneath all three,
  and it is written to be run cell by cell, unattended, whenever an engine or an
  isolation path changes.

  THE TASK IS THE SIMPLEST ONE THAT CAN STILL FAIL HONESTLY. Each cell plants a
  random nonce in the agent's OWN composed profile context, then asks for one
  JSON object carrying it — JSON only, no preamble, no postamble, no code
  fences. The nonce is random per run, so no memorised "hello world" answer can
  pass; it lives in the CONTEXT rather than the prompt, so a cell also proves
  ctxloom's context delivery survived that isolation scheme.

  THE ASSERTION IS STRICT ON PURPOSE. stdout (whitespace-trimmed, nothing else
  stripped) must parse as JSON and equal {"hello": "<the nonce>"} exactly. An
  engine that wraps its answer in ```json fences, or opens with "Here is your
  JSON:", goes RED. That red is a FINDING about that engine in that cell — the
  precise thing this matrix exists to produce — and it must never be absorbed by
  loosening the matcher. Anything that has to be tolerated gets its own tagged
  examples block and its own measured evidence, where a reader can see it.
  The failure message names WHICH shape it found: a run that failed outright, a
  silent no-op (exit 0, empty stdout), an output-FORMAT failure, a
  CONTEXT-DELIVERY failure (well-formed JSON, no nonce), or a wrong shape.

  EACH CELL USES PRODUCTION'S OWN CREDENTIAL MECHANISM, NOT A HARNESS
  SUBSTITUTE. We drive real engines on real subscriptions, and every axis
  already has a solved mechanism: the host axis uses the engine's real home;
  the worktree axis seeds per-agent homes from the engine's declared seed; the
  container axis mounts the host credential store (claude's read-write, so a
  refresh lands in the live chain — merge 07072acf), identically under either
  ownership mode. Those mechanisms all resolve from the real host home, so
  these cells deliberately do NOT isolate it, and seed nothing themselves: a
  cell that substituted its own credential delivery would prove nothing about
  the product, and a cell more cautious than the product is not a test of it.
  What this floor asserts on stays isolated — a fresh temp project holds the
  fixture, and the assertion reads only stdout.

  THE CONTAINER CELLS WENT GREEN ON 2026-08-13, AND THAT IS WHAT THEY WERE FOR.
  Agent containerization had never been demonstrably correct here — the
  container-auth lane sat red for fifteen days — so these rows were added red ON
  PURPOSE, as the map the fix would be measured against. Container auth keying
  then landed, and the cells were run serially, passing on BOTH container axes
  against real engines, credentials
  arriving through the real-home read-write mount.

  THAT EVIDENCE IS NOW ATTRIBUTED TO container-rootless SPECIFICALLY, not
  reproved. The runtime axis split into container-rootless/container-rootful on
  2026-08-17, after every measurement above was taken; nothing about the axis
  rename changed what actually ran. Every box this suite has ever run on has
  had a reachable rootless docker daemon and no reachable rootful one (see the
  header paragraph above on why a box has at most one), so the runtime that
  answered these eight cells was necessarily rootless. The container-rootful
  rows added alongside them below are the SAME cells this section describes,
  under the ownership this suite has never yet been able to reach — wired, and
  waiting on a runner that actually has a rootful daemon (a privileged docker
  restart, a dedicated CI job/VM, or DinD) to become live-verified in their own
  right rather than inherit this paragraph's evidence by assumption.

  CLAUDE'S FULL FOUR-CELL FLOOR WAS RE-VERIFIED ON 2026-08-16, RIGHT AFTER THIS
  MATRIX NARROWED TO CLAUDE-ONLY ACTIVE ROWS. All four claude cells were run on
  the subscription lane and all four PASSED: host/none and host/worktree each
  ~5.5s, container/none 57s (including the image build), container/worktree
  8s. Narrowing the matrix to claude did not just restructure the table — it
  re-measured every cell claude carries. Read "container/none" and
  "container/worktree" in that sentence as container-rootless, per the
  reattribution above — the axis had no rootful/rootless split yet on
  2026-08-16.

  The cells stay exactly as strict as they were. A red here is now a
  REGRESSION rather than an expectation, which is the whole point of having run
  them while they failed. Do not tag them green-by-loosening, and do not delete
  them because they fail.

  ADDRESSING ONE CELL. Every Examples block carries its engine, its runtime axis
  and its workspace axis as tags — the idiom isolation_probe.feature
  established — so `just engine-matrix <engine> <runtime> <workspace>` runs
  exactly one cell (runtime is host, container-rootless or container-rootful).
  That is the unit of work here: these are real, paid engine calls, a container
  cell can be minutes long, and running the matrix in parallel on a loaded box
  is how a machine gets OOM-killed mid-measurement.

  # Each cell self-skips LOUDLY, naming the engine and BOTH axes, and every
  # remaining skip names something PRODUCTION cannot do — never something this
  # harness declined to arrange. A container-rootful
  # row that skips because THIS BOX has no rootful daemon reachable is a
  # DIFFERENT kind of blank — an ENVIRONMENT limitation, not a production one —
  # and CTXLOOM_REQUIRE_RUNTIMES is what turns that blank into a hard failure on
  # a runner that is supposed to have one (see dockergate's own doc).
  # A blank cell in the matrix always has a reason attached.
  Scenario Outline: A <engine> run under runtime <runtime> and workspace <workspace> returns exactly the JSON it was asked for
    Given the engine matrix targets "<engine>" under runtime "<runtime>" and workspace "<workspace>"
    When it runs the JSON hello-world task in one turn
    Then the run's output is exactly the expected JSON object

    # ACTIVE FLOOR — claude only. The host and container-rootless pairs were
    # VERIFIED LIVE 2026-08-16 on the subscription lane: all four cells ran
    # serially, all four PASSED — host/none ~5.5s, host/worktree ~5.5s,
    # container(-rootless)/none 57s (including the image build),
    # container(-rootless)/worktree 8s. See the header for the fuller account.
    @claude-code @host @ws-none
    Examples:
      | engine      | runtime | workspace |
      | claude-code | host    | none      |

    @claude-code @host @ws-worktree
    Examples:
      | engine      | runtime | workspace |
      | claude-code | host    | worktree  |

    @claude-code @container-rootless @ws-none
    Examples:
      | engine      | runtime           | workspace |
      | claude-code | container-rootless | none      |

    @claude-code @container-rootless @ws-worktree
    Examples:
      | engine      | runtime           | workspace |
      | claude-code | container-rootless | worktree  |

    # WIRED, NOT YET LIVE-VERIFIED. Same cell as the container-rootless pair
    # above, opposite ownership — see the header's "THAT EVIDENCE IS NOW
    # ATTRIBUTED" paragraph for why the earlier container evidence does not
    # cover this row. This development box runs docker ROOTLESS only
    # (containercell.probeDocker), so docker-rootful is unreachable here and
    # this row self-skips on it. The cheapest real coverage available without a
    # privileged daemon restart is podman: rootless vs rootful podman is an
    # INVOCATION property (podman vs sudo podman), not a second daemon, and the
    # gate below reads containercell's own probed RootMapsToInvoker to decide
    # whether THIS box's podman actually serves this row — never substituting
    # a rootless answer for a rootful one. Run `just engine-matrix claude-code
    # container-rootful none` on a box that actually has a rootful docker (or a
    # rootful-invoked podman) to move this row to live-verified.
    @claude-code @container-rootful @ws-none
    Examples:
      | engine      | runtime          | workspace |
      | claude-code | container-rootful | none      |

    # WIRED, NOT YET LIVE-VERIFIED — see the note on the container-rootful/none
    # row above; identical reasoning, worktree axis.
    @claude-code @container-rootful @ws-worktree
    Examples:
      | engine      | runtime          | workspace |
      | claude-code | container-rootful | worktree  |

    # OPENCODE, on a FREE hosted model (liveOpencodeModel, opencode/big-pickle):
    # no key, no subscription turn. The host rows are live-verified; the
    # container rows below them are wired, not yet live-verified.
    @opencode @host @ws-none
    Examples:
      | engine   | runtime | workspace |
      | opencode | host    | none      |

    @opencode @host @ws-worktree
    Examples:
      | engine   | runtime | workspace |
      | opencode | host    | worktree  |

    # opencode.Opencode.Container installs the release (installFragment). Both
    # rootless cells are LIVE-VERIFIED (2026-10-13; the evidence is in
    # capability_probe_registry.go). The rootful cells are WIRED, NOT YET
    # LIVE-VERIFIED: they need a reachable rootful daemon.
    # On the live-container lane they skip loudly: its image ships no opencode
    # binary on the harness side (engineAvailable).
    @opencode @container-rootless @ws-none
    Examples:
      | engine   | runtime            | workspace |
      | opencode | container-rootless | none      |

    @opencode @container-rootless @ws-worktree
    Examples:
      | engine   | runtime            | workspace |
      | opencode | container-rootless | worktree  |

    @opencode @container-rootful @ws-none
    Examples:
      | engine   | runtime           | workspace |
      | opencode | container-rootful | none      |

    @opencode @container-rootful @ws-worktree
    Examples:
      | engine   | runtime           | workspace |
      | opencode | container-rootful | worktree  |
