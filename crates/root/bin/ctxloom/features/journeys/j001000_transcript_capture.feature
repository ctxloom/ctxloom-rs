# Ported from journeys/j001000_transcript_capture.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@doc
@pending
Feature: Cross-engine transcript capture — every engine's conversation becomes one record you own

  Alice drives whichever engine she likes, through its own native screen, and
  still keeps a lasting memory of what happened. Each engine keeps its
  conversations in its own private store, in its own format, and that store
  changes or disappears when the engine does.

  ctxloom keeps its own record instead. Whatever engine Alice ran, when the
  session ends its conversation is in ctxloom's record of the session, in one
  form, so reading it back, searching it or resuming it later never depends on
  which engine she happened to use that day.

  Nobody runs an import. The record is there when the session ends, and it is
  brought up to date whenever Alice reads or resumes the session, even one
  that did not end cleanly.

  Background:
    Given an initialized ctxloom project

  # Only a real engine, driven through its own screen, can show this end to
  # end.
  @live
  Scenario Outline: An interactive engine run's own log is captured when it exits
    # substitution: Alice reads the session back through ctxloom, as she would, instead of the record's lines being inspected.
    Given a real <agent> agent is available
    When I run the <agent> agent interactively to completion with prompt "leave a mark"
    Then ctxloom's record of that session holds the conversation she just had
    And the record names "<engine>" as the engine she had it with

    Examples:
      | agent       | engine      |
      | Claude      | claude-code |
