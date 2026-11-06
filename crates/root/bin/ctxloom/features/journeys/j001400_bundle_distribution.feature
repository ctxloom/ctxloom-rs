# Ported from journeys/j001400_bundle_distribution.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending: none has steps yet. What a pull
#   must keep of the published tree is in cli/deps.feature; delivery into
#   each workspace and from inside a container is in cli/materialize.feature.

@doc
@pending
Feature: Publishing a bundle's whole surface, and a consumer receiving it intact

  Trent has written down how his company works, and it is not one kind of
  thing. It is guidance his engineers' assistants read, slash commands they
  invoke by hand, an MCP server that reaches the company ledger, a hook that
  stamps every file edit, a skill package with a script the model runs on its
  own, and the profile that composes all of it. That is one coherent unit of
  work. Trent publishes it in one act, to his company's repository; Alice, one
  of his engineers, references it from her project, pulls it, and every part
  of it reaches her assistant, unchanged.

  Background:
    Given Trent authors a directory-form bundle "atelier" carrying every surface kind

  Scenario Outline: Every surface kind survives publication and reaches the consumer's assistant
    # substitution: the journey asserts what reaches Alice's assistant; the pulled tree's bytes are deps's contract.
    Given Trent publishes the "atelier" tree to his company repo
    When Alice references the company's "atelier" bundle and pulls it
    Then the "<kind>" reaches Alice's assistant carrying the marker "<marker>"

    Examples:
      | kind       | marker                  |
      | fragment   | ATELIER-FRAGMENT-4a91c2 |
      | command    | ATELIER-COMMAND-7d33e1  |
      | mcp-server | ATELIER-MCP-1f88b0      |
      | hook       | ATELIER-HOOK-9c02af     |
      | skill      | ATELIER-SKILL-3e77da    |

  # A profile is not something her assistant reads: it chooses what her
  # assistant reads. Alice receives Trent's profile when she can use it and
  # gets what it chooses.
  Scenario: A published profile arrives and becomes one of Alice's profiles
    # substitution: the Then says what Alice can do with the profile; the pulled file's bytes are deps's contract.
    Given Trent publishes the "atelier" tree to his company repo
    When Alice references the company's "atelier" bundle and pulls it
    Then Alice can use the company's "studio" profile, and her assistant receives the marker "ATELIER-PROFILE-6b41fc" it selects
