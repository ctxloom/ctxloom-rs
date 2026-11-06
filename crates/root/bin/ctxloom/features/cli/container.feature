# world: isolation
# Ported from cli/container.feature (rev f36e8dfb2)
# NOT ported: the Rule "Check reports; it never builds, changes or blocks
#   anything" (tagged @pending, both its Scenario Outlines): it is
#   @image-mock-agent, so it needs a live container runtime and a built mock
#   agent image (plan 5.6: the live lane, CTXLOOM_REQUIRE_RUNTIMES, scheduled
#   and never in hooks). The hermetic half of `container check` is the next
#   Rule's.
#   The bare noun's heading "Available Commands:" is clap's "Commands:" here
#   (plan 8.2: the command line is clap's, not cobra's); the step maps it.
@doc
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

  Rule: The namespace answers what it holds, and refuses what it does not

    `container` has no safe read-only view to default to — `check` probes a
    runtime, `build` writes an image — so the bare form prints help, which is
    still a real answer to "what does this noun do". A MISSPELLED verb is the
    case that matters: a namespace that printed its own help on a typo and
    exited 0 is this project's characteristic silent no-op wearing cobra's
    dispatch as a disguise.

    Scenario: The bare noun lists what the namespace holds
      Given an initialized ctxloom project
      When I run "ctxloom container"
      Then the command succeeds
      And the output contains "Available Commands:"
      And the output contains "scaffold"
      And the output contains "tooling"

    Scenario: A misspelled verb fails instead of printing help
      Given an initialized ctxloom project
      When Alice mistypes the verb she wanted:
        """
        ctxloom container buidl
        """
      Then the command fails
      And the output contains "unknown command"
      And the output contains "buidl"

    # Help is prose, and no encoding carries prose. A caller that asked for
    # json to parse and got help text back — with a 0 exit saying all was well
    # — is the same silent no-op one layer up.
    Scenario: A machine-readable format aimed at the namespace itself is refused
      Given an initialized ctxloom project
      When I run "ctxloom --format json container"
      Then the command fails
      And the output contains "help is not a payload"

  @image-mock-agent
  @pending
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

  Rule: Check's usage errors and its text labels need no runtime at all

    The rest of what `container check` promises does not depend on what this
    machine has installed, so it is asserted where no image is built and the
    host's runtimes are hidden: a backend name is validated before anything is
    probed, and the text report labels every axis whether or not a probe
    answered. An explicit `--format text` is also the half of "an explicit
    format beats the derived default" that the json rows above cannot show.

    Scenario: An unknown backend is a usage error, and the real ones are named
      Given an initialized ctxloom project
      When Alice asks about an engine that does not exist:
        """
        ctxloom container check totally-bogus-engine
        """
      Then the command fails
      And the output contains "unknown backend"
      And the output contains "totally-bogus-engine"
      And the output contains "claude-code"

    Scenario: The text report labels every axis, and writes nothing
      Given an initialized ctxloom project
      And I record the project tree
      When Alice asks whether containerized agents could run here:
        """
        ctxloom container check mock --format text
        """
      Then the command succeeds
      And the output reports "image" matching "Container capability (backend: mock)"
      And the output reports "in_container" matching "in a container:"
      And the output reports "runtime" matching "runtime:"
      And the output reports "shared_fs" matching "shared fs:"
      And the project tree is unchanged

    Scenario: With no backend named, the text report still names one
      Given an initialized ctxloom project
      And the mock LLM responds "MOCK-REPLY-NEVER-SENT-CONTAINER-CHECK-ONLY-PROBES"
      When I run "ctxloom container check --format text"
      Then the command succeeds
      And the output reports "image" matching "Container capability (backend:"
      And the output does not contain "(unresolved)"
      And the output reports "shared_fs" matching "shared fs:"
      And the output does not contain "MOCK-REPLY-NEVER-SENT-CONTAINER-CHECK-ONLY-PROBES"

  Rule: Build validates its backend before touching any daemon

    `container build` needs a real container runtime to actually build an
    image, which is why no scenario in this file drives a real build (see this
    file's own header). But the backend name is validated BEFORE any of that —
    the same `unknown backend` refusal `container check` uses — so a typo is
    caught immediately rather than surfacing as a confusing daemon error two
    steps later, or worse, resolving to whatever backend the fallback happens
    to pick.

    # FOLDED FROM journeys/j002400_container.feature (Phase 3, D7): the one
    # claim in that file's three now-deleted generic scenarios that had no
    # home anywhere else. Moved down rather than restated, since this is
    # comprehensive per-noun coverage, not a narrative beat.
    Scenario: An unknown backend is refused before any daemon is touched
      Given an initialized ctxloom project
      When Alice asks to build an image for an engine that does not exist:
        """
        ctxloom container build totally-bogus-engine
        """
      Then the command fails
      And the output contains "unknown backend"
      And the output contains "totally-bogus-engine"
      And the output contains "claude-code"

  Rule: Scaffold gives a project a devcontainer, and never touches an existing one

    `container scaffold` writes .devcontainer/ — a devcontainer.json building
    the Dockerfile beside it, seeded from ctxloom's embedded default base — so
    the agent image's base becomes a file you edit. It writes no config: an
    unset isolation_base already adopts a project devcontainer.

    A project that already has a devcontainer is refused outright: that is the
    environment the human develops in, and the agent image already builds on
    it. There is no --force, because there is nothing a scaffold could add that
    editing the existing one would not.

    Scenario: Scaffolding writes a devcontainer seeded from the default base
      Given an initialized ctxloom project
      When Alice takes ownership of the base image stage:
        """
        ctxloom container scaffold
        """
      Then the command succeeds
      And the file ".devcontainer/Dockerfile" contains "FROM"
      And the file ".devcontainer/devcontainer.json" contains "Dockerfile"
      And the file ".ctxloom/config.yaml" does not contain "isolation_base"

    # THE DESTROYER: the refusal must leave the human's bytes exactly where
    # they were — asserted by the marker SURVIVING and by the embedded
    # default's "FROM" being ABSENT, because a clobber that kept the file
    # non-empty would satisfy a file-exists check.
    Scenario: An existing devcontainer is refused and left untouched
      Given an initialized ctxloom project
      And the project already has the file ".devcontainer/Dockerfile":
        """
        MY-OWN-DEVCONTAINER: hand-written, must survive a scaffold.
        """
      When Alice scaffolds over the environment she already has:
        """
        ctxloom container scaffold
        """
      Then the command fails
      And the output contains "already has a devcontainer"
      And the file ".devcontainer/Dockerfile" contains "MY-OWN-DEVCONTAINER"
      And the file ".devcontainer/Dockerfile" does not contain "FROM"
      And the file ".devcontainer/devcontainer.json" does not exist

    # The spec's other canonical spelling. The positive control is the first
    # scenario in this Rule, which proves the same fixture DOES produce
    # .devcontainer/devcontainer.json — so its absence here means the write
    # was refused, not that scaffolding never writes anything.
    Scenario: A root .devcontainer.json is a devcontainer too
      Given an initialized ctxloom project
      And the project already has the file ".devcontainer.json":
        """
        {"image": "mcr.microsoft.com/devcontainers/base:ubuntu"}
        """
      When Alice scaffolds a second devcontainer beside it:
        """
        ctxloom container scaffold
        """
      Then the command fails
      And the output contains ".devcontainer.json"
      And the file ".devcontainer/devcontainer.json" does not exist

  Rule: Tooling collection reads only registered companions, and never applies anything itself

    A companion declares the tools its content needs inside the agent image as
    the typed `tooling` field of its loadout's init section. `container tooling
    list` collects those declarations from REGISTERED companions and emits them
    with instructions for the LLM: locate or scaffold the base Containerfile,
    propose the additions as a diff, get the user's explicit approval per
    change, then rebuild.

    A companion nobody registered (`ctxloom companion add`) is never run, so
    it declares nothing — and nothing is ever written here. The edit is the
    LLM's, gated by the user.

    # Adjusted under the loadout contract v2 (ugly-yodel): a bundle no longer
    # declares tooling — the well-known `tooling` command is gone. A companion
    # declares it, typed, in its loadout; the excluded case is therefore a
    # companion on PATH that this machine never registered, which is never
    # executed and so declares nothing at all.
    #
    # ABSENCE SATISFIED ABSENCE. With nothing declared anywhere, "none
    # reported" was equally consistent with registration scoping working and
    # with collection being broken outright — a render that dropped EVERY
    # declaration, registered or not, left this green. So the fixture declares
    # tooling twice: once from a companion nobody registered (must be
    # withheld, and the summary line is then a fact about REGISTRATION), and
    # once from a registered companion (must come through — the positive
    # control that makes "none reported" mean something).
    #
    # The unregistered companion's body must appear on no route: it is
    # publisher-authored text from a binary nothing ran. The "does not contain
    # TOOLING-DECL-SHADY" assertions pin that.
    Scenario Outline: An unregistered companion contributes nothing, and a registered one comes through
      Given an initialized ctxloom project
      And a companion "shady" declaring container tooling "TOOLING-DECL-SHADY", on PATH but not registered
      When Alice collects what her installed content needs in the image:
        """
        ctxloom container tooling list <flags>
        """
      Then the command succeeds
      And the output reports "declarations" as empty, saying "No registered companion declares container tooling"
      And the output does not contain "TOOLING-DECL-SHADY"
      Given a companion "tooled" declaring container tooling "TOOLING-DECL-TOOLED"
      When Alice collects again now that a registered companion declares tooling:
        """
        ctxloom container tooling list <flags>
        """
      Then the command succeeds
      And the output contains "TOOLING-DECL-TOOLED"
      And the output does not contain "TOOLING-DECL-SHADY"

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         |
      |               |
      | --format json |
      | --format text |

    # `tooling` is a sub-noun carrying the spine verb `list` rather than a bare
    # leaf, so it composes with the rest of the CLI's noun-verb shape — and its
    # bare form still lists, through the same seam `ctxloom remote` uses, so
    # nothing a caller already types stops working.
    Scenario: Bare container tooling lists the declarations
      Given an initialized ctxloom project
      And a companion "tooled" declaring container tooling "TOOLING-DECL-TOOLED"
      When I run "ctxloom container tooling"
      Then the command succeeds
      And the output contains "TOOLING-DECL-TOOLED"
      And the output does not contain "Available Commands:"

    # The instructions and the declarations are two different fields, and a
    # caller parsing this needs both: emitting only the preamble would read as
    # a working response and carry no content to apply.
    Scenario: The machine-readable form carries the instructions and the declarations
      Given an initialized ctxloom project
      And a companion "tooled" declaring container tooling "TOOLING-DECL-TOOLED"
      When I run "ctxloom --format json container tooling list"
      Then the command succeeds
      And the output is valid JSON
      And the output contains "instructions"
      And the output contains "TOOLING-DECL-TOOLED"

  Rule: Prune plans before it removes, and says so when there is nothing to prune with

    `container prune` sweeps the superseded agent images ctxloom built — a dry
    run unless `--yes`. What is specifiable without a container runtime is
    its two refusals: a `--runtime` that names no runtime ctxloom knows, and a
    machine with no runtime at all. The second is its own documented exit
    status (3), distinct from "a removal failed" (1): a caller scripting prune
    has to be able to tell "could not look" from "looked and failed", and a
    0 here would report a clean sweep of images nobody could see.

    Scenario: A --runtime ctxloom does not know is refused, naming the ones it does
      Given an initialized ctxloom project
      When I run "ctxloom container prune --runtime rkt"
      Then the command fails
      And the output contains "unknown container runtime"
      And the output contains "rkt"
      And the output contains "docker"

    Scenario: With no container runtime on this machine, prune exits 3 rather than reporting a clean sweep
      Given an initialized ctxloom project
      And no container runtime is installed
      When I run "ctxloom container prune"
      Then the command exits with code 3
      And the output contains "no container runtime is available to prune"
