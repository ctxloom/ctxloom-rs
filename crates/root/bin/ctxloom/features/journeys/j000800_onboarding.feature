# Ported from journeys/j000800_onboarding.feature (rev f36e8dfb2)
# NOT ported: "Materializing for an engine that cannot carry hooks says so" is
#   materialize's contract, in cli/materialize.feature.
@doc
@pending
Feature: A new engineer clones the repo and is already set up

  The day a new engineer joins, everything the team has standardized on should
  already be reaching their assistant: not because they followed a setup
  document, but because it travels with the project. Cloning IS the
  onboarding. Bob clones the team's project, fetches what it draws on from
  elsewhere, and the first session he starts through ctxloom carries the
  team's context in its session home, the same context the rest of the team
  has, whichever engine he uses and whichever companion tools his machine
  happens to have.

  Background:
    Given the team's project carries the context Carol has standardized on
    And Bob has just joined and has never used ctxloom before

  Scenario: Bob clones the project and his assistant already has the team's context
    # substitution: Bob starts his session with ctxloom run.
    When Bob clones the project
    And Bob starts a session:
      """
      ctxloom run
      """
    Then his assistant receives the team's standardized context

  Scenario Outline: Bob's engine is not Alice's and the team's context still reaches him natively
    # substitution: Bob starts his session with ctxloom run on his engine.
    When Bob clones the project
    And Bob starts a session on <engine>:
      """
      ctxloom run --engine <engine>
      """
    Then his assistant receives the team's standardized context in <engine>'s own native surface

    Examples:
      | engine      |
      | claude-code |
      | mock        |

  Scenario: Content the project references from another repository reaches Bob on a fresh clone
    # substitution: Bob fetches with ctxloom deps pull and starts his session with ctxloom run.
    Given the project references a bundle published by Trent's company
    When Bob clones the project
    And Bob fetches the context the project draws on:
      """
      ctxloom deps pull
      """
    And Bob starts a session:
      """
      ctxloom run
      """
    Then his assistant receives the company's content
    And his assistant still receives the team's own context, because the project is first-party

  Scenario: Bob receives the versions the team pinned, not the latest ones
    # substitution: Bob fetches with ctxloom deps pull and starts a session, rather than forcing a reinstall.
    Given the project pins the versions of the context it draws from elsewhere
    And an upstream has since published a newer version
    When Bob clones the project
    And Bob fetches the context the project draws on:
      """
      ctxloom deps pull
      """
    And Bob starts a session:
      """
      ctxloom run
      """
    Then he receives the pinned versions, the same ones the rest of the team has
    And he does not receive the newer upstream version

  # Bob's machine has some of the team's companion tools and not others.
  # What does not depend on a missing companion still reaches him, nothing
  # fails, and a companion's own guidance reaches him only when he has it.
  # When he lacks one, his session still starts, and ctxloom warns him which
  # guidance was left out and which companion would bring it.
  Scenario Outline: Companion-dependent guidance reaches Bob only if he has the companion
    # substitution: Bob starts his session with ctxloom run, and is warned about a missing companion.
    Given the team's context includes guidance for the "reprise" companion
    And the "reprise" companion is <presence> on Bob's machine
    When Bob clones the project
    And Bob starts a session:
      """
      ctxloom run
      """
    Then his assistant receives the team's context that does not depend on reprise
    And the reprise-dependent guidance <reaches> his assistant
    And nothing fails because of the companion's presence or absence
    And Bob <is warned> that the reprise-dependent guidance was left out because reprise is not on his machine

    Examples:
      | presence      | reaches        | is warned     |
      | installed     | reaches        | is not warned |
      | not installed | does not reach | is warned     |
