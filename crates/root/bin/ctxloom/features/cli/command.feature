# world: surfaces
# Ported from cli/command.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending: create, the MCP resource reads
#   (`the agent reads resource` has no MCP session in this port), remove, the
#   --no-distill edit and distill refusals (they need an editor and the
#   distiller), and the misspelled --bundle filter.
@doc
Feature: command — authoring reusable prompt templates for AI coding assistants

  A "command" is a user-invoked slash template: something a person or an
  assistant explicitly runs by name, as opposed to a fragment (context an
  assistant just has, unasked) or a skill (something an engine discovers and
  loads on its own by progressive disclosure). `ctxloom command` is the noun
  that owns creating, reading, editing, distilling, and deleting these
  templates inside a bundle.

  This is the comprehensive per-noun spec: what the noun DOES, leaf by leaf,
  including the refusals and the shapes that only matter to a machine. The
  narrative version — Carol authoring a team standard once and it reaching
  every teammate's assistant untouched — is
  journeys/j000700_team_authoring.feature, which asserts what a PERSON sees.

  # WHERE THE BOUNDARY FALLS. WHICH engine-native file a materialized command
  # lands in (.claude/commands/, $CODEX_HOME/prompts/, a SKILL.md package for
  # the engines with no slash-command idiom of their own) is per-engine
  # knowledge this noun owns by SUBJECT, even though the command that writes
  # it is `ctxloom materialize` — so the full three-engine matrix is built
  # HERE, never sampled down to one engine, which would prove strictly less.
  #
  # cli/manage.feature's own "shipped commands in its own idiom" scenario
  # covers a DIFFERENT claim through the same renderer: that `manage install`
  # ships ctxloom's own first-party "discover" command, to engines this
  # outline does not reach. Built-in content via
  # install, a team's own authored content via materialize — two claims, not
  # one doubled.

  Rule: A materialized command lands in each engine's own native surface

    # ONE command, PARSED in its own native shape, never a bare file-exists.
    Scenario Outline: A team's command lands in each engine's own command surface
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When Alice materializes the team profile for <engine>
      Then the materialized <engine> command file carries the shared command's body, in its own native shape

      Examples:
        | engine      |
        | claude-code |

  @pending
  Rule: Creating a command writes real content, and it reaches every surface

    # Same hazard as fragment.feature's create scenario: "review" is the
    # argument the command line passed, and it is the YAML key, the listed
    # name, and the resource entry alike — so `Content: ""` in cli.createItem
    # stored no body at all and every axis stayed green regardless.
    # "Add content here." is the placeholder body create actually writes, and
    # it is the only string here that proves real bytes landed rather than a
    # name being echoed back three times.
    Scenario: Alice creates a command and it is listed and exposed to the agent
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When Alice creates a command for her team:
        """
        ctxloom command create demo review
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/prompts/review.md" exists
      And the file ".ctxloom/content/bundles/v2/demo/prompts/review.md" contains "Add content here."
      When I run "ctxloom command list"
      Then the output contains "review"
      When the agent reads resource "ctxloom://commands"
      Then the resource contains "review"

  Rule: A command's content reaches a person and an agent byte for byte

    # `show` prints the item's name as a header, echoed straight from the
    # argument: the body marker exists only inside the command's own bytes.
    Scenario: Showing a command's content over the CLI
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a command "review" in bundle "demo" exists
      When Alice reads it herself:
        """
        ctxloom command show demo#commands/review
        """
      Then the command succeeds
      And the output contains "review"
      And the output contains "COMMAND-BODY-review"

    # The MIME type is a static field on the envelope: every MCP resource
    # returning an EMPTY body left this green on the type alone. The body is
    # the resource. The session endpoint serves the command as the SESSION's
    # package carries it, so the owner stands on a profile that selects it.
    @pending
    Scenario: Reading a single command over MCP
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a command "review" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And a session owner is standing on the profile "dev"
      When the agent reads resource "ctxloom://commands/review"
      Then the resource MIME type is "text/markdown"
      And the resource contains "COMMAND-BODY-review"

  @pending
  Rule: Removing a command removes it from the manifest and from every listing

    # Bare `remove` is a preview: it must leave the manifest untouched. A
    # guard that quietly destroyed anyway would still pass a scenario that
    # only checked exit code — the manifest-still-contains check is what
    # actually catches that.
    Scenario Outline: Bare command remove reports and destroys nothing
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a command "review" in bundle "demo" exists
      When I run "ctxloom command remove demo#commands/review <flags>"
      Then the command succeeds
      And the output reports "applied" as "<reports nothing removed>"
      And the output reports "apply" as "<names the apply command>"
      And the file ".ctxloom/content/bundles/v2/demo/prompts/review.md" contains "COMMAND-BODY-review"

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         | reports nothing removed | names the apply command |
      |               | false                   | ctxloom command remove demo#commands/review --yes |
      | --format json | false                   | ctxloom command remove demo#commands/review --yes |
      | --format text | Nothing was removed     | ctxloom command remove demo#commands/review --yes |


    # Both halves are asserted on payload, separately: a remove that only
    # trimmed the cached listing (leaving the YAML entry behind) and a remove
    # that only trimmed the YAML (leaving a stale listing) would each satisfy
    # exactly one of these three checks alone.
    Scenario: Removing a command removes it from the manifest, the listing, and the agent's resource
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a command "review" in bundle "demo" exists
      When Alice removes it:
        """
        ctxloom command remove demo#commands/review --yes
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/prompts/review.md" does not exist
      When I run "ctxloom command list"
      Then the output does not contain "review"
      When the agent reads resource "ctxloom://commands"
      Then the resource does not contain "review"

  @pending
  Rule: --no-distill controls whether an edit gets re-distilled, honestly

    # --no-distill is a per-invocation opt-out, distinct from the item's own
    # persistent no_distill field: it skips calling the LLM for THIS edit and
    # leaves the distilled form EMPTY rather than stale (SetItemContent clears
    # Distilled/DistilledBy/ContentHash the instant raw content changes,
    # before it even decides whether to redistill). A warning is the only
    # place that fact is visible to the author — a silent skip would read
    # identically to "already up to date and nothing to do".
    Scenario Outline: Editing with --no-distill warns instead of leaving a stale distillation unmentioned
      Given a ctxloom project with a marker editor
      And a bundle "demo" exists
      And a command "review" in bundle "demo" exists
      When Alice edits it without redistilling:
        """
        ctxloom command edit demo#commands/review --no-distill <flags>
        """
      Then the command succeeds
      And the output reports "distilled" as "<says the distillation was not refreshed>"
      And the file ".ctxloom/content/bundles/v2/demo/prompts/review.md" contains "EDITED-BY-TEST"

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         | says the distillation was not refreshed  |
      |               | false                                    |
      | --format json | false                                    |
      | --format text | distilled form not refreshed (--no-distill) |

    # editInEditor returns whatever bytes are on disk with no length check, so
    # a crashed editor, a truncated write, or a wrapper script that never
    # actually saved used to replace the command's ENTIRE body with nothing
    # and still print "Updated command ...". An intentional blank is not a
    # thing anyone does through this path; losing a command's content to a
    # wrapper that silently failed very much is.
    Scenario: An editor that returns nothing is refused rather than emptying the command
      Given a ctxloom project with an editor that empties the file
      And a bundle "demo" exists
      And a command "review" in bundle "demo" exists
      When she edits it and the editor comes back empty:
        """
        ctxloom command edit demo#commands/review
        """
      Then the command fails
      And the output contains "refusing to overwrite"
      And the file ".ctxloom/content/bundles/v2/demo/prompts/review.md" contains "COMMAND-BODY-review"

  @pending
  Rule: A command already marked no_distill is reported, not silently skipped

    # no_distill is checked BEFORE the distiller-availability check, so this
    # answer is the same whether or not an LLM is even configured — the
    # author's own declaration wins first, and distill says so rather than
    # exiting 0 with no explanation of which of the two reasons applied.
    Scenario Outline: Distilling a command marked no_distill reports why it did nothing
      Given an initialized ctxloom project
      And the project already has the bundle "demo":
        """
        version: "1.0.0"
        commands:
          review:
            no_distill: true
            content: "Review guidance that is never auto-summarized."
        """
      When Alice tries to distill it anyway:
        """
        ctxloom command distill demo#commands/review <flags>
        """
      Then the command succeeds
      And the output reports "reason" as "<names no_distill as the reason>"

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         | names no_distill as the reason |
      |               | no_distill                     |
      | --format json | no_distill                     |
      | --format text | is marked as no_distill        |

  @pending
  Rule: A --bundle filter that names nothing is refused, not answered empty

    # The UNFILTERED listing is non-empty whenever any command exists
    # anywhere, so a --bundle typo used to fall through to "Commands (0):" —
    # a message indistinguishable from a bundle that legitimately holds none.
    Scenario: Listing commands with a misspelled --bundle is refused, not answered empty
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a command "review" in bundle "demo" exists
      When I run "ctxloom command list --bundle nosuchbundle"
      Then the command fails
      And the output contains "no bundle named"
      And the output contains "nosuchbundle"
