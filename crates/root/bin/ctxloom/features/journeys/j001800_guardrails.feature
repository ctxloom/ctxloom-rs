# world: surfaces
# Ported from journeys/j001800_guardrails.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending.
@doc
Feature: Guardrails — when the assistant does not listen

  An assistant told "use the task runner" will still, sometimes, reach for the
  raw tool out of habit. One told to check for an existing helper before
  writing a new one will still, sometimes, write a fourth copy. Guidance can
  only ask.

  So Alice installs two companions, ltk and reprise, that do mechanically what
  guidance can only request. ctxloom's part is delivery: when Alice starts a
  session, each companion's guidance reaches her assistant, and ltk is set to
  run on every shell command and file edit her assistant makes. What reaches
  her assistant is as honest about each companion as the companion is about
  itself. Bob, on the same team, has neither companion, and his sessions start
  just the same, with the team's own context.

  # Alice has both companions.
  Scenario: Alice's assistant receives the task-runner and reuse-before-you-write guidance
    Given Alice's project exists
    And ltk and reprise are installed on Alice's machine
    When Alice starts a session
    Then her assistant receives ltk's task-runner guidance
    And her assistant receives reprise's reuse-before-you-write guidance
    And her assistant's engine is configured to run ltk on every shell command and file edit

  # Alice's assistant is told what ltk really does: it redirects a command to
  # the task runner and says why, and it is not a sandbox.
  @pending
  Scenario: ltk's own guidance is honest about what it does — a redirect, not a block
    Given Alice's project exists
    And ltk and reprise are installed on Alice's machine
    When Alice starts a session
    Then her assistant is told ltk is a cooperative redirect, not a sandbox

  # Bob has neither, and nothing about his session depends on them.
  Scenario: Bob, without either companion installed, still gets the team's context and nothing fails
    Given the team's project carries the context Carol has standardized on
    And Bob clones the project
    And the "reprise" companion is not installed on Bob's machine
    And the "ltk" companion is not installed on Bob's machine
    When Bob starts a session
    Then his assistant receives the team's standardized context
    And nothing fails because of the companion's presence or absence

  # In the session ctxloom starts for her, her assistant reaches for a raw
  # shell command, and ltk is asked about it before it runs. The guard lives
  # in the session's own session home: her project has no engine files of
  # ctxloom's in it, before or after.
  @pending
  Scenario: Alice's guard runs in the session ctxloom started, and her project stays clean
    # new: the guard is seen working in a session, with the project untouched.
    Given Alice's project exists
    And ltk and reprise are installed on Alice's machine
    When Alice starts a session and her assistant runs a raw shell command
    Then ltk was asked about the command before it ran
    And her project holds no engine files of ctxloom's
