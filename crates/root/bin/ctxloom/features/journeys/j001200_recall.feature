# Ported from journeys/j001200_recall.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@doc
@pending
Feature: The archaeologist — what did we decide in March?

  Somebody asks why the worktree layout is the way it is. Three people remember
  the conversation. Nobody remembers the reason, and the one person who might
  have written it down has left. The decision was made in a session, in March,
  and the session is still on disk, which is only useful if anyone can find it
  and put it back in front of a model.

  ctxloom keeps a record of every session's conversation, whatever engine held
  it. This journey is the payoff: Alice finds the March session by the
  question she asked in it, reads what it was about, and resumes it so the
  model has that conversation in front of it again. A record that can be kept
  but not recalled is an archive, not a memory.

  Background:
    Given a design question everyone remembers deciding and nobody remembers why

  # She half-remembers the question, not the session's name.
  Scenario: A phrase from the question she asked finds the session that answered it
    # substitution: told as the user's flow, with the commands she runs.
    When I run "ctxloom session search worktree layout"
    Then the search names the March session by a phrase from its first prompt

  # Before she resumes it, she makes sure it is the right one.
  Scenario: Show prints what the session was about
    # substitution: no --format; every scenario is read in both formats.
    When I run "ctxloom session show amber-quiet-heron"
    Then ctxloom prints the question the session was opened with

  # The payoff: the model she asks now has the March conversation in front of
  # it.
  Scenario: Resuming puts the conversation in front of the model
    Given the mock LLM responds "MOCK-REPLY"
    When I run "ctxloom run --one-shot --session amber-quiet-heron -p default Remind me what we concluded."
    Then the command succeeds
    And the mock recorded input contains "J001200-TRANSCRIPT-ONLY-MARKER"

  # A session's name is three random words, and she gets one wrong.
  Scenario: A mistyped harp is refused, naming the harp and where to find the right one
    # new: ctxloom refuses a session name that does not resolve rather than running without it.
    Given the mock LLM responds "MOCK-REPLY"
    When I run "ctxloom run --one-shot --session no-such-harp-anywhere -p default Remind me what we concluded."
    Then ctxloom refuses, naming the harp it could not find and the command that lists the sessions there are
    And no model was asked anything

  # A week later Bob asks the same question, and the March session answers him
  # too.
  Scenario: The next person to ask gets the same conversation back
    # new: recalling a session leaves it for the next person to recall.
    Given the mock LLM responds "MOCK-REPLY"
    And Alice has already resumed the March session
    When I run "ctxloom run --one-shot --session amber-quiet-heron -p default Why is the worktree layout like this?"
    Then the command succeeds
    And the mock recorded input contains "J001200-TRANSCRIPT-ONLY-MARKER"
