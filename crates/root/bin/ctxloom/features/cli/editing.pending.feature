# Ported from cli/editing.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   editing.feature with its steps (gherkin_lint.test.rs).

@pending
Feature: Editing content
  edit opens content for modification. Flag-driven edits change metadata directly;
  editor-driven edits round-trip the content through the configured editor. With a
  marker editor the change is observable in the bundle file and across MCP.

  What remains here is the ITEM nouns' editor round-trip. The bundle's own
  `edit` (flag-driven metadata and item attachment) moved to
  cli/bundle.feature, and the profile's moved to cli/profile.feature, when
  those per-noun specs took over: each is now asserted alongside the rest of
  its noun's surface rather than beside an unrelated noun's.

  # An edit distills what it saved, and a failed distillation fails the edit
  # while the content still lands raw. The distiller is pinned to a backend
  # that cannot start, so the outcome is the same on every host rather than
  # depending on whether ambient engine credentials happen to be exported;
  # what is asserted is that the change LANDS despite that failure.
  #
  # The agent axis is a SESSION started after the edit: its endpoint serves
  # the package its launch carried, which is the edited content.
  Scenario: Editing a fragment lands the change across axes
    Given a ctxloom project with a marker editor
    And a bundle "demo" exists
    And a fragment "testing" in bundle "demo" exists
    And a profile "dev" with bundle "demo"
    And the mock LLM responds "unused: the backend cannot start"
    And the distillation backend cannot start
    When I run "ctxloom fragment edit demo#fragments/testing"
    Then the command fails
    And the output contains "content saved RAW"
    And the file ".ctxloom/content/bundles/v2/demo/fragments/testing.md" contains "EDITED-BY-TEST"
    Given a session owner is standing on the profile "dev"
    When the agent reads resource "ctxloom://fragments/testing"
    Then the resource contains "EDITED-BY-TEST"

  Scenario: Editing a prompt lands the change in the bundle
    Given a ctxloom project with a marker editor
    And a bundle "demo" exists
    And a command "review" in bundle "demo" exists
    And the mock LLM responds "unused: the backend cannot start"
    And the distillation backend cannot start
    When I run "ctxloom command edit demo#commands/review"
    Then the command fails
    And the output contains "content saved RAW"
    And the file ".ctxloom/content/bundles/v2/demo/prompts/review.md" contains "EDITED-BY-TEST"
