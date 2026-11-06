# Ported from journeys/j002200_isolation.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j002200_isolation.feature with its steps (gherkin_lint.test.rs).

@doc
@pending
Feature: Bounding what the agent can reach, even with permissions bypassed

  Priya runs an agent with its permission prompts turned off. She is not being
  reckless — approving every file read one dialog at a time is unusable at the
  pace the work actually happens, and her employer's policy is not "prompt the
  human more", it is "the assistant must not be able to reach what it has no
  business reaching." Those two facts together are the whole problem: the
  moment the prompts are off, the engine's own guardrails are not what is
  protecting the rest of her disk. Something else has to be.

  Asking the engine nicely does not work. Vendor CLIs treat an environment
  variable as a suggestion — one of them writes to a global path regardless of
  what HOME says, and this suite asserts that leak as a known fact elsewhere.
  So a boundary an engine has to cooperate with is not a boundary. It has to
  be a property of where the process actually runs.

  ctxloom's answer is two independent axes: WORKSPACE (its own git worktree,
  or the live project directory) and RUNTIME (a host process, or a container).
  This journey is Priya establishing that they are real. That a run REQUESTING
  a worktree lands its engine somewhere genuinely distinct, and two isolated
  runs never share that somewhere — not that the setting round-trips through
  `agent show`, which would prove only that ctxloom can remember a preference.
  And that when she asks for a container and this machine cannot give her one,
  the run ABORTS rather than quietly proceeding unsandboxed. A sandbox that
  silently degrades is worse than none, because she would have stopped.
  Dropping to the host is available, but only when she says --degraded and
  therefore knows.

  # WHAT THIS JOURNEY CAN AND CANNOT SEE (honesty, mirroring j002100_delegation's
  # own note). The built-in mock backend is the only engine tests/acceptance
  # can drive hermetically, and it is a bare echo — it never spawns a real
  # engine binary. Two structural facts about the isolation seam follow from
  # that, discovered by running it, not merely read from source:
  #
  #   1. isolation.Prepare's Worktree policy never os.Chdir's the PLUGIN
  #      subprocess itself (`ctxloom llm serve mock` — see
  #      internal/adapters/isolation/{none,worktree}.go's SpawnClient, both of
  #      which spawn via exec.Command with no Cmd.Dir). A REAL engine honors
  #      the workspace by having ITS OWN Execute spawn a grandchild process
  #      with Cmd.Dir = the resolved WorkDir; the mock never spawns a
  #      grandchild, so it cannot observe isolation via os.Getwd() — that
  #      value is identical across every workspace axis, confirmed live. The
  #      mock now also records req.WorkDir (internal/engines/mock/backend.go),
  #      the value isolation.Prepare actually resolved and threaded through
  #      RunOptions.WorkDir — THAT is the honest signal this journey reads.
  #   2. Per-engine config-home isolation (CLAUDE_CONFIG_DIR) is decided off
  #      the agent binding's engine_home, for every cell alike
  #      (operations.ResolveInTreeAgentHome), and the built-in "mock" backend
  #      declares no relocatable home at all — hermetically true for every
  #      workspace axis. This journey therefore proves the WORKSPACE boundary
  #      itself (distinct worktree checkouts, no escape into the project
  #      tree), not per-engine config-home variable isolation — that needs a
  #      real registered-engine fixture, out of hermetic scope here.
  #
  # How a run in an isolated config-home authenticates is a further,
  # separate claim this journey does not make either way — see (2).
  #
  # UPDATE (isolation-matrix task): (2)'s gap is now filled, below, WITHOUT
  # abandoning the mock's hermetic guarantee. A real registered backend name
  # drives isolation.Prepare
  # exactly as a live run would — but PATH is rebuilt from scratch to a
  # scratch dir plus /usr/bin:/bin, so the literal binary a backend execs
  # resolves ONLY to a
  # recording spy script this suite writes, NEVER to a real installed engine
  # — no live credential, no network call, ever, in any scenario in this
  # file. The spy dumps an allow-listed part of its OWN environment (what a
  # real engine process receives), whether its token var equals the exported
  # fixture token, and any credential file its config home holds (there must
  # be none), captured from INSIDE the spawned process — the per-agent
  # config-home is reaped after the run, so this is the only vantage point
  # from which its contents are observable at all. See j002200_isolation.doc.md for the rendered matrix this proves and does
  # not prove, and steps_j002200_isolation_matrix.go's own package doc for why
  # an engine driven over a stateful handshake rather than a plain oneshot
  # exec gets the fail-loud/warn CONTRACT below but not the exact spawned-env
  # payload — that half stays pinned at the Go level (auth_test.go).
  #
  # The RUNTIME axis's real container LAUNCH boundary needs a live container
  # daemon plus a built agent image; that is out of hermetic scope. What IS
  # hermetically provable — and is proven below — is the fail-loud contract when
  # a requested container can't launch: exit code 3 with a ClassIsolation
  # finding naming the escape hatch, or exit 0 on the host under --degraded.
  #
  # ONE real-launch scenario DOES live here now, tagged @container (excluded
  # from the default run, gated by `just test-acceptance-container`, self-skips
  # where no runtime is reachable): "A containerized engine's write reaches the
  # host through the same read-write bind mount…", below. It demonstrates the
  # SHARED-IDENTITY property of ctxloom's read-write host bind mount — the
  # mechanism a container run's session engine home is mounted with. No
  # credential file is mounted into a container: claude there authenticates
  # with the token, delivered as a secret file.

  Background:
    Given Alice has a git-backed project with a mock agent

  # The workspace axis is not merely a flag that parses — it changes WHERE
  # the run's engine actually executes. "none" (the default) shares the live
  # project dir; "worktree" gives the run a fresh checkout the ephemeral
  # session scratch owns, never under the project tree.
  Scenario Outline: The same run lands in the workspace its axis dictates
    When Alice runs the mock agent under workspace "<workspace>" with prompt "isolation-check"
    Then the run's workdir reflects the "<workspace>" workspace axis

    Examples:
      | workspace |
      | none      |
      | worktree  |

  # The workspace axis is a convenience, and it degrades: a worktree is cut
  # from a git repository, so where there is none the run goes ahead in the
  # live project directory rather than refusing. That is the opposite of the
  # runtime axis below, where a container that cannot be had is a refusal.
  # Losing a worktree loses no boundary that was promised; losing a container
  # would.
  Scenario: Asking for a worktree outside git runs in the live project directory instead of refusing
    Given Alice's project is not a git repository
    When Alice runs the mock agent under workspace "worktree" with prompt "no-git-check"
    Then the run's workdir reflects the "none" workspace axis

  # LOCKED — the core boundary claim: two isolated runs never share the
  # workspace they get. Genuinely different prompts (task-one / task-two) so
  # a one-value assertion can't accidentally pass twice.
  Scenario: Two isolated runs share no workspace between them
    When Alice runs the mock agent under workspace "worktree" with prompt "task-one"
    And Alice runs the mock agent under workspace "worktree" with prompt "task-two"
    Then the two runs' workdirs are distinct and neither is under the project directory

  # Non-escape: a worktree run's per-agent config edits are hidden from the
  # shared project tree via .git/info/exclude (worktree.go's
  # excludeConfigFromMerge), not left as new tracked/untracked files a
  # developer could accidentally merge back.
  Scenario: A worktree run leaves the project tree clean
    When Alice runs the mock agent under workspace "worktree" with prompt "cleanup-check"
    Then none of the per-agent worktree config artifacts appear in the project's git status
    And the shared git exclude file carries the ctxloom per-agent worktree config block

  # LOCKED — the runtime axis's REFUSAL contract: an EXPLICITLY-requested
  # container that cannot launch here is a NON-DEGRADABLE ClassIsolation
  # finding that aborts the run (exit 3) rather than landing unsandboxed on the
  # host. --degraded does NOT downgrade it, and this row is what proves that.
  #
  # INVERTED 2026-09-15 by the degradation audit (obstinate-judiciary). The
  # --degraded row used to expect a warned, working HOST run, and the human
  # ruled that break deliberately: "it BREAKS currently-documented behaviour —
  # an explicitly requested container falling back to the host under
  # --degraded. That break is the point, not a side effect." One flag cannot
  # mean both "I accept a thinner context" and "I accept no sandbox".
  #
  # The two rows now differ only in the FLAG, not the outcome, and that is the
  # assertion: the flag must make no difference here. Deliberately kept as two
  # rows rather than collapsed to one — a single row could not tell "--degraded
  # is ignored" from "--degraded was never passed".
  #
  # WHAT THIS SCENARIO ACTUALLY REACHES: isolation.chainFor, via the
  # NO-REACHABLE-DAEMON reason only. It does NOT reach isolation.prepareChain's
  # downgrade branch, and an earlier version of this comment claimed it did.
  # With no runtime, chainFor never puts a container tier in the chain, so
  # prepareChain's `IsContainerPolicyName(p.Name()) && !IsContainerPolicyName(next)`
  # never evaluates — measured, by mutation: BOTH operands of that condition can
  # be replaced with `true` and this suite stays green, as can `continue` ->
  # `break` on the line below it.
  #
  # So the other reasons named in prepareChain's own comment — absent image,
  # shared-fs probe, unresolvable auth, i.e. a REACHABLE runtime whose
  # PrepareWorkspace fails — are UNCOVERED here. Covering them needs a scenario
  # that reaches a reachable-runtime-but-failing-prepare state; see
  # uninvited-maternity. Do not read this scenario as proof of that path.
  Scenario Outline: Requesting a container with no runtime REFUSES, and --degraded does not rescue it
    When Alice runs the container-bound agent with flags "<flags>"
    Then the run aborts with an isolation finding

    Examples:
      | flags      |
      |            |
      | --degraded |

  # The PREVIEW of the same refusal (ruled 2026-09-27): a dry run does not
  # stop at it. It renders the plan it could compute, lists the finding with
  # its fix after it, and exits exactly as the run would — the same abort,
  # the same gate, whatever the output format. The JSON row reads the finding
  # off the plan's own findings array, with its remedy. Both rows read the
  # probed environment off the plan: runtime unavailable, reach unknown —
  # never the host the refused run does not fall back to.
  Scenario Outline: Previewing a container with no runtime shows the plan, then refuses as the run would
    When Alice previews the container-bound agent with flags "<flags>"
    Then the run aborts with an isolation finding
    And the preview rendered its plan and the refusal's fix

    Examples:
      | flags         |
      |               |
      | --format json |

  # LOCKED — isolation.prepareChain's container-to-host DOWNGRADE: the sibling
  # gate the row above deliberately EXCLUDES. There a container is never
  # selected (no runtime); here one IS selected and then fails to START.
  #
  # Reaching it needs both halves at once, which is why it did not exist before:
  # the runtime must report itself reachable (so chainFor puts a container
  # policy in the chain) AND the image must be unproducible. The DEFAULT image
  # name is content-addressed, so it is absent but BUILDABLE and ctxloom builds
  # it — measured. An isolation_images override is "run AS-IS and never built",
  # so it is absent AND unbuildable, and isolation.Container.ensureImage errors.
  #
  # What this pins is the security half of the contract: prepareChain's
  # `IsContainerPolicyName(p.Name()) && !IsContainerPolicyName(next)` guards the
  # finding that says the session is NOT sandboxed. Both operands of that
  # condition could be replaced with `true` with the suite staying green.
  # INVERTED 2026-09-15 with its sibling above, and for the same ruling. The
  # --degraded row expected a host run; a selected-then-unstartable container
  # is now refused in both modes.
  #
  # It keeps its OWN outcome step ("aborts at the container START gate"), which
  # asserts prepareChain's finding and asserts the runtime-gate finding is
  # ABSENT. Both scenarios now exit 3, so that per-gate wording is the only
  # thing still telling these two failure modes apart — collapsing them onto
  # one shared step would silently let either gate satisfy both rows, which is
  # a confusion this feature has already paid for once.
  Scenario Outline: A container that was selected and cannot start REFUSES, and --degraded does not rescue it
    When Alice runs a container-bound agent whose image cannot be produced, with flags "<flags>"
    Then the run aborts at the container START gate

    Examples:
      | flags      |
      |            |
      | --degraded |

  # The same START gate, reached with BOTH axes requested: Alice wants her own
  # worktree AND a container around it. Here the container tier is the
  # worktree-backed one, and the next tier down is a plain HOST worktree — a
  # real, working workspace. That makes this the one degrade where losing the
  # container still leaves something that looks like isolation, so it is the
  # one most tempting to accept quietly. The row above cannot stand in for it:
  # its chain holds the live-dir container, never the worktree-backed one, so
  # a boundary check that recognised only the live-dir container would pass
  # there and let this run land on the host believing it had a sandbox.
  Scenario Outline: A containerized worktree that cannot start REFUSES rather than keeping the worktree on the host
    When Alice runs a container-bound agent whose image cannot be produced, with flags "--workspace worktree <flags>"
    Then the run aborts at the container START gate

    Examples:
      | flags      |
      |            |
      | --degraded |

  # THE BOUNDARY THAT WAS ACCEPTED AND THEN LOST, which is a different fault
  # from every gate above and the only one that is NOT degradable.
  #
  # The rows above cover a container that could never be BUILT or SELECTED.
  # They too now refuse in both modes (the degradation audit removed the host
  # fallback), so this row is no longer distinguished by BEING fatal under
  # --degraded — it is distinguished by WHERE the boundary is lost: there the
  # container never existed, here it was accepted and then died. Here the
  # runtime is reachable, the ownership
  # matches, the image is present, and the daemon ACCEPTS the run — then the
  # container never reaches running. The session was already told it had a
  # boundary. Falling back now would run on the host something that believes
  # it is sandboxed, so the launch itself is the harm and --degraded must NOT
  # rescue it: both rows below expect exit 3.
  #
  # This is also the only row that exercises the transport-start gate. The
  # finding is raised in startContainerInteractive and gated in startTransport,
  # and DELETING THAT GATE leaves every unit test green — the finding is simply
  # recorded and checked by nothing, so the run warns and exits 1. That is the
  # characteristic defect of this codebase (a green-looking no-op) reproduced
  # inside the fix meant to prevent it, and this row is what kills it.
  #
  # The engine-never-ran assertion is not decoration: an abort that still
  # launched the engine would satisfy an exit-code check alone while doing the
  # exact thing the finding claims to have prevented.
  Scenario Outline: A container accepted by the daemon that never runs is fatal in BOTH modes
    When Alice runs a container-bound agent whose container dies at the daemon, with flags "<flags>"
    Then the run aborts because the container never reached running state

    Examples:
      | flags      |
      |            |
      | --degraded |

  # The same lost boundary, when the daemon SAYS why it dropped the container.
  # The container was started with --rm, so by the time Alice looks, it and
  # its logs are gone; what the daemon wrote while killing it is the only
  # account of what happened, and she is given it with the refusal.
  Scenario: A container the daemon kills before it runs is refused with the daemon's own reason
    When Alice runs a container-bound agent whose container the daemon kills, saying why
    Then the run aborts because the container never reached running state
    And Alice is told the daemon's own reason

  # ===========================================================================
  # THE CONTAINER HOME AXIS — one read-write bind mount, not a copy.
  #
  # A container run's session engine home is a read-write bind of the host
  # directory, so what the engine writes there is on the host. Credentials do
  # not ride it: claude authenticates with the token, delivered as a secret
  # file.
  # The @container lane launches the built-in MOCK in a real container and
  # proves the shared-identity property live: a write made from INSIDE the
  # container reaches the HOST at the same path.
  # ===========================================================================
  @container @image-mock-agent @reach-back @R5
  Scenario: A containerized engine's write reaches the host through a read-write bind mount
    When Alice runs the container-bound agent in a real container
    Then the engine's in-container write is the same file the host holds

  # The worktree axis inside a container: the per-agent checkout the engine
  # works in belongs to the session, so session cleanup can sweep it and a
  # resume can find it again. A checkout parked in the OS temp dir is scratch
  # nothing accounts for.
  @container @image-mock-agent
  Scenario: A containerized worktree run works in a checkout its session owns
    When Alice runs the container-bound agent in a real container, in a worktree
    Then the containerized worktree's checkout lives in the session's own scratch

  # ===========================================================================
  # PER-ENGINE CONFIG-HOME ISOLATION MATRIX — fills the gap the journey's own
  # top note used to flag as out of hermetic scope. Every scenario below
  # drives a REAL registered backend name through isolation.Prepare via a
  # PATH-sandboxed recording spy (see the top-of-file UPDATE note) — never a
  # real engine binary, never a live credential, never a network call. See
  # j002200_isolation.doc.md for the rendered outcome matrix (isolated / LEAKS /
  # not executed) these scenarios and their Go-level siblings together prove.
  # ===========================================================================

  # workspace "none" for ALICE'S OWN SESSION is the baseline every other row in
  # the matrix is measured against: it shares the live project dir AND the
  # engine's shared global config — by design, not a bug — so NONE of the
  # config-home machinery below (auth gating, curated HOME, findings) fires
  # at all. Asserting this explicitly is what lets a later "worktree" cell's
  # finding read as isolation actually engaging, rather than the isolation
  # machinery just always firing regardless of which axis was requested.
  #
  # Alice types `ctxloom run` and names no agent; the project's default agent
  # is bound for her, but the session is hers, and — in this fixture — the
  # bound agent's engine_home is UNDECLARED, which resolves to the real host
  # home by default (engine_home is strictly opt-in: see the sibling
  # "An in-tree AGENT run gets a ctxloom-controlled config home" scenario,
  # which is the positive control for every absence asserted here — it uses
  # the SAME fixture with the ONE addition of an explicit
  # `engine_home: session` declaration on the binding, and gets the opposite
  # outcome). The two must be read together, or "nothing was created"
  # degenerates into "nothing happens in this fixture at all".
  #
  # The baseline is asserted on PAYLOAD, not on the absence of a warning: the
  # engine really runs (exit 0, the spy process leaves its recording), it runs
  # in the LIVE project dir, not one of its config-home variables points into a
  # per-agent scratch tree, and no ctxloom-controlled home was created in the
  # project. An earlier version checked only that two phrases were missing from
  # the output, which an audit showed is equally satisfied by a run in which the
  # engine never launched at all.
  # ADJUSTED, ruled 2026-09-21: engine_home session by default. A run naming
  # no agent gets the session home
  # too — the real home is reached only by a binding's explicit, unsafe
  # `engine_home: host`. The engine still runs in the live project dir; what
  # moved is its config home. The "touches no isolation mechanism" reading
  # of this scenario is now pinned by the explicit-host scenario below.
  Scenario Outline: workspace "none" runs Alice's own bare session in its session home, in the live project dir
    Given Alice has a git-backed project
    And Alice has whatever credentials "<engine>" needs to authenticate
    When Alice runs "<engine>" under workspace "none" as her own session, naming no agent
    Then the run reports no isolation finding
    And the spy "<engine>" process's "<var>" env var points at this session's config-home instance
    And the spy "<engine>" process ran in the live project dir

    Examples:
      | engine      | var               |
      | claude-code | CLAUDE_CONFIG_DIR |

  # THE OTHER HALF, and the positive control the scenario above depends on: the
  # SAME project, the SAME none axis, the SAME engine — but Alice names an
  # agent WHOSE BINDING DECLARES engine_home: session, so the run gets a
  # controlled home instead of hers. A ctxloom agent that opts in this way is
  # not entitled to her memory, plugins, personal MCP registrations, global
  # agents or steering, and must not write its session state into them, so it is
  # given a PER-SESSION config-home INSTANCE under her ctxloom home
  # (`~/.ctxloom/sessions/<harp>/home/<leaf>`) instead — created at session
  # start, holding a generated config and no credential, and disposable.
  #
  # THE DECLARATION RESTATES THE DEFAULT (ruled 2026-09-21): the sibling
  # "undeclared engine_home" scenario below uses this exact fixture MINUS the
  # declaration and proves the SAME outcome; only an explicit `engine_home:
  # host` — the unsafe selection — keeps the real home.
  #
  # This is asserted on PAYLOAD from INSIDE the spawned engine process, not on
  # ctxloom's own say-so: the spy dumps the config-home variable it was really
  # handed.
  Scenario Outline: An in-tree AGENT run gets a per-session config-home instance instead of Alice's own home
    Given Alice has a git-backed project
    And Alice has whatever credentials "<engine>" needs to authenticate
    And Alice's agent declares engine_home "session"
    When Alice runs the isolated "<engine>" agent under workspace "none"
    Then the run reports no isolation finding
    And the spy "<engine>" process's "<var>" env var points at this session's config-home instance
    And Alice's own "<engine>" home directory was never created by the run

    Examples:
      | engine      | var               |
      | claude-code | CLAUDE_CONFIG_DIR |

  # UNDECLARED engine_home — ADJUSTED, ruled 2026-09-21: engine_home session
  # by default. This is the SAME fixture
  # as the scenario above with ONE difference: the "iso" binding never
  # declares engine_home at all — and gets the session home all the same.
  # Against a tree whose parser defaulted to host this scenario fails: the
  # spy would report no CLAUDE_CONFIG_DIR at all.
  Scenario Outline: An in-tree AGENT run with an undeclared engine_home gets the session home too
    Given Alice has a git-backed project
    And Alice has whatever credentials "<engine>" needs to authenticate
    When Alice runs the isolated "<engine>" agent under workspace "none"
    Then the run reports no isolation finding
    And the spy "<engine>" process's "<var>" env var points at this session's config-home instance

    Examples:
      | engine      | var               |
      | claude-code | CLAUDE_CONFIG_DIR |

  # THE UNSAFE SELECTION. engine_home: host is the ONE way a run keeps the
  # real host home (ruled 2026-09-21): a binding that NAMES "host" gets
  # exactly what it asked for, and the launch names the selection unsafe. A
  # bug that ignored a declared "host" value (treating every resolved agent
  # binding as session regardless of what it declared) would make this
  # scenario red while the undeclared one stayed green, since only THIS one
  # exercises the declared-host branch.
  #
  # With no engine home to deliver beneath, claude's MCP servers go to the
  # project's own .mcp.json, which claude loads only for a repository Alice
  # trusted in her own claude — so this run is in a trusted project, and the
  # scenario after it is the untrusted one.
  Scenario Outline: An in-tree AGENT run that declares engine_home: host keeps Alice's own real home
    Given Alice has a git-backed project
    And Alice has whatever credentials "<engine>" needs to authenticate
    And Alice has trusted the project in her own claude
    And Alice's agent declares engine_home "host"
    When Alice runs the isolated "<engine>" agent under workspace "none"
    Then the run reports no isolation finding
    And no ctxloom-controlled config home exists for "<engine>" in the project

    Examples:
      | engine      |
      | claude-code |

  Scenario: An in-tree AGENT run that declares engine_home: host in a project Alice never trusted is refused
    Given Alice has a git-backed project
    And Alice has whatever credentials "claude-code" needs to authenticate
    And Alice has not trusted the project in her own claude
    And Alice's agent declares engine_home "host"
    When Alice runs the isolated "claude-code" agent under workspace "none"
    Then the run is refused because an untrusted repository's session cannot load the project's MCP file

  # The credential half of the "gets a ctxloom-controlled config home" scenario
  # above, claude-code only. Alice's OWN host session under the top-level
  # `auth: login` shares her login in place (CLAUDE_SECURESTORAGE_CONFIG_DIR set to
  # what her claude resolves), and the setup-token is blanked even when one
  # is exported: the declared mode decides. Nothing is copied into the session
  # home: Alice's own native login is neither read into it nor rewritten.
  #
  # The instance-side assertions are read from INSIDE the running engine: the
  # spy dumps what it was handed while it runs (whether the token var is
  # empty, never its value). The instance itself is the session's disposable
  # engine home (paths.HarpMembers), removed when the run closes. The last
  # line pins that disposal end to end.
  Scenario: An in-tree run shares Alice's own login in place and copies no credential
    Given Alice has a git-backed project
    And Alice has a "claude-code" credential fixture on the host
    And Alice has exported a "claude-code" setup-token
    And Alice's agent declares engine_home "session"
    And Alice's own session authenticates with "login"
    When Alice runs the isolated "claude-code" agent under workspace "none"
    Then the spy "claude-code" process shares Alice's own login in place
    And the spy "claude-code" process was handed no setup-token
    And the isolated "claude-code" home holds no credential file
    And the host "claude-code" credential file was never modified
    And the "claude-code" config-home instance is removed when the run closes

  # THE AMBIENT COPY-IN'S OTHER HALF (D4): claude's instance `.claude.json` is
  # GENERATED by ctxloom, not copied. Alice's onboarding answer crosses by name
  # so an agent session does not re-onboard; the workspace-trust answer is
  # generated for the directory the run works in when — and only when — Alice
  # trusted the repository in her own claude, since that verdict is hers and
  # ctxloom never answers the prompt for her; her account identity (oauthAccount) crosses because
  # claude reads it; and NOTHING else crosses — not her own mcpServers
  # registrations (and the secrets in their env blocks), not her per-project
  # prompt history, and not her standing "skip every permission prompt"
  # answer, which belongs to her own interactive session and not to an agent.
  #
  # The fixture deliberately carries all of them so the negative assertions
  # have something real to fail on: a scenario that proves "nothing leaked"
  # against a config with nothing in it proves nothing at all.
  Scenario Outline: An in-tree AGENT run generates claude's instance config without carrying Alice's own registrations or history
    Given Alice has a git-backed project
    And Alice has a "claude-code" credential fixture on the host
    And Alice has exported a "claude-code" setup-token
    And Alice has a personal claude config carrying her own MCP servers
    And Alice has <verdict> the project in her own claude
    And Alice's agent declares engine_home "session"
    When Alice runs the isolated "claude-code" agent under workspace "none"
    Then the run reports no isolation finding
    And the instance's claude config carries <answer> trust answer and the account identity, and none of Alice's own registrations or history
    And the host "claude-code" credential file was never modified

    Examples:
      | verdict     | answer        |
      | trusted     | the generated |
      | not trusted | no            |

  # Alice's login session needs no token: with none exported and no API var
  # it still proceeds, on her own login shared in place.
  Scenario: Alice's in-tree login session with no token proceeds on her own login
    Given Alice has a git-backed project
    And Alice has no "claude-code" credentials or API key on the host
    And Alice has a "claude-code" credential fixture on the host
    And Alice's agent declares engine_home "session"
    And Alice's own session authenticates with "login"
    When Alice runs the isolated "claude-code" agent under workspace "none"
    Then the run reports no isolation finding
    And the spy "claude-code" process's "CLAUDE_CONFIG_DIR" env var points at this session's config-home instance
    And the spy "claude-code" process shares Alice's own login in place
    And the spy "claude-code" process was handed no setup-token
    And the isolated "claude-code" home holds no credential file

  # ...but it needs the login. With no claude login on this host at all, her
  # login session is refused before any engine is spawned, naming what to do,
  # rather than started logged out (ruled 2026-09-27, C1/F7: the host and the
  # container refuse alike).
  Scenario: Alice's login session with no login on the host is refused, naming auth token
    Given Alice has a git-backed project
    And Alice has no "claude-code" credentials or API key on the host
    And Alice has no "claude-code" credentials on the host
    And Alice's agent declares engine_home "session"
    And Alice's own session authenticates with "login"
    When Alice runs the isolated "claude-code" agent under workspace "none"
    Then the run is refused without any isolation finding, naming "auth: token"

  # The token riding the environment reaches the run: the engine really
  # launches, and the config-home variable it is handed points at this
  # session's instance -- the controlled home the binding asked for is still
  # in place.
  #
  # "PROCEED" is the load-bearing word in this scenario's title, so it is what
  # gets asserted: the run exits 0 and the engine really launches. Asserting
  # merely that no finding was printed used to let this pass under a mutation
  # that stopped any engine from launching, and under one that collapsed
  # isolation to nothing at all.
  Scenario Outline: The same engines proceed without any isolation finding once their token rides the environment
    Given Alice has a git-backed project
    And Alice has no "<engine>" credentials on the host
    And Alice has exported a "<engine>" setup-token
    And Alice's agent declares engine_home "session"
    When Alice runs the isolated "<engine>" agent under workspace "worktree"
    Then the run reports no isolation finding
    And the spy "<engine>" process's "<var>" env var points at this session's config-home instance

    Examples:
      | engine      | var               |
      | claude-code | CLAUDE_CONFIG_DIR |

  # The worktree cell gets the SAME session home and the SAME auth as the
  # in-tree cell — the home is orthogonal to the worktree. The spy is
  # cooperative BY CONSTRUCTION, so this proves ctxloom's own bookkeeping
  # (the var it sets, the token it blanks, the login it leaves alone),
  # not that a real engine honours them; the vendor half is the live
  # isolation probe (tests/acceptance/features/probes/isolation_probe.feature).
  Scenario: A worktree claude run shares Alice's own login in place and never touches it
    Given Alice has a git-backed project
    And Alice has a "claude-code" credential fixture on the host
    And Alice has exported a "claude-code" setup-token
    And Alice's agent declares engine_home "session"
    And Alice's own session authenticates with "login"
    When Alice runs the isolated "claude-code" agent under workspace "worktree"
    Then the spy "claude-code" process's "CLAUDE_CONFIG_DIR" env var points at this session's config-home instance
    And the spy "claude-code" process shares Alice's own login in place
    And the spy "claude-code" process was handed no setup-token
    And the isolated "claude-code" home holds no credential file
    And the host "claude-code" credential file was never modified

  # THE HUMAN MINTS, CTXLOOM READS (ruled 2026-09-28). A session with no
  # top-level `auth:` authenticates with a token the human mints with `claude
  # setup-token` and exports; ctxloom never captures or stores one. With none
  # exported the run is refused before any engine is spawned, at a terminal
  # or not, naming how to mint and export it. An exported API key does not
  # stand in: the declared mode decides.
  Scenario: A token session with no exported token is refused, naming claude setup-token
    Given Alice has a git-backed project
    And Alice has no "claude-code" credentials or API key on the host
    And Alice has set the "claude-code" API key in the environment
    And Alice's agent declares engine_home "session"
    When Alice runs the isolated "claude-code" agent under workspace "none"
    Then the run is refused without any isolation finding, naming "claude setup-token"

  # ARGV/STDIN VISIBILITY (U161-F01) — the spy previously dumped only its own
  # environment; it never emitted "$@" and never read stdin, so every argv
  # defect (a flag whose value is dropped, a variadic flag swallowing the
  # prompt, a missing required flag, a mutually exclusive pair emitted
  # together) was structurally invisible to this suite, in the one place
  # that could have seen it — the actual argv/stdin a real engine binary
  # receives. This scenario puts claude-code's per-engine argv/stdin payload
  # under acceptance coverage for the first time.
  Scenario: The spy captures the real argv and stdin a live engine binary would receive
    Given Alice has a git-backed project
    And Alice has exported a "claude-code" setup-token
    When Alice runs the isolated "claude-code" agent under workspace "worktree"
    Then the spy "claude-code" process's ARGV contains "--print"
    And the spy "claude-code" process's STDIN contains "hello"
