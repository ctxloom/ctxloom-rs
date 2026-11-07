# Ported from journeys/j000700_team_authoring.feature (rev f36e8dfb2)
# Ported from journeys/j001400_bundle_distribution.feature (rev f36e8dfb2)
# NOT ported: Go's "Carol's own active assistant picks up the change she just
#   made" (@future). What a pull must keep of the published tree is in
#   cli/deps.feature; delivery into each workspace and from inside a
#   container is in cli/materialize.feature.
@doc
@pending
Feature: A team's standards, authored once in a repository of their own, reach every project that depends on them

  A team's standards only help if they reach every engineer's assistant the
  same way. Carol, the team lead, writes down how the team works: guidance
  the assistants read, a commit convention they invoke by hand, a review
  skill with a script beside it, an MCP server, a hook, and a profile that
  composes them. She keeps all of it in a repository of its own that holds
  nothing but ctxloom content, no code, so any of the team's projects can
  depend on it.

  The team's project depends on Carol's standards the way it depends on any
  other project: it names her repository as a remote, puts her bundle in a
  profile and pulls it. From then on Bob, working in the team's project, has
  her standards in every session he starts. When Carol changes them, the
  team moves the project onto the new version, and Bob's next pull brings it,
  with the old version gone.

  Background:
    # substitution: told as the user's flow, with the commands she runs.
    Given Carol keeps the team's standards in a repository of their own, "team-standards"
    And Bob works in the team's project

  # Carol authors the standards with ctxloom, in a repository that holds only
  # what ctxloom keeps, and pushes it.
  Scenario: Carol authors the team's standards in a repository that is nothing but ctxloom content
    # new: the depended-on project is a repository of ctxloom content alone.
    # Carol tells the setup interview what this repository is, so her
    # assistant sets it up as content for others rather than as a codebase.
    When Carol sets ctxloom up in "team-standards":
      """
      ctxloom init
      """
    And she tells her assistant in the setup interview:
      """
      This repository is standalone ctxloom content that other projects
      depend on. It has no code: don't configure it for any language,
      toolchain or test runner.
      """
    Then the setup leaves "team-standards" configured as content only, for no language or toolchain
    When Carol authors the team's standards in "team-standards":
      """
      ctxloom bundle create standards -d "How the team works"
      ctxloom fragment create standards#fragments/house-style
      ctxloom command create standards#commands/conventional-commits
      ctxloom skill create standards#skills/review-checklist -d "Reviews a change against the team's checklist"
      """
    And she pushes "team-standards"
    Then Carol sees the house-style fragment, the conventional-commits command and the review-checklist skill in the "standards" bundle
    And "team-standards" holds nothing but its .ctxloom directory

  # The team's project depends on Carol's repository. Every kind of thing she
  # authored reaches Bob's session, unchanged.
  Scenario Outline: Everything Carol authored reaches a session in the project that depends on it
    # new: told as the user's flow.
    Given Carol has authored and pushed the "standards" bundle, carrying a fragment, a command, a skill, an MCP server and a hook
    When Carol has the team's project depend on her standards:
      """
      ctxloom remote create team https://github.com/acme/team-standards
      ctxloom profile edit default --add-bundle team/standards
      ctxloom deps pull
      """
    And she commits it to the project
    And Bob pulls the project
    And Bob fetches what the project depends on and starts a session:
      """
      ctxloom deps pull
      ctxloom run
      """
    Then Bob's session offers the team's <kind> "<name>", as Carol wrote it

    Examples:
      | kind       | name                 |
      | fragment   | house-style          |
      | command    | conventional-commits |
      | skill      | review-checklist     |
      | mcp-server | ledger               |
      | hook       | stamp-edits          |

  # A profile is not something Bob's assistant reads: it chooses what his
  # assistant reads. Carol's profile arrives with her bundle, and Bob can
  # start a session with it.
  Scenario: A profile Carol authored arrives with her bundle, and Bob can start a session with it
    # new: told as the user's flow.
    Given Carol has authored and pushed the "standards" bundle, carrying a "reviewing" profile that selects the review-checklist skill
    And the team's project depends on Carol's standards
    When Bob fetches what the project depends on and starts a session with Carol's profile:
      """
      ctxloom deps pull
      ctxloom run --profile team/standards#profiles/reviewing
      """
    Then Bob's session offers the review-checklist skill the profile selects

  # Carol changes the commit convention. The team moves the project onto the
  # new version, and Bob's next pull brings it; the old one is gone.
  Scenario: Carol changes a command and the change reaches the depending project, not the old version
    # new: told as the user's flow.
    Given the team's project depends on Carol's standards
    And Bob's session already offers Carol's conventional-commits command
    When Carol edits the command in "team-standards":
      """
      ctxloom command edit standards#commands/conventional-commits
      """
    And she pushes "team-standards"
    And Carol moves the team's project onto the new version:
      """
      ctxloom deps upgrade team/standards --yes
      """
    And she commits it to the project
    And Bob pulls the project
    And Bob fetches what the project depends on and starts a session:
      """
      ctxloom deps pull
      ctxloom run
      """
    Then Bob's session offers the updated conventional-commits command
    And it no longer offers the previous version

  # Carol's house style is long; she has ctxloom shorten it, and Bob's
  # assistant gets the short form.
  Scenario: Carol distills verbose guidance and the depending project receives the compact form
    # new: told as the user's flow.
    Given the team's project depends on Carol's standards
    And Carol has authored a verbose house-style fragment in "team-standards"
    When Carol distills the fragment:
      """
      ctxloom fragment distill standards#fragments/house-style
      """
    And she pushes both forms
    And Bob pulls the project with distilled context enabled
    Then Bob's assistant receives the distilled guidance
    And it does not receive the verbose original
