# Ported from journeys/j002200_isolation.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending: they wait for `ctxloom run` to
#   launch an engine. The container gates, the dry-run preview, the worktree's
#   git exclude and the credential refusals are cli/run.feature's.

@doc
@pending
Feature: Bounding what the agent can reach, even with permissions bypassed

  Alice runs an agent with its permission prompts turned off. She is not
  being reckless: approving every file read one dialog at a time is unusable
  at the pace the work happens, and her employer's policy is not "prompt the
  human more", it is "the assistant must not be able to reach what it has no
  business reaching". Once the prompts are off, the engine's own guardrails
  are not what protects the rest of her disk. Something else has to.

  ctxloom gives her two independent choices. The workspace: the live project,
  or a checkout of its own. The runtime: her machine, or a container. Each
  run also gets its own session home, so the agent does not run on her own
  engine setup, with her memory, her MCP servers and her history.

  This journey is Alice establishing that the choices are real: that a run
  asking for its own checkout works somewhere else, that two such runs never
  share one, and that when she asks for a container and the machine cannot
  give her one, the run stops rather than quietly running unsandboxed. A
  sandbox that silently degrades is worse than none, because she would have
  stopped.

  Background:
    # substitution: told as the user's flow, with the commands she runs.
    Given Alice has a git-backed project with a mock agent "mock"

  # The workspace changes where the agent works, and where its work lands:
  # in the project itself, or in a checkout of its own that its session owns,
  # inside the project's .ctxloom where she can see it and git cannot. It holds on her machine and in a container alike.
  @every-runtime
  Scenario Outline: The same run lands in the workspace its axis dictates
    # substitution: said as where the agent's work lands, on every runtime lane
    When Alice runs the mock agent under workspace "<workspace>":
      """
      ctxloom run --one-shot --agent mock --workspace <workspace> "write a short note"
      """
    Then what the agent wrote is in <where>

    Examples:
      | workspace | where                                                      |
      | none      | the project                                                |
      | worktree  | a checkout its session owns, inside the project's .ctxloom |

  # A checkout of its own is a convenience: outside a git repository there is
  # nothing to check out, so the agent works in the project instead. Losing it
  # loses no boundary Alice was promised; losing a container would.
  Scenario: Asking for a worktree outside git runs in the live project directory instead of refusing
    # substitution: said as where the agent's work lands
    Given Alice's project is not a git repository
    When Alice runs the mock agent under workspace "worktree":
      """
      ctxloom run --one-shot --agent mock --workspace worktree "write a short note"
      """
    Then what the agent wrote is in the project

  # Two agents each working in a checkout of their own never share one, and
  # neither works in the project.
  Scenario: Two isolated runs share no workspace between them
    # substitution: told as the user's flow, with the commands she runs.
    When Alice runs the mock agent twice, each under workspace "worktree":
      """
      ctxloom run --one-shot --agent mock --workspace worktree "fix the login bug"
      ctxloom run --one-shot --agent mock --workspace worktree "update the changelog"
      """
    Then the two runs' workdirs are distinct and neither is under the project directory

  # When the agent is done, Alice's project shows nothing it did not ask for.
  Scenario: A worktree run leaves the project tree clean
    # substitution: the project's git status is what Alice sees; the exclude file is cli/run.pending.feature's
    When Alice runs the mock agent under workspace "worktree":
      """
      ctxloom run --one-shot --agent mock --workspace worktree "write a short note"
      """
    Then Alice's project shows no change in git

  # Alice asks for a container on a machine that cannot run one. The run
  # stops and tells her why, and saying --degraded does not change that.
  Scenario Outline: Requesting a container with no runtime REFUSES, and --degraded does not rescue it
    # substitution: told as the user's flow, with the commands she runs.
    When Alice runs her container-bound agent "sandboxed":
      """
      ctxloom run --one-shot --agent sandboxed <flags> "write a short note"
      """
    Then the run aborts with an isolation finding

    Examples:
      | flags      |
      |            |
      | --degraded |

  # The container started and then died before the agent could run in it.
  # It is gone, and so are its logs; what the container program said as it
  # dropped the container is the only account there is, and Alice is given it
  # with the refusal.
  Scenario: A container the daemon kills before it runs is refused with the daemon's own reason
    # substitution: told as the user's flow, with the commands she runs.
    When Alice runs her container-bound agent "sandboxed", whose container the daemon kills, saying why:
      """
      ctxloom run --one-shot --agent sandboxed "write a short note"
      """
    Then the run aborts because the container never reached running state
    And Alice is told the daemon's own reason

  # Alice's agent runs from its own session home, whether or not its agent
  # says so: her own engine setup is never opened, and is left as it was.
  Scenario Outline: An in-tree AGENT run gets a per-session config-home instance instead of Alice's own home
    # substitution: one scenario for a session's own home, with the agent declaring it or not
    Given Alice has a git-backed project
    And Alice has whatever credentials "<engine>" needs to authenticate
    And Alice's agent <declares>
    When Alice runs her isolated "<engine>" agent "isolated" under workspace "none":
      """
      ctxloom run --one-shot --agent isolated --workspace none "write a short note"
      """
    Then the run reports no isolation finding
    And the agent ran from its own session home
    And Alice's own "<engine>" setup was never touched by the run

    Examples:
      | engine      | declares                      |
      | claude-code | declares engine_home "session" |
      | claude-code | declares no engine_home        |

  # An agent can opt into Alice's own engine setup. It gets exactly that, and
  # ctxloom tells her the choice is unsafe.
  Scenario Outline: An in-tree AGENT run that declares engine_home: host keeps Alice's own real home
    # substitution: the run says the choice is unsafe; no session home is what Alice sees
    Given Alice has a git-backed project
    And Alice has whatever credentials "<engine>" needs to authenticate
    And Alice has trusted the project in her own claude
    And Alice's agent declares engine_home "host"
    When Alice runs her isolated "<engine>" agent "isolated" under workspace "none":
      """
      ctxloom run --one-shot --agent isolated --workspace none "write a short note"
      """
    Then the run reports no isolation finding
    And the run tells Alice it is running on her own engine setup, which is unsafe
    And the agent ran with no session home of its own

    Examples:
      | engine      |
      | claude-code |
