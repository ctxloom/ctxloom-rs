# world: surfaces
# Ported from journeys/j000800_onboarding.feature
# NOT ported: nothing.
# substitution: "Bob clones the project" is a `git clone` of the team's bare
#   origin made in the scenario's temporary directory; every remote is a
#   local bare repository. "Bob starts a session" is, as in Go, `materialize
#   default` into `out` for claude-code. Go's `mock` engine row runs on this
#   port's `mock`, whose native surface is MOCK_CONTEXT.md. A companion
#   is registered with the command that runs it (nothing is searched for), so
#   "installed" is the stub companion (the compiled
#   `ctxloom-companion-stub`, standing in for reprise: only the shape of the
#   guidance it delivers is asserted, never reprise's wording) registered in
#   Bob's home config with `companion add --command`; "not installed" is
#   `reprise` registered with a command whose program is not there.
@doc
Feature: A new engineer clones the repo and is already set up

  The day a new engineer joins, everything the team has standardized on should
  already be reaching their assistant — not because they followed a setup
  document, but because it travels with the project. Cloning IS the onboarding.
  If it does not work this way, every new hire's assistant behaves differently
  from everyone else's until someone notices and walks them through a checklist,
  which is exactly the drift ctxloom exists to remove.

  Background:
    Given the team's project carries the context Carol has standardized on
    And Bob has just joined and has never used ctxloom before

  # LOCKED — the whole journey in one line: cloning IS the setup. No init
  # interview, no checklist, no copied prompts.
  Scenario: Bob clones the project and his assistant already has the team's context
    When Bob clones the project
    And Bob starts a session
    Then his assistant receives the team's standardized context

  # LOCKED — REPRODUCIBILITY, the point of the lockfile. Bob must get what the
  # TEAM pinned, not whatever the remote is serving today.
  #
  # Bob's fetch here is FORCED deliberately, so the pull runs the fetch-and-
  # record path rather than skipping a reference it considers installed; the
  # assertion reads the installed commit out of Bob's own lockfile.
  Scenario: Bob receives the versions the team pinned, not the latest ones
    Given the project pins the versions of the context it draws from elsewhere
    And an upstream has since published a newer version
    When Bob clones the project
    And Bob fetches the context the project draws on, reinstalling every reference
    Then he receives the pinned versions, the same ones the rest of the team has
    And he does not receive the newer upstream version

  # LOCKED — referenced content arrives on a fresh clone exactly as the team's
  # own does: the remote was registered by the team, and the lockfile pins what
  # it serves, so Bob's fresh machine takes the same content with no second
  # decision. The team's own context flows beside it.
  Scenario: Content the project references from another repository reaches Bob on a fresh clone
    Given the project references a bundle published by Trent's company
    When Bob clones the project
    And Bob fetches the context the project draws on
    And Bob starts a session
    Then his assistant receives the company's content
    And his assistant still receives the team's own context, because the project is first-party

  # LOCKED — GRACEFUL DEGRADATION and its contrast in one place, the case a new
  # machine makes unavoidable: Bob will have some of the team's companion tools and
  # not others. Whatever does not depend on the missing companion must arrive and
  # nothing may fail — a setup that breaks on an absent optional tool is one no new
  # hire can complete. The companion-dependent guidance reaches him ONLY when the
  # companion is present, which is what makes the degradation meaningful rather than
  # silent loss. (A setup with the companion is Bob-on-a-fuller-machine, not a
  # different person — so the Background still holds for both rows.)
  Scenario Outline: Companion-dependent guidance reaches Bob only if he has the companion
    Given the team's context includes guidance for the "reprise" companion
    And the "reprise" companion is <presence> on Bob's machine
    When Bob clones the project
    And Bob starts a session
    Then his assistant receives the team's context that does not depend on reprise
    And the reprise-dependent guidance <reaches> his assistant
    And nothing fails because of the companion's presence or absence

    Examples:
      | presence      | reaches        |
      | installed     | reaches        |
      | not installed | does not reach |


  # LOCKED — hermetic materialization, over the engine axis.
  Scenario Outline: Bob's engine is not Alice's and the team's context still reaches him natively
    When Bob clones the project
    And Bob starts a session on <engine>
    Then his assistant receives the team's standardized context in <engine>'s own native surface

    Examples:
      | engine      |
      | claude-code |
      | mock        |

  # A backend that cannot carry hooks loses the team's guardrail; this asks
  # for the materialize report naming what it could not deliver.
  #
  # It asserts on the RIGHT thing: it is not sensitive to the eventual wording,
  # only to whether the loss is reported at all.
  Scenario: Materializing for an engine that cannot carry hooks says so
    Given Carol's team profile carries a shared fragment, command, MCP server, and hook
    When Alice materializes the team profile for mock-lossy
    Then the materialize report names the hook it could not deliver to mock-lossy
