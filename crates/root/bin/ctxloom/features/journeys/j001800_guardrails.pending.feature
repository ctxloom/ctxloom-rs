# Ported from journeys/j001800_guardrails.feature
# NOT ported: any of it yet.

@doc
@pending
Feature: Guardrails — when the assistant does not listen

  An assistant told "use the task runner" will still, sometimes, reach for the
  raw tool out of habit. Guidance can only ask; ltk and reprise, the
  companions Alice installs, act. What each companion then does is its own
  business. ctxloom's part is that the companion's guidance and its guard
  reach the session she starts, and that what reaches her assistant is as
  honest about the companion as the companion is about itself.

  # Alice's assistant is told what ltk really does: it redirects a command to
  # the task runner and says why, and it is not a sandbox.
  Scenario: ltk's own guidance is honest about what it does — a redirect, not a block
    Given Alice's project exists
    And ltk and reprise are installed on Alice's machine
    When Alice starts a session
    Then her assistant is told ltk is a cooperative redirect, not a sandbox

  # In the session ctxloom starts for her, her assistant reaches for a raw
  # shell command, and ltk is asked about it before it runs. The guard lives
  # in the session's own session home: her project has no engine files of
  # ctxloom's in it, before or after.
  Scenario: Alice's guard runs in the session ctxloom started, and her project stays clean
    # new: the guard is seen working in a session, with the project untouched.
    Given Alice's project exists
    And ltk and reprise are installed on Alice's machine
    When Alice starts a session and her assistant runs a raw shell command
    Then ltk was asked about the command before it ran
    And her project holds no engine files of ctxloom's
