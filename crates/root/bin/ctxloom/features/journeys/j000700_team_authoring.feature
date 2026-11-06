# world: surfaces
# Ported from journeys/j000700_team_authoring.feature
# NOT ported: "Carol distills verbose guidance and teammates receive the
#   compact form" (distillation needs an engine to write the compact form;
#   not in this batch) and the @future scenario (Go does not run it either).
#   Ported: the authored command reaching a teammate over a real git pull,
#   and an edit replacing the old version. The command is authored as a file
#   of the bundle's tree (`prompts/<name>.md`), where Go wrote the command
#   into an inline `commands:` field: the same act in the form this port
#   reads. Carol and Bob are two checkouts of a bare origin made in the
#   scenario's temporary directory.
@doc
Feature: A team lead shares a command with the team

  A team's standards only help if they reach every engineer's assistant the same
  way. When the lead writes down how the team works, it should land in every
  teammate's assistant automatically, just by being part of the project.

  Scenario: Carol authors a command and a teammate gains it
    Given Carol is working in the team's project
    When Carol authors a "conventional-commits" command
    And she commits it to the project
    And Bob pulls the project
    Then Bob's assistant can invoke the conventional-commits command

  Scenario: Carol changes a command and the change reaches teammates, not the old version
    Given Bob's assistant already has Carol's conventional-commits command
    When Carol edits the command
    And she commits the change
    And Bob pulls the project again
    Then Bob's assistant has the updated command
    And it no longer has the previous version
