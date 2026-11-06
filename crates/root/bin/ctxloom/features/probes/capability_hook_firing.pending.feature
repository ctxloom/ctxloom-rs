# Ported from probes/capability_hook_firing.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   capability_hook_firing.feature with its steps (gherkin_lint.test.rs).

@live
@pending
Feature: P3 — hooks actually FIRE, proven by the hook's own stamp file

  ctxloom writes hooks into its engines' native surfaces — for claude, that is
  .claude/settings.json.
  That ctxloom writes the right BYTES is well proven —
  golden tests, the settings-io tests, and
  TestDeliveryApproach_EveryHookEngineDeliversACarriedEvent in tests/integration all
  check carriage, and carriage is our own behaviour.

  Whether a vendor binary then READS that file and EXECS the command in it has
  never been proven anywhere in this repo, hermetically or live. That is the one
  step in the chain we do not control, and it is the entire subject of this
  feature. Carriage without firing is a hook that exists only in a file nobody
  ran — success-shaped, silent, and exactly the failure this project produces
  most often.

  THE PROOF IS A FILE, NOT A SENTENCE. The hook ctxloom delivers does one thing:
  it appends the harp it was handed ON ITS OWN ARGV to a stamp file beside the
  script itself, resolved from the script's own location rather than from a cwd
  nobody promised us. The assertion reads that file's BYTES. An engine cannot
  fake this by being clever with the settings file it was given — the harp sits in that file in plain text and any engine
  could quote it back — because only an engine that actually EXECUTED the
  command can make the stamp file exist at all. Quotable versus executable is
  the whole probe.

  Nothing here can flake on prose habits. There are no fences to strip, no
  preamble to tolerate, no output contract to honour. That is deliberate: the
  design's own counter-argument notes that a strict text assertion measures
  engine and prompt jointly, so where a claim admits a non-prose observable, the
  probe takes it.

  AN EMPTY STAMP FILE IS NOT A PASS, AND A MISSING ONE IS NOT A SKIP. Three
  outcomes are named separately because they are three different findings: the
  file is absent (the hook never ran), the file exists and is empty (the hook
  ran and wrote nothing — this project's characteristic silent no-op), the file
  carries bytes that are not this cell's harp (something ran, but not this hook
  with this argument). A bare existence check would blur all three.

  TWO STAGES. Stage (a) — firing — runs on every cell. Stage (b) asks whether the
  engine INGESTS what the hook printed, and it is asserted only where production
  itself makes that claim: an engine whose context route is a hook. None is:
  claude takes its context as a launch's system prompt and no hook carries it
  (onectx), so asserting an echo on a claude cell would red it for failing to do
  something ctxloom never asked. Stage (b) runs on no cell today. It uses a SECOND minted harp, planted only in the hook's
  standard output, so the two stages cannot satisfy each other.

  WHAT IS ABSENT HERE IS DECLARED, NOT FORGOTTEN. A backend that declares hooks
  gone at the mechanism level (noHooksReason) gets no Examples row — a gate by
  ABSENCE, recorded in the probe registry, where the completeness test refuses an
  undeclared gap. The same holds at finer grain for a single unsupported kind
  (unsupportedHookKinds[bundles.HookEventSessionEnd]), which is precisely why
  this probe plants on session_start.

  THE CONTAINER AXIS, AND THE REASON IT WAITED. This probe was host/none only,
  and the reason was physical rather than cautious: the stamp file was an
  absolute HOST path, so a containerized engine would have written it inside
  another filesystem namespace and this assertion — reading the host path —
  would have reported a hook-firing failure that was really a mount gap. That is
  now fixed AT THE FIXTURE, which is why it needed no mount and no production
  change: the script resolves its stamp from `dirname "$0"`, so the proof lands
  beside the script wherever the workspace was mounted or checked out, and the
  workspace is the one thing a container cell bind-mounts. The stamp is then read
  from the directory the engine ACTUALLY RAN IN, which for a worktree cell is a
  per-agent checkout rather than the project.

  Moving the fixture inside the workspace does mean an agent can now READ the
  stamp script. That costs nothing: the verdict demands the file EXIST carrying
  this cell's argv harp, and reading a path does not create a file. That is not a
  theoretical guard — a P3 cell of 2026-08-13 found its harp by grepping the
  fixture's own script, answered correctly, and red anyway.

  ADDRESSING ONE CELL. Every Examples block carries its engine and both axes as
  tags, so an ACCEPTANCE_TAGS expression of the live tag, this probe's tag, the
  engine tag, the runtime tag and the workspace tag — joined with && — runs
  exactly one cell. (Written out in prose rather than shown literally: a
  description line that BEGINS with an at-sign is parsed as a tag line, and a
  tag containing whitespace aborts the parse of this whole file, taking every
  scenario in it. That is not hypothetical; it happened while writing this
  paragraph.) These are real paid turns; run them one at a time.

  Scenario Outline: A <engine> run execs the session_start hook ctxloom delivered
    Given the hook-firing probe targets "<engine>" under runtime "<runtime>" and workspace "<workspace>"
    When it runs one turn with that hook installed
    Then the hook's stamp file carries the harp it was given on its argv

    @claude-code @host @ws-none @probe-p3-hook-firing
    Examples:
      | engine      | runtime | workspace |
      | claude-code | host    | none      |

    # The container cells, added once the stamp stopped being a host-absolute
    # path. A PAIR on purpose: P6's host/worktree cell failed where both-off and
    # both-on passed, so a container row without its worktree partner rebuilds
    # that blind spot. Stage (a) only, as on claude's host row — no hook carries
    # claude's context, so this cell must not assert an echo production never
    # asked for.
    # The settings, hooks and command files claude reads live under its engine
    # home, which the runner writes at the CONTAINER side of that mount
    # (runner.Execute, pinned by TestCoordContainerEngineHome_DeliveredAtTheContainerSidePath).
    # A write to the host path instead leaves claude a hookless home: exit 0, a
    # normal answer, and no stamp — the shape this row exists to catch.
    @claude-code @container-rootless @ws-none @probe-p3-hook-firing
    Examples:
      | engine      | runtime            | workspace |
      | claude-code | container-rootless | none      |

    # The worktree partner. The hook script reaches the per-agent checkout only
    # because the fixture is COMMITTED, and the stamp is written there rather
    # than in the project — probeCellRunDir resolves that checkout after the run.
    # A clean checkout is pruned by the WIP-safe teardown, so a hook that never
    # fires leaves NO checkout to read and the cell refuses rather than falling
    # back to the project directory, which would let it pass on the host
    # fixture's evidence.
    @claude-code @container-rootless @ws-worktree @probe-p3-hook-firing
    Examples:
      | engine      | runtime            | workspace |
      | claude-code | container-rootless | worktree  |

    # OPENCODE, on a free hosted model (liveOpencodeModel): the host row is
    # live-verified. opencode runs no hook command: its session_start fires from the bridge
    # plugin's session.created (opencode hooks.go), so this cell is the one
    # live proof that the bridge executes a hook ctxloom wrote.
    @opencode @host @ws-none @probe-p3-hook-firing
    Examples:
      | engine   | runtime | workspace |
      | opencode | host    | none      |

    # opencode.Opencode.Container installs the release (installFragment). Both
    # rootless cells are LIVE-VERIFIED (2026-10-13; the evidence is in
    # capability_probe_registry.go).
    # On the live-container lane they skip loudly: its image ships no opencode
    # binary on the harness side (engineAvailable).
    @opencode @container-rootless @ws-none @probe-p3-hook-firing
    Examples:
      | engine   | runtime            | workspace |
      | opencode | container-rootless | none      |

    @opencode @container-rootless @ws-worktree @probe-p3-hook-firing
    Examples:
      | engine   | runtime            | workspace |
      | opencode | container-rootless | worktree  |
