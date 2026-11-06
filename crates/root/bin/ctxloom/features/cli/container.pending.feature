# Ported from cli/container.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   container.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from cli/container.feature`.

@doc
@pending
Feature: container — the images isolated agents run in, and the questions you can ask without one

  Covers: `ctxloom container check`, `container scaffold`, `container prune`, `container tooling
  list`, the bare `ctxloom container tooling` form, and the bare `ctxloom
  container` namespace itself.

  An agent with `runtime: container` does not run on your machine — it runs
  inside a per-backend IMAGE, and this noun owns that image. The image builds in
  two stages: a shared BASE (the distro plus the coding-agent tool layer) and
  the engine's AGENT stage (the client CLI plus the running ctxloom binary)
  layered on top. `build` produces it, `scaffold` makes the base stage yours to
  edit, `tooling list` collects what installed content says it needs in there,
  and `check` tells you whether any of it can work here at all.

  WHAT THIS FILE CANNOT ASSERT, AND SAYS SO RATHER THAN PRETENDING. `ctxloom
  container build` needs a real container runtime, network pulls of each
  engine's official installer, and minutes of wall clock. It is listed in
  completeness_test.go's excludedLeaves for exactly that reason, and no
  scenario here drives it; the containerized run path is exercised by
  j002400_container.feature, behind a tag the default suite skips.
  What IS specifiable without a runtime is everything around it: the read-only
  diagnosis, the file scaffold, the registered-companion collection, and the namespace's
  own dispatch.

  @image-mock-agent
  Rule: Check reports; it never builds, changes or blocks anything

    `container check` answers whether `runtime: container` agents can launch
    here: whether THIS process is already inside a container, which runtime is
    reachable, whether the backend's image is present, and whether the
    runtime's daemon shares this filesystem (the docker-outside-of-docker
    detector, where bind mounts silently resolve against the wrong filesystem
    and launches hang).

    No probe outcome ever fails the command — read the report, not the exit
    code. A USAGE error still is one.

    # DIAGNOSTIC-ONLY is a claim about what the command does NOT do, and no
    # assertion on its own stdout can see it writing a file somewhere else: the
    # static "Container capability" header prints either way. The read-only
    # half is asserted where it actually lives — the project tree, byte for
    # byte.
    #
    # The report's CONTENT is asserted by the lines that are always present
    # whatever this machine has installed. Whether a runtime is reachable is a
    # fact about the developer's laptop; that the report answers the question
    # at all is a fact about ctxloom.
    #
    # FOLDED FROM journeys/j002400_container.feature (Phase 3, D7): that
    # journey used to assert this same axis-completeness claim on its own,
    # plus one line this scenario did not yet check ("runtime:", the reachable
    # runtime name/status line) — moved down here rather than restated in two
    # places, so the noun's own spec is the one place asserting every axis the
    # report claims to answer.
    #
    # shared_fs="ok" is a claim the check can only earn against a PRESENT
    # image (Diagnose only runs the definitive marker probe when one exists;
    # otherwise it falls back to an advisory heuristic). The Rule's
    # @image-mock-agent tag has the suite build one before any scenario runs,
    # rather than leaving that claim resting on whatever happens to already
    # be on the machine (dragging-neatness); the Background checks it is there.
    # On a machine with no container runtime the hermetic lane excludes this
    # Rule before the run and prints every row it left out, with the reason
    # (excludeSkippedSuiteImages); CTXLOOM_REQUIRE_DOCKER=1 makes it a failure.
    # The subject is "mock", not a vendor engine, because that build costs
    # nothing beyond the base image — see the Background's own comment.
    #
    # Whether this process is in a container is a fact about the MACHINE: CI
    # runs this suite inside the devcontainer image, where "true" is the
    # correct report. So the claim is that the report answers with a boolean;
    # WHICH boolean the markers produce is containerprobe's unit tests' to pin.
    # shared_fs="ok" is different: it is a precondition this lane REQUIRES of
    # the machine — a daemon that resolves this process's paths to the same
    # files. Docker-outside-of-docker does not, unless the directory the
    # scenarios run in is shared with the daemon at the SAME path.
    Background:
      Given the mock agent image is available for the shared-filesystem probe

    @reach-back @R6
    Scenario Outline: The capability check is diagnostic-only
      Given an initialized ctxloom project
      And I record the project tree
      When Alice asks whether containerized agents could run here:
        """
        ctxloom container check mock <flags>
        """
      Then the command succeeds
      And the output reports "image" matching "<names the backend it checked>"
      And the output reports "in_container" matching "<says whether it is in a container>"
      And the output reports "runtime" matching "<names the runtime>"
      And the output reports "shared_fs" matching "<reports the shared fs>"
      And the project tree is unchanged

    @json-default
    Examples: no --format at all takes the derived default off a terminal, and an explicit json agrees
      | flags         | names the backend it checked          | says whether it is in a container | names the runtime | reports the shared fs |
      |               | mock                                  | ^(true\|false)$                   | \S               | ok                    |
      | --format json | mock                                  | ^(true\|false)$                   | \S               | ok                    |

    # Named backend vs. resolved default are two different code paths: with no
    # argument the project's configured default is resolved, and a report that
    # named no engine at all would describe nothing while still looking like a
    # report.
    #
    # unsent-refinish, ruled 2026-09-04: a bare invocation resolves through
    # cfg.PrimaryLabel() (Config.ResolveLLM's empty-label case), so setting
    # llm.defaults.primary: mock here redirects the resolved default to the
    # image this Rule's Background already builds, and shared_fs can genuinely
    # earn "ok" without a vendor build.
    Scenario Outline: With no backend named, the check reports on the project's default
      Given an initialized ctxloom project
      And the mock LLM responds "MOCK-REPLY-NEVER-SENT-CONTAINER-CHECK-ONLY-PROBES"
      When I run "ctxloom container check <flags>"
      Then the command succeeds
      And the output reports "image" matching "<names the backend it resolved>"
      And the output does not contain "(unresolved)"
      And the output reports "shared_fs" matching "<reports the shared fs>"
      And the output does not contain "MOCK-REPLY-NEVER-SENT-CONTAINER-CHECK-ONLY-PROBES"

    @json-default
    Examples: no --format at all takes the derived default off a terminal, and an explicit json agrees
      | flags         | names the backend it resolved  | reports the shared fs |
      |               | ctxloom-agent-\S               | ok                    |
      | --format json | ctxloom-agent-\S               | ok                    |
