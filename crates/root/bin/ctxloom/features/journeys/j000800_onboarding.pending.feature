# Ported from journeys/j000800_onboarding.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j000800_onboarding.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from journeys/j000800_onboarding.feature`.

@doc
@pending
Feature: A new engineer clones the repo and is already set up

  The day a new engineer joins, everything the team has standardized on should
  already be reaching their assistant — not because they followed a setup
  document, but because it travels with the project. Cloning IS the onboarding.
  If it does not work this way, every new hire's assistant behaves differently
  from everyone else's until someone notices and walks them through a checklist,
  which is exactly the drift ctxloom exists to remove.

  Two things must hold that are easy to get wrong. The new engineer must receive
  the SAME context as the rest of the team, not merely the newest thing the
  remotes happen to be serving that morning — otherwise "we all use the same
  standards" is a hope, not a fact. And their machine is not a clone of anyone
  else's: they will have some of the team's companion tools installed and not
  others, and the context that does not depend on the missing ones must still
  arrive intact.

  # NOTE ON TRUST: adding a git repository is the trust act, and the team made it
  # once, when it registered the remote the project draws on. What Bob inherits
  # by cloning is the project's own committed context and the content its
  # lockfile pins from those remotes — nothing is asked of him a second time.

  Background:
    Given the team's project carries the context Carol has standardized on
    And Bob has just joined and has never used ctxloom before

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
