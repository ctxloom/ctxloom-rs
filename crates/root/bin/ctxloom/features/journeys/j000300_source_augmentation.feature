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
  steps. Alice initializes ctxloom in her project, names the companions she
  uses when it offers to add them, and takes the setup interview it starts: it carries ctxloom's own setup
  guidance plus every companion's, composed together. Nothing replaces
  anything; contributions add up. Her organization gets consistent onboarding
  without her losing ctxloom's baseline or her own preferences.

  Scenario: Installed companions augment the setup interview, they do not replace it
    # substitution: Alice names each companion when init offers, before the interview; the Thens say what the interview carries.
    Given her company ships a companion "acme" carrying the company's onboarding steps
    And her own tooling ships a companion "alice-tools" carrying her setup preferences
    When Alice runs "ctxloom init" at a terminal, answering:
      | question                | answer           |
      | Add companions by name? | acme alice-tools |
    And she takes the setup interview it starts
    Then the setup interview includes ctxloom's own setup guidance
    And it includes the company's onboarding steps
    And it includes her personal setup preferences

  # Alice turns the company's companion off for now, keeping it registered.
  # Her next setup interview goes without the company's steps, and with
  # everything else.
  Scenario: A disabled companion's guidance stays out of the interview, and the others' stays in
    # new: told as the user's flow.
    Given her company ships a companion "acme" carrying the company's onboarding steps
    And her own tooling ships a companion "alice-tools" carrying her setup preferences
    When Alice adds both companions, disables the company's, and takes the setup interview:
      """
      ctxloom companion add acme
      ctxloom companion add alice-tools
      ctxloom companion disable acme
      ctxloom init
      """
    Then the setup interview includes ctxloom's own setup guidance
    And it does not include the company's onboarding steps
    And it includes her personal setup preferences

  # With a real assistant: it acts on what the company's companion asked.
  @live
  Scenario Outline: A real <engine> assistant follows the composed setup guidance
    # substitution: Alice names the company's companion when init offers, then takes the interview with her real assistant.
    Given her company ships a companion "acme" whose setup guidance asks her assistant to confirm a company codeword
    And Alice's assistant is a real <engine> assistant
    When Alice runs "ctxloom init" at a terminal, answering:
      | question                | answer |
      | Add companions by name? | acme   |
    And she takes the setup interview it starts
    Then the assistant's setup response confirms the company codeword

    Examples:
      | engine      |
      | claude-code |
      | opencode    |
