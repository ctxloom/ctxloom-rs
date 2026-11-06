# Ported from cli/fragment.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   fragment.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from cli/fragment.feature`.
#   Left out, not pending: "A hand-authored context file survives materialization byte-for-byte": j000400_multi_engine already carries it (cli/fragment.feature's NOT ported line).

@doc
@pending
Feature: fragment — reusable context units, and the engine surface each one reaches

  A "fragment" is context an assistant just HAS, unasked — as opposed to a
  command (something a person or an assistant explicitly invokes by name) or
  a skill (something an engine discovers and loads on its own by progressive
  disclosure). `ctxloom fragment` is the noun that owns creating, reading,
  searching, and deleting these context units inside a bundle, and it is also
  the noun that owns proving a fragment actually LANDS: every engine reads its
  context from a different file, in a different shape, and a team that
  authors guidance once must see it reach each engine's own surface without
  anyone hand-translating it.

  This is the comprehensive per-noun spec: what the noun DOES, leaf by leaf,
  including the engine-surface matrix that only matters to a machine parsing
  a generated file. The narrative version — Carol's team writing one shared
  profile once and it reaching every engine in its own
  native format — is journeys/j000400_multi_engine.feature, which
  asserts what a PERSON sees; MCP, hooks, and commands are that same
  journey's other three surfaces, and live in cli/mcp.feature,
  cli/manage.feature, and cli/command.feature respectively — this file owns
  only the context surface.

  Rule: A fragment authored once reaches every axis: manifest, listing, and agent resource

    # Every literal on the three axes below is "testing" — the argument the
    # command line just passed. It is the YAML key in the manifest, the name
    # in the listing, and the name in the resource, so a create that stored
    # ZERO BYTES of content left all three green (Content: "" in
    # cli.createItem): this project's characteristic silent no-op, on its
    # primary authoring verb. "Add content here." is the placeholder body
    # create actually writes, and it appears nowhere else in a freshly
    # created project.
    Scenario: Alice creates a fragment and it lands in the manifest, the listing, and the agent's resource
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When Alice adds a fragment to the team's shared guidance:
        """
        ctxloom fragment create demo testing
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/fragments/testing.md" exists
      And the file ".ctxloom/content/bundles/v2/demo/fragments/testing.md" contains "Add content here."
      When I run "ctxloom fragment list"
      Then the output contains "testing"
      When the agent reads resource "ctxloom://fragments"
      Then the resource contains "testing"

  Rule: A fragment's content reaches a person and an agent byte for byte, and is findable by name

    # The MIME type is a static field on the envelope: every MCP resource
    # returning an EMPTY body left this green on the type alone. The body is
    # the resource. The session endpoint serves the fragment as the SESSION's
    # package carries it, so the owner stands on a profile that selects it.
    Scenario: Reading a single fragment over MCP
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And a session owner is standing on the profile "dev"
      When the agent reads resource "ctxloom://fragments/testing"
      Then the resource MIME type is "text/markdown"
      And the resource contains "FRAGMENT-BODY-testing"

    Scenario: Search finds a fragment by name
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When I run "ctxloom search --local testing"
      Then the command succeeds
      And the output contains "testing"

  Rule: Removing a fragment removes it from the manifest and from every listing

    # Bare `remove` is a preview: it must leave the manifest untouched. A
    # guard that quietly destroyed anyway would still pass a scenario that
    # only checked exit code — the manifest-still-contains check is what
    # actually catches that.
    Scenario Outline: Bare fragment remove reports and destroys nothing
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When I run "ctxloom fragment remove demo#fragments/testing <flags>"
      Then the command succeeds
      And the output reports "applied" as "<reports nothing removed>"
      And the output reports "apply" as "<names the apply command>"
      And the file ".ctxloom/content/bundles/v2/demo/fragments/testing.md" contains "FRAGMENT-BODY-testing"

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         | reports nothing removed | names the apply command |
      |               | false                   | ctxloom fragment remove demo#fragments/testing --yes |
      | --format json | false                   | ctxloom fragment remove demo#fragments/testing --yes |
      | --format text | Nothing was removed     | ctxloom fragment remove demo#fragments/testing --yes |


    # Both halves are asserted on payload, separately: a remove that only
    # trimmed the cached listing (leaving the YAML entry behind) and a remove
    # that only trimmed the YAML (leaving a stale listing or a stale MCP
    # resource) would each satisfy exactly one of these three checks alone.
    Scenario: Removing a fragment removes it from the manifest, the listing, and the agent's resource
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When Alice removes it:
        """
        ctxloom fragment remove demo#fragments/testing --yes
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/fragments/testing.md" does not exist
      When I run "ctxloom fragment list"
      Then the output does not contain "testing"
      When the agent reads resource "ctxloom://fragments"
      Then the resource does not contain "testing"
