# Ported from journeys/j002400_container.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j002400_container.feature with its steps (gherkin_lint.test.rs).

@pending
Feature: The container runtime axis — can an engine actually land in a container here, and did it?

  ctxloom's runtime axis puts an agent's ENGINE process inside a container
  (`runtime: container-rootless`) instead of on the host, so a risky or untrusted task
  runs against a fresh HOME, a scoped filesystem, and read-only credential
  mounts rather than the operator's live environment. Containers are the axis
  that makes isolation a PROPERTY of the runtime rather than a request the
  engine may ignore — some vendor CLIs write credentials to a global path no
  matter what environment variable you hand them — which is exactly why it is
  the axis that must be proven by observation rather than assumed.

  `ctxloom container check` is how an operator answers "will `runtime:
  container-rootless` work on this host?" without launching anything, and `container
  build` provisions the per-backend agent image a run would use. Without that
  diagnostic surface the only way to find out is to start a run and watch it
  abort.

  # WHAT THIS JOURNEY CAN AND CANNOT SEE. tests/acceptance drives a `ctxloom`
  # SUBPROCESS, so every assertion observes what the real command printed, the
  # exit class it returned, or a file it left behind — never an injected seam.
  # The real launch is NOT hermetic: it needs a reachable docker/podman daemon
  # and an agent image. So the one scenario below that actually lands an
  # engine in a container is tagged @container, excluded from the default
  # run, and has its own gate — `just test-acceptance-container`. A green
  # default run of this file therefore does NOT mean "containers proven"; it
  # means nothing ran, because the only scenario left is the launch itself.
  # The launch is never faked here.
  #
  # FOLDED (Phase 3, D7): this file used to carry three more scenarios —
  # `container check <backend>` answering every capability axis, `container
  # check <unknown>` failing loud, and `container build <unknown>` failing
  # loud — kept only because the OLD completeness gate credited a leaf on
  # literal `I run "<leaf>"` TEXT, and these were the only text mentioning
  # `container check`/`container build` at all. Coverage now follows
  # EXECUTION, not text, so that reason is gone, and cli/container.feature's
  # own "Check reports" Rule already asserts the diagnostic-axis and
  # unknown-check-backend claims with a STRONGER payload (it also proves the
  # project tree is byte-for-byte unchanged, which these never checked). The
  # one thing that was genuinely new — `container build <unknown>` also
  # validates before touching a daemon — moved DOWN into cli/container.feature
  # as its own Rule, rather than being restated here. NOT TAGGED @doc: the
  # only scenario left is @container and excluded from the default run, so a
  # default docs-generation pass captures no evidence from this file at all.
  #
  # DELIBERATELY NOT RESTATED (covered elsewhere): j002200_isolation.feature owns
  # the fail-loud/degrade contract for a requested container that CANNOT launch
  # — and self-skips where one can, so on a docker-having machine THIS file is
  # the only executable evidence for the runtime axis. agent.feature owns the
  # `--runtime container-rootless` round-trip through agent set/show and the `container
  # check` header line. cli/container.feature owns the comprehensive,
  # hermetic surface around the launch (diagnostics, scaffold, tooling).

  Background:
    Given an initialized ctxloom project

  # THE REAL BOUNDARY. Everything cli/container.feature proves is the surface;
  # this proves the launch. It is DIFFERENTIAL by construction — the same agent
  # runs twice, once per runtime — because an absolute reading cannot be
  # trusted: when the test harness itself runs inside a devcontainer, "am I in
  # a container" is true on BOTH sides, and a scenario asserting only that
  # would pass with no container ever launched. Running the host leg first
  # establishes what this machine looks like, so the container leg's claim is
  # "somewhere this host is not" rather than "somewhere containerish".
  #
  # The evidence is the engine's own record file, written from wherever the
  # engine actually ran and reaching the host through the workspace mount. Exit
  # code cannot serve: a containerized run and a run that silently never
  # containerized both exit 0 and both echo the prompt.
  @container @image-mock-agent
  Scenario: A containerized run puts the engine somewhere this host is not
    When Alice runs the mock agent with runtime "host"
    Then the engine left its record, and it ran on this host
    When Alice runs the mock agent with runtime "container"
    Then the engine left its record, and it ran inside a container, not on this host
