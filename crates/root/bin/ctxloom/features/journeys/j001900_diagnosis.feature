# world: surfaces
# Ported from journeys/j001900_diagnosis.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending.
@doc
Feature: The day the assistant goes blind

  Monday morning. "The assistant knew our deploy process on Friday. It doesn't
  today." And the worst part of the report is the second half: it is still
  reaching Bob's assistant and not Alice's. Nothing errored. Nothing is red.
  The guidance simply stopped arriving, and Alice has to find out where.

  Content travels a long way to her assistant: written, packaged into a
  bundle, pulled, composed into a profile, delivered to the engine, and read
  by it. Alice walks it back from the symptom, and at every step there is a
  ctxloom command that tells her, out loud, whether the guidance made it that
  far. Each scenario is one Monday, with the guidance lost at one step, and
  the command she asks there names the cause. The walk ends where ctxloom's
  knowledge ends: whether the engine read what it was given is something
  ctxloom says plainly it cannot know.

  Background:
    Given Alice's team ships its deploy process as ctxloom content

  # Where it starts: Bob's assistant has it and hers does not. She asks
  # ctxloom to compare the two and say what differs.
  Scenario: His assistant has the guidance and hers does not, and something compares the two
    Given Bob's checkout of the same project delivers the deploy guidance and Alice's does not
    When Alice asks ctxloom to compare her delivered context with Bob's
    Then ctxloom reports the deploy guidance as present for Bob and absent for her

  # Her first question: what would her assistant be handed right now? Here
  # the answer is reassuring, and the trouble is further along.
  @pending
  Scenario: The dry run shows her the context that would be sent, not just the command line
    # substitution: the flag is spelled `--dry-run`.
    Given the runbook is composed into Alice's profile
    When I run "ctxloom run --dry-run -p default"
    Then the dry run shows her the deploy guidance that would be composed

  # She opens her engine straight in the project, where she materialized her
  # context last week, and the guidance has moved on since. The wiring report
  # says the file she materialized is stale.
  @pending
  Scenario: The composed context moved on and the engine's file did not, and the wiring report says so
    # substitution: the stale file is one Alice materialized into her project herself.
    Given the runbook is composed into Alice's profile
    And Alice materialized her context into the project last week, before the runbook changed
    When I run "ctxloom manage check"
    Then the wiring report names the materialized surface as stale

  # The runbook is installed, but her profile does not compose it. She asks
  # which profile her agent runs, then what that profile composes.
  @pending
  Scenario: The runbook is admitted but in no profile, and profile show says so
    # substitution: one scenario, read in both formats; she asks the agent first, then its profile.
    Given the runbook is installed, but composed into no profile
    When I run "ctxloom agent show default"
    Then the agent listing names the profile it composes
    When I run "ctxloom profile show default"
    Then the profile listing does not name the runbook among its bundles
    And her assistant does not receive the deploy guidance

  # Someone held the runbook at an older version and forgot. The listing says
  # it is held, so a deliberate freeze does not look like a broken pull.
  @pending
  Scenario: The runbook is frozen at an older version, and the listing names the hold
    # substitution: one scenario, read in both formats.
    Given Carol published the runbook, and Alice's assistant receives its deploy guidance
    And Carol publishes a newer runbook while Alice's copy is held
    And Alice syncs on Monday
    When I run "ctxloom bundle list"
    Then the installed-bundle listing names the runbook as held
    And her assistant still receives the older deploy guidance and not the newer

  # The deploy process was written down, but never put in a bundle. The search
  # says it found nothing.
  @pending
  Scenario: Nothing packaged the deploy process, and the search says so
    Given the deploy process exists only as a loose file in Alice's repo, in no bundle
    When I run "ctxloom search deploy"
    Then the search results name no packaged item carrying the deploy process
    And her assistant does not receive the deploy guidance

  # Every step checks out and the assistant is still blind. Did the engine
  # read what it was given? ctxloom says plainly that it cannot know, and
  # never claims the engine read it.
  @pending
  Scenario: Nothing claims the engine read the file, and something says so
    Given every earlier inspector is green and the deploy guidance is materialized into the engine's own surface
    When Alice asks ctxloom what her engine actually ingested
    Then some inspector states plainly that it cannot confirm the engine read what ctxloom delivered
