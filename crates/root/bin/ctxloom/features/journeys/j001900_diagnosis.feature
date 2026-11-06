# world: surfaces
# Ported from journeys/j001900_diagnosis.feature
# NOT ported: the rest of the walk (j001900_diagnosis.pending.feature).
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
  far. The walk ends where ctxloom's knowledge ends: whether the engine read
  what it was given is something ctxloom says plainly it cannot know.

  Background:
    Given Alice's team ships its deploy process as ctxloom content

  # Where it starts: Bob's assistant has it and hers does not. She asks
  # ctxloom to compare the two and say what differs.
  Scenario: His assistant has the guidance and hers does not, and something compares the two
    Given Bob's checkout of the same project delivers the deploy guidance and Alice's does not
    When Alice asks ctxloom to compare her delivered context with Bob's
    Then ctxloom reports the deploy guidance as present for Bob and absent for her
