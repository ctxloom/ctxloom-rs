# world: surfaces
# Ported from cli/profile.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending.
@doc
Feature: profile — the composition that decides what an agent actually receives

  Covers: `ctxloom profile list` (and its `ls` alias), `profile show`, `profile
  create`, `profile edit`, `profile modify` (the attach/detach and
  include/exclude surface), `profile remove` (and its `rm`/`del` aliases),
  `profile export`, `profile import`, and the bare `ctxloom profile` form,
  with `ctxloom materialize` as the vehicle that delivers a profile at rest.

  A profile is a COMPOSITION. It owns no content of its own: it names bundles
  to draw from, parents to inherit from, and a mask of what to leave out, and
  the result is the context one agent starts with. `.ctxloom/content/bundles/v2/project/profiles/<name>.yaml`
  is the whole of it — which is why every scenario below judges a profile by
  what it ASSEMBLES, not by what its file says it wants.

  THERE IS NO `profile attach` OR `profile include`. Composition is expressed
  as flags on two commands, and the vocabulary is worth learning once:

  | intent                      | how it is spelled                                   |
  | attach a bundle             | `profile create --include <bundle>` / `modify --add-bundle`|
  | detach a bundle             | `profile modify --remove-bundle <bundle>`           |
  | attach a parent             | `profile create --parent <p>` / `modify --add-parent`|
  | detach a parent             | `profile modify --remove-parent <p>`                |
  | mask inherited content out  | `profile modify --exclude-fragment` / `--exclude-mcp`|
  | stop masking it out         | `profile modify --include-fragment` / `--include-mcp`|

  The mask exists because a profile cannot DELETE what it inherits — the
  bundle belongs to whoever published it. `--exclude-*` is a veto applied at
  assembly, and `--include-*` lifts that veto; neither touches the bundle. So
  the only honest test of either is the assembled surface, and that is what
  every scenario in the mask rule reads.

  EXPORT AND MATERIALIZE ARE NOT THE SAME VERB, and confusing them is the
  everyday mistake this noun invites. `export` publishes the DEFINITION — the
  profile YAML, a few lines naming bundles — so another project can import it
  and resolve it against its own installation. `materialize` writes the
  ASSEMBLED SURFACE — CLAUDE.md, .mcp.json, .claude/commands — so a plain
  `claude`, a CI job, or a human can launch into that context with ctxloom out
  of the loop entirely. One is portable and inert; the other is local and
  ready to run. The scenario that pins the difference writes both from one
  profile and reads each other's payload out of neither.

  Which ENGINE each materialized surface is native to, and the per-engine
  matrix that proves one profile reaches all of them, is cli/fragment.feature
  and journeys/j000400_multi_engine.feature. This file owns the composition
  itself.

  @pending
  Rule: A created profile is a real file, and every read surface agrees with it

    # ONE BODY, ONCE PER ENCODING. Off a terminal ctxloom derives JSON, so the
    # no-flag row exercises the DEFAULT every script receives; the explicit
    # rows prove a typed --format wins in both directions. The count claim
    # rides the text column — the payload is a bare array with no length
    # field — while the structured rows address the entry BY PATH.
    Scenario Outline: Creating a profile writes its definition and it is listed back
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When Alice names the context she wants to work in:
        """
        ctxloom profile create dev --include demo -d day-to-day-work
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" exists
      # The composition, not just the name: a create that wrote a profile with
      # an empty bundles list would satisfy a name check and assemble nothing.
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" contains "- demo"
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" contains "day-to-day-work"
      When I run "ctxloom profile list <flags>"
      Then the command succeeds
      And the output reports "$.0.name" as "<the sole entry>"
      And the output reports "$.0.description" as "<names the description>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | the sole entry | names the description |
        |               | dev            | day-to-day-work        |
        | --format json | dev            | day-to-day-work        |
        | --format text | Profiles (1):  | day-to-day-work        |

    # A profile with nothing to draw on has nothing to compose, so creation
    # refuses rather than writing a file that would assemble to nothing later
    # — the silent no-op this project is prone to, moved forward to the one
    # moment a person can still fix it.
    Scenario: A profile that names neither a bundle nor a parent is refused
      Given an initialized ctxloom project
      And a bundle "demo" exists
      # The composed form first, so "no file was written" below is read in a
      # fixture where a create demonstrably DOES write one.
      When I run "ctxloom profile create dev --include demo"
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" exists
      When I run "ctxloom profile create hollow"
      Then the command fails
      And the output contains "at least one parent (--parent) or included bundle (--include) is required"
      And the file ".ctxloom/content/bundles/v2/project/profiles/hollow.yaml" does not exist

    Scenario Outline: Showing a profile reads its composition back
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When Alice checks what the profile is composed of:
        """
        ctxloom profile show dev <flags>
        """
      Then the command succeeds
      And the output reports "name" as "<names the profile>"
      And the output reports "bundles" containing "<the composed bundle>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | names the profile | the composed bundle |
        |               | dev                | demo                 |
        | --format json | dev                | demo                 |
        | --format text | Profile: dev       | - demo               |

    # The bare noun answers rather than teaches, through the same seam
    # `ctxloom remote` and `ctxloom deps` use. The `ls` alias is driven in the
    # same fixture, because an alias that reached a different path would be a
    # second listing nobody is testing.
    Scenario Outline: Bare profile lists the profiles, and so does ls
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When I run "ctxloom profile <flags>"
      Then the command succeeds
      And the output reports "$.0.name" as "<the sole entry>"
      And the output does not contain "Available Commands:"
      When I run "ctxloom profile ls <flags>"
      Then the command succeeds
      And the output reports "$.0.name" as "<the sole entry>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | the sole entry |
        |               | dev            |
        | --format json | dev            |
        | --format text | dev            |

  @pending
  Rule: Composition is by attachment, and detaching takes the content back out

    A bundle or a parent is attached by name and detached by the same name.
    Neither change is observable in the file alone — a profile that lists a
    bundle it cannot resolve looks identical on disk to one that works — so
    every attachment here is read out of the ASSEMBLED context.

    # Both directions in one fixture, and each half read on payload. The
    # second bundle is what makes the detachment falsifiable: a
    # `--remove-bundle` that emptied the profile outright would satisfy the
    # "gone" assertion just as well as a correct one does.
    Scenario: A bundle is attached to a profile and detached again, and the assembled context follows
      Given an initialized ctxloom project
      And a bundle "house-style" exists
      And a fragment "house-rules" in bundle "house-style" exists
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When Alice attaches the team's house style to her profile:
        """
        ctxloom profile modify dev --add-bundle house-style
        """
      Then the command succeeds
      When I run "ctxloom materialize dev --target attached --backend claude-code"
      Then the command succeeds
      And the file "attached/CLAUDE.md" contains "FRAGMENT-BODY-house-rules"
      And the file "attached/CLAUDE.md" contains "FRAGMENT-BODY-testing"
      When Alice detaches it again:
        """
        ctxloom profile modify dev --remove-bundle house-style
        """
      Then the command succeeds
      When I run "ctxloom materialize dev --target detached --backend claude-code"
      Then the command succeeds
      And the file "detached/CLAUDE.md" does not contain "FRAGMENT-BODY-house-rules"
      And the file "detached/CLAUDE.md" contains "FRAGMENT-BODY-testing"

    # Inheritance is the other axis, and the one a team uses to keep a shared
    # base in one place. The child's OWN bundle is asserted throughout, so
    # "the parent's content is gone" is distinguishable from "the profile
    # stopped resolving at all" — which is what a broken detach looks like
    # from the outside.
    Scenario Outline: A parent profile's content is inherited, and detaching the parent takes it away
      Given an initialized ctxloom project
      And a bundle "house-style" exists
      And a fragment "house-rules" in bundle "house-style" exists
      And a profile "base" with bundle "house-style"
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When Alice composes her profile on top of the team's base:
        """
        ctxloom profile create dev --parent base --include demo -d day-to-day-work
        """
      Then the command succeeds
      When I run "ctxloom profile show dev <flags>"
      Then the command succeeds
      And the output reports "parents" containing "<inherits the parent>"
      When I run "ctxloom materialize dev --target inherited --backend claude-code"
      Then the command succeeds
      And the file "inherited/CLAUDE.md" contains "FRAGMENT-BODY-house-rules"
      And the file "inherited/CLAUDE.md" contains "FRAGMENT-BODY-testing"
      When Alice stops inheriting from the base:
        """
        ctxloom profile modify dev --remove-parent base
        """
      Then the command succeeds
      When I run "ctxloom materialize dev --target standalone --backend claude-code"
      Then the command succeeds
      And the file "standalone/CLAUDE.md" does not contain "FRAGMENT-BODY-house-rules"
      And the file "standalone/CLAUDE.md" contains "FRAGMENT-BODY-testing"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | inherits the parent |
        |               | base                 |
        | --format json | base                 |
        | --format text | - base               |

    # A parent that does not exist is a composition that can never assemble.
    # Refusing at creation is friction up front; accepting it would defer the
    # failure to every launch afterwards, where the person who typed the name
    # is no longer looking.
    Scenario: Inheriting from a profile that does not exist is refused at creation
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "base" with bundle "demo"
      # A real parent composes, so the refusal below is about the NAME rather
      # than about `--parent` never having worked in this fixture.
      When I run "ctxloom profile create good --parent base --include demo"
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/project/profiles/good.yaml" exists
      When I run "ctxloom profile create dev --parent no-such-base --include demo"
      Then the command fails
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" does not exist

    # `modify` with nothing to do must not read as a successful change.
    #
    # NOT TABLED. `profile modify` is format debt
    # (format_coverage_test.go's formatDebtAllowlist): it prints this prose
    # regardless of the resolved format, and an EXPLICIT --format json
    # request errors outright rather than rendering one — so there is no
    # json row this scenario could give.
    Scenario: A modify that was told to change nothing says so
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When I run "ctxloom profile modify dev"
      Then the command succeeds
      And the output contains "No changes made."
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" contains "- demo"

  @pending
  Rule: The mask hides inherited content without deleting it

    A profile cannot remove an item from a bundle it does not own, so
    `--exclude-fragment` and `--exclude-mcp` are a VETO applied while the
    context is assembled. `--include-*` lifts the veto. The bundle is untouched
    throughout — which is why both scenarios end by lifting the mask and
    finding the content still there to come back.

    Scenario: A fragment is masked out of the assembled context and let back in
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      # Established first: the fragment really does reach the surface, so the
      # absence asserted next is a mask working rather than a fixture that
      # never delivered anything.
      When I run "ctxloom materialize dev --target before --backend claude-code"
      Then the command succeeds
      And the file "before/CLAUDE.md" contains "FRAGMENT-BODY-testing"
      When Alice masks the inherited fragment out of her own context:
        """
        ctxloom profile modify dev --exclude-fragment testing
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" contains "exclude_fragments"
      When I run "ctxloom materialize dev --target masked --backend claude-code"
      Then the command succeeds
      And the file "masked/CLAUDE.md" does not contain "FRAGMENT-BODY-testing"
      # The rest of the bundle still arrives: the veto took one item, not the
      # attachment.
      And the file "masked/CLAUDE.md" contains "Add your content here."
      # And the bundle was never edited — lifting the mask brings the same
      # bytes straight back, which it could not do if the exclusion had
      # touched the manifest.
      When Alice lifts the mask:
        """
        ctxloom profile modify dev --include-fragment testing
        """
      Then the command succeeds
      When I run "ctxloom materialize dev --target restored --backend claude-code"
      Then the command succeeds
      And the file "restored/CLAUDE.md" contains "FRAGMENT-BODY-testing"

    # The same veto over an EXECUTABLE surface, which is where it matters most:
    # an MCP server a profile excludes must not be launched for that profile.
    # Two servers are declared so the masked run still has a registry to read —
    # "the excluded server is absent" is asserted alongside "the other one is
    # still there", which a materialize that wrote no registry at all could
    # not satisfy.
    Scenario: An MCP server is masked out of the materialized registry and let back in
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And I run "ctxloom bundle edit demo --add-mcp tree-sitter --add-mcp thinker"
      And a profile "dev" with bundle "demo"
      When I run "ctxloom materialize dev --target before --backend claude-code"
      Then the command succeeds
      And the file "before/.mcp.json" registers an MCP server named "tree-sitter"
      And the file "before/.mcp.json" registers an MCP server named "thinker"
      When Alice vetoes one of the servers her bundle ships:
        """
        ctxloom profile modify dev --exclude-mcp tree-sitter
        """
      Then the command succeeds
      When I run "ctxloom materialize dev --target masked --backend claude-code"
      Then the command succeeds
      And the file "masked/.mcp.json" registers no MCP server named "tree-sitter"
      And the file "masked/.mcp.json" registers an MCP server named "thinker"
      When Alice lifts the veto:
        """
        ctxloom profile modify dev --include-mcp tree-sitter
        """
      Then the command succeeds
      When I run "ctxloom materialize dev --target restored --backend claude-code"
      Then the command succeeds
      And the file "restored/.mcp.json" registers an MCP server named "tree-sitter"

  @pending
  Rule: A profile is edited by flag or by editor, and both land in the file

    # A profile is a structured document, so the append-a-line marker editor
    # the item nouns use would leave invalid YAML behind — which is why this
    # scenario once had no observable effect to assert and settled for the exit
    # code, passing even against an edit path that returned immediately and did
    # nothing. The description-rewriting editor keeps the document valid, so
    # the round-trip is assertable on the file AND on the read-back surface.
    #
    # NOT TABLED. `profile edit` is format debt, the same allowlist entry as
    # `profile modify` above: prose only, no json row to give.
    Scenario: Editing a profile in the configured editor round-trips into the file
      Given a ctxloom project with a description-rewriting editor
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When Alice opens the profile in her editor:
        """
        ctxloom profile edit dev
        """
      Then the command succeeds
      And the output contains "Updated profile"
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" contains "EDITED-BY-TEST"
      When I run "ctxloom profile show dev"
      Then the command succeeds
      And the output contains "EDITED-BY-TEST"

    Scenario Outline: The description can be set by flag instead
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" contains "acceptance fixture profile"
      When Alice re-describes the profile without opening an editor:
        """
        ctxloom profile modify dev -d updated-desc
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" contains "updated-desc"
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" does not contain "acceptance fixture profile"
      When I run "ctxloom profile show dev <flags>"
      Then the output reports "description" as "<the new description>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | the new description |
        |               | updated-desc         |
        | --format json | updated-desc         |
        | --format text | updated-desc         |

    # The engine a profile prefers to launch is its own axis, distinct from
    # the description above — a labeled llm config, read back out of the
    # profile file it was written to.
    Scenario: The preferred engine can be set by flag
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And I run "ctxloom llm create big --type mock --model o1"
      When Alice sets which engine this profile prefers to launch:
        """
        ctxloom profile modify dev --llm big
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" contains "llm: big"

  Rule: Export publishes the definition; materialize writes the runnable surface

    # Same shape as the bundle round-trip: a profile export that dropped the
    # bundles: list still imported as a profile named dev and still listed.
    # That list is the profile's entire content, so both ends assert it.
    #
    # `profile remove` is asserted on its EFFECT here (the file is gone) before
    # the re-import, not just on exit code: without that assertion this
    # scenario would pass just as happily against a `remove` that reported and
    # destroyed nothing, since the re-import would recreate the file either
    # way.
    @pending
    Scenario Outline: Export, remove, and re-import a profile
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When Alice exports the profile definition to share it:
        """
        ctxloom profile export dev pexport
        """
      Then the command succeeds
      And the file "pexport/dev.yaml" contains "- demo"
      When I run "ctxloom profile remove dev --yes"
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" does not exist
      When Alice imports it back:
        """
        ctxloom profile import pexport/dev.yaml -f
        """
      Then the command succeeds
      When I run "ctxloom profile list <flags>"
      Then the output reports "$.0.name" as "<the reimported profile>"
      When I run "ctxloom profile show dev <flags>"
      Then the command succeeds
      And the output reports "bundles" containing "<the composed bundle>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | the reimported profile | the composed bundle |
        |               | dev                     | demo                 |
        | --format json | dev                     | demo                 |
        | --format text | dev                     | - demo               |

    # The clobber guard, proven by what SURVIVES the refusal rather than by the
    # error text: a guard that printed the message and overwrote anyway would
    # pass an exit-code check and still have eaten the local edit. The forced
    # retry afterwards proves the guard was the only thing in the way.
    @pending
    Scenario: Importing over an existing profile refuses, and the local edit survives
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And I run "ctxloom profile export dev pexport"
      And I run "ctxloom profile modify dev -d LOCAL-EDIT-MARKER"
      When Alice imports the shared copy over her own:
        """
        ctxloom profile import pexport/dev.yaml
        """
      Then the command fails
      And the output contains "profile already exists"
      And the output contains "use --force to overwrite"
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" contains "LOCAL-EDIT-MARKER"
      When Alice decides she meant it:
        """
        ctxloom profile import pexport/dev.yaml --force
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" does not contain "LOCAL-EDIT-MARKER"

    # THE DISTINCTION THE WHOLE RULE IS NAMED FOR, asserted by writing both
    # from one profile and reading each other's payload out of neither. An
    # export that quietly assembled, or a materialize that quietly copied the
    # definition, would each satisfy a file-exists check on both destinations
    # and be entirely wrong.
    @pending
    Scenario: Export carries the definition and materialize carries the assembled context
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When Alice publishes the definition and stands up a runnable surface:
        """
        ctxloom profile export dev definition
        ctxloom materialize dev --target surface --backend claude-code
        """
      Then the command succeeds
      # The definition NAMES the bundle — the `bundles:` key and the entry
      # under it — and carries none of its content.
      And the file "definition/dev.yaml" contains "bundles:"
      And the file "definition/dev.yaml" contains "- demo"
      And the file "definition/dev.yaml" does not contain "FRAGMENT-BODY-testing"
      # The surface carries the assembled content and is NOT the definition:
      # the fragment's body reaches the engine's own context file, the
      # bundle's command reaches the engine's own command directory, and the
      # `bundles:` key that identifies a definition appears nowhere in it.
      And the file "surface/CLAUDE.md" contains "FRAGMENT-BODY-testing"
      And the file "surface/.claude/commands/demo-example.md" contains "Example prompt content. Describe what this prompt does."
      And the file "surface/CLAUDE.md" does not contain "bundles:"

    # The uncarried surfaces have to reach the PROSE, not only the JSON: the
    # person who needs this line is reading a terminal.
    Scenario: The uncarried surfaces are stated in the prose report, not only in the JSON
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When I run "ctxloom materialize team --target out-launch-text --backend mock-launch --format text"
      Then the command succeeds
      And the materialize report names each surface mock-launch does not carry, with a reason

  @pending
  Rule: Removal reports first and destroys only when told to

    # Bare `remove` is a preview: it must name what it would destroy AND leave
    # the profile in place. A guard that quietly destroyed anyway would still
    # pass a scenario that only checked exit code or the report text — the
    # file-exists check and the follow-up listing are what actually catch that.
    Scenario Outline: Bare profile remove reports what would go and destroys nothing
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When Alice asks what removing the profile would cost:
        """
        ctxloom profile remove dev <flags>
        """
      Then the command succeeds
      And the output reports "applied" as "<nothing was applied>"
      And the output reports "detail" containing "<the cost>"
      And the output reports "apply" as "<the command to actually do it>"
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" exists
      When I run "ctxloom profile list <flags>"
      Then the output reports "$.0.name" as "<the profile survives>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | nothing was applied                              | the cost    | the command to actually do it    | the profile survives |
        |               | false                                             | 1 bundle(s) | ctxloom profile remove dev --yes | dev                   |
        | --format json | false                                             | 1 bundle(s) | ctxloom profile remove dev --yes | dev                   |
        | --format text | Nothing was removed. Re-run with --yes to apply: | 1 bundle(s) | ctxloom profile remove dev --yes | dev                   |

    # NOT TABLED. `profile list`'s payload is a bare array with no length
    # field, so an EMPTY result has no path a structured row could address —
    # the same gap the count claim in the scenario above works around by
    # riding the text column only. Pinned to --format text explicitly, since
    # the derived default off a terminal is JSON and a bare "output contains"
    # would otherwise read the wrong encoding.
    Scenario: Removing a profile with --yes takes the file and the listing entry
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When Alice applies the removal:
        """
        ctxloom profile remove dev --yes
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" does not exist
      When I run "ctxloom profile list --format text"
      Then the output contains "No profiles defined."

    # The aliases are not decoration: `rm` is what a person's fingers type, and
    # an alias that reached a different code path — a different default, a
    # missing preview — would be a destroyer with no guard on it.
    Scenario Outline: The rm alias previews and destroys exactly as remove does
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When I run "ctxloom profile rm dev <flags>"
      Then the command succeeds
      And the output reports "applied" as "<nothing was applied>"
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" exists
      When I run "ctxloom profile del dev --yes"
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/project/profiles/dev.yaml" does not exist

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | nothing was applied                              |
        |               | false                                             |
        | --format json | false                                             |
        | --format text | Nothing was removed. Re-run with --yes to apply: |
