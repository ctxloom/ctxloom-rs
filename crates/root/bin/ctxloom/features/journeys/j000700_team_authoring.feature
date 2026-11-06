# world: surfaces
# Ported from journeys/j000700_team_authoring.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending: distillation needs an engine to
#   write the compact form, and the @future one waits on each engine's
#   behaviour.
@doc
Feature: A team lead shares a command with the team

  A team's standards only help if they reach every engineer's assistant the
  same way. When Carol, the team lead, writes down how the team works (a commit
  convention, a review checklist, a house pattern), it lands in every
  teammate's assistant just by being part of the project. No one copies
  prompts around: Carol authors it once, commits it, and Bob has it the next
  time he pulls and starts a session.

  Scenario: Carol authors a command and a teammate gains it
    Given Carol is working in the team's project
    When Carol authors a "conventional-commits" command
    And she commits it to the project
    And Bob pulls the project
    Then Bob's assistant can invoke the conventional-commits command

  # Carol's guidance is long; she has ctxloom shorten it, and the team's
  # assistants get the short form.
  @pending
  Scenario: Carol distills verbose guidance and teammates receive the compact form
    Given Carol has authored a verbose fragment in the project
    When Carol distills the fragment
    And she commits both forms
    And Bob pulls the project with distilled context enabled
    Then Bob's assistant receives the distilled guidance
    And it does not receive the verbose original

  Scenario: Carol changes a command and the change reaches teammates, not the old version
    Given Bob's assistant already has Carol's conventional-commits command
    When Carol edits the command
    And she commits the change
    And Bob pulls the project again
    Then Bob's assistant has the updated command
    And it no longer has the previous version

  # Whether Carol's own open session sees her change at once depends on the
  # engine.
  @pending @future
  Scenario: Carol's own active assistant picks up the change she just made
    Given Carol has an active session in the project
    When Carol edits a fragment
    Then her own assistant reflects the change without her copying anything
