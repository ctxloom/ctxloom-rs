# Ported from journeys/j000300_source_augmentation.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending: none has steps yet. Go's "An
#   installed companion augments the setup interview" and its @live twin are
#   the same flow as the first scenario and the @live outline here, and are
#   merged into them.

@doc
@pending
Feature: Companions shape how a project is set up

  Setting up a project is not one-size-fits-all. A company standardizes how its
  projects are configured; a developer's own tooling carries its own setup
  steps. Alice initializes ctxloom in her project, adds the companions she
  uses, and runs ctxloom's setup interview: it carries ctxloom's own setup
  guidance plus every companion's, composed together. Nothing replaces
  anything; contributions add up. Her organization gets consistent onboarding
  without her losing ctxloom's baseline or her own preferences.

  Scenario: Installed companions augment the setup interview, they do not replace it
    # substitution: Alice adds each companion herself before the interview; the Thens say what the interview carries.
    Given her company ships a companion carrying the company's onboarding steps
    And her own tooling ships a companion carrying her setup preferences
    When Alice initializes ctxloom in her project
    And she adds her company's companion to her project
    And she adds her own tooling's companion to her project
    And she runs ctxloom's setup interview
    Then the setup interview includes ctxloom's own setup guidance
    And it includes the company's onboarding steps
    And it includes her personal setup preferences

  # For one setup, Alice wants ctxloom's own guidance and nothing else.
  Scenario: Switching companions off for one setup keeps their guidance out of the interview
    # substitution: as the scenario above, with the interview run with companions switched off.
    Given her company ships a companion carrying the company's onboarding steps
    And her own tooling ships a companion carrying her setup preferences
    When Alice initializes ctxloom in her project
    And she adds her company's companion to her project
    And she adds her own tooling's companion to her project
    And she runs ctxloom's setup interview with companions switched off
    Then the setup interview includes ctxloom's own setup guidance
    And it does not include the company's onboarding steps
    And it does not include her personal setup preferences

  # With a real assistant: it acts on what the company's companion asked.
  @live
  Scenario Outline: A real <engine> assistant follows the composed setup guidance
    # substitution: Alice adds the company's companion herself, then runs the interview with her real assistant.
    Given her company ships a companion whose setup guidance asks her assistant to confirm a company codeword
    When Alice initializes ctxloom in her project
    And she adds her company's companion to her project
    And she runs ctxloom's setup interview with her real <engine> assistant
    Then the assistant's setup response confirms the company codeword

    @claude-code
    Examples:
      | engine |
      | Claude |

    @opencode
    Examples:
      | engine   |
      | Opencode |
