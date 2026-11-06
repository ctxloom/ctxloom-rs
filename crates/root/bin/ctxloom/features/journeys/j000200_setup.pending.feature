# Ported from journeys/j000200_setup.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j000200_setup.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from journeys/j000200_setup.feature`.

@doc
@pending
Feature: Setting up ctxloom on a project

  A developer's assistant is only as good as the context it is handed. Left to
  chance, every engineer's assistant behaves differently and none of them knows
  the team's standards. ctxloom's first job is to make the right context — the
  developer's own, the team's, the company's — actually reach the assistant,
  automatically, on every session, without copying anything by hand. Adding a
  repository is the trust act: what the developer adds is what reaches the
  assistant.

  # HARNESS CAVEAT: the init discovery / agent-setup machinery some of these
  # scenarios exercise may not all be built yet (see the source-augmentation
  # feature and its tasks). If a piece is missing, FILE the gap — do not fake a
  # scenario green or assume it exists.

  # LOCKED — RUNTIME delivery. The setup session CONFIGURES; a RESTART delivers the
  # configured context to a fresh agent (the setup session itself cannot see what
  # it just installed — init.go offerSessionRelaunch). Mock engine, called out.
  Scenario Outline: Setup configures the agents, then a restart delivers their context
    Given Alice has a personal ctxloom repository
    And her company has a ctxloom repository
    When Alice runs the ctxloom setup for <engine>
    And she adds her personal and company repositories as sources
    And the setup interview composes her agents' profiles from the sources' fragments
    And ctxloom offers to restart into her newly configured session
    And Alice accepts the restart
    Then the restarted mock engine receives her personal repository's fragments
    And the restarted mock engine receives her company repository's fragments

    Examples:
      | engine      |
      | claude-code |

  # LOCKED — @live: a REAL assistant, after the restart; proves it can USE the
  # delivered context. Self-skips when the engine is unavailable. One block per
  # engine, tagged with it, so one engine's row is addressable alone.
  @live
  Scenario Outline: The restarted <engine> assistant can see every source
    Given Alice has personal and company ctxloom repositories, served by a real <engine> assistant
    And each source carries a distinct marker phrase
    And Alice has completed setup and restarted into her configured session
    When she asks her assistant to repeat every marker phrase it can see
    Then its reply contains her personal repository's marker
    And its reply contains her company repository's marker

    @claude-code
    Examples:
      | engine |
      | Claude |

    @opencode
    Examples:
      | engine   |
      | Opencode |
