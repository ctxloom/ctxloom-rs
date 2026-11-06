# Ported from cli/search.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@doc
@pending
Feature: search — finding content by name, tag and type, here and upstream

  Covers: `ctxloom search`, in full. It is a SOLO VERB — there are no
  subcommands under it — and everything it does is expressed by one optional
  query argument and four flags: `--tag`, `--type`, `--local` and `--remote`.

  Search answers "what have I got, and what could I have?" in one question.
  Local content is fragments, commands, skills, profiles and MCP servers
  already installed here; remote content is the bundles a configured remote
  publishes and this project has NOT installed. Both halves run against the
  same query and come back in one answer, because the distinction a person
  cares about is what the content IS, not which side of the pull it sits on.

  THREE NARROWINGS, AND EACH IS A CLAIM ABOUT WHAT IS LEFT OUT:

  | flag      | narrows by | leaves out                                     |
  | --type    | kind       | every other kind                               |
  | --tag     | tag        | everything untagged with it — FRAGMENTS ONLY   |
  | --local   | source     | remote repositories                            |
  | --remote  | source     | this project's installed content               |

  A narrowing is untestable by asserting the match appears: a search that
  ignored the flag entirely still returns it. So every scenario below seeds
  something the same query WOULD match, and asserts that the narrowing
  excluded it — with the un-narrowed run in the same fixture, so "excluded" is
  proven capable of failing. Exclusion is pinned by COUNT plus the identity of
  what survived: a total that could only be reached without the excluded item,
  naming what it is, together prove the same fact a membership check on the
  excluded name would — without needing an object-array "does not contain".

  A SEARCH WITH NOTHING TO LOOK FOR IS REFUSED rather than answered. An empty
  query matches everything, and dumping the library in response to a mistyped
  command is the failure mode this refusal exists for.

  Rule: A search must be told what to look for

    # A command that fails writes its error to stderr and stdout stays empty
    # in every encoding, so the refusal itself is prose whichever format is
    # resolved; only the preceding SUCCESSFUL search is tabled.
    Scenario Outline: A search with neither a query nor a tag is refused, not answered with everything
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      # Asked properly, it answers — so the refusal below is a refusal rather
      # than a search that never worked in this fixture.
      When I run "ctxloom search --local testing <flags>"
      Then the command succeeds
      And the output reports "count" as "<result count>"
      When Alice runs the search having typed no query at all:
        """
        ctxloom search --local
        """
      Then the command fails
      And the output contains "please provide a search query or tags"
      And the output does not contain "Results ("

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | result count |
        |               | 1            |
        | --format json | 1            |
        | --format text | Results (1): |

    Scenario Outline: A --type nobody recognizes is refused, and the message names the vocabulary
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When I run "ctxloom search --local --type fragment testing <flags>"
      Then the command succeeds
      And the output reports "count" as "<result count>"
      When I run "ctxloom search --local --type widget testing"
      Then the command fails
      And the output contains "unknown type"
      And the output contains "valid: fragment, command, skill, profile, bundle, mcp_server"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | result count |
        |               | 1            |
        | --format json | 1            |
        | --format text | Results (1): |

  Rule: A name search reaches every kind of local content at once

    # One query, every local kind, grouped by kind in the answer. The fixture
    # seeds a fragment and a command whose names the same query matches, so
    # the result proves the search crossed kinds rather than stopping at the
    # first one it knows about. The structured rows read TYPE off each named
    # result directly — a stronger claim than the rendered grouping headers
    # they replace, which only imply type by which section a name sits under.
    Scenario Outline: One query finds matching content of more than one kind
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a command "testing-review" in bundle "demo" exists
      When Alice looks for everything about testing:
        """
        ctxloom search --local testing <flags>
        """
      Then the command succeeds
      And the output reports "count" as "<result count>"
      And the output reports "local[name=testing].type" as "<names the fragment>"
      And the output reports "local[name=testing-review].type" as "<names the command>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | result count | names the fragment | names the command |
        |               | 2            | fragment            | command            |
        | --format json | 2            | fragment            | command            |
        | --format text | Results (2): | - testing           | - testing-review   |

  Rule: --type narrows to one kind, and that is a claim about exclusion

    # RESTRICT is a claim about what is left OUT, and asserting only that the
    # match appears says nothing about it: a search that ignored --type
    # entirely still returns the fragment. The fixture seeds a command whose
    # name the same query matches; the baseline run (fixed, not tabled — the
    # harness derives its own default off a terminal either way) proves the
    # count would be 2 if nothing were excluded, so the narrowed count of 1
    # naming the survivor is what proves the exclusion.
    Scenario Outline: Restricting to fragments drops the command the same query matched
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a command "testing-review" in bundle "demo" exists
      When I run "ctxloom search --local testing"
      Then the command succeeds
      And the output reports "count" as "2"
      When Alice narrows the same search to fragments:
        """
        ctxloom search --local --type fragment testing <flags>
        """
      Then the command succeeds
      And the output reports "count" as "<result count>"
      And the output reports "local[name=testing].type" as "<the kind that survived>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | result count | the kind that survived |
        |               | 1            | fragment                |
        | --format json | 1            | fragment                |
        | --format text | Results (1): | - testing               |

    # The mirror of the row above, so the flag is shown to select rather than
    # merely to survive one value: the same query, narrowed the other way,
    # keeps the command and drops the fragment.
    Scenario Outline: Restricting to commands drops the fragment the same query matched
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a command "testing-review" in bundle "demo" exists
      When Alice narrows the same search to commands:
        """
        ctxloom search --local --type command testing <flags>
        """
      Then the command succeeds
      And the output reports "count" as "<result count>"
      And the output reports "local[name=testing-review].type" as "<the kind that survived>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | result count | the kind that survived |
        |               | 1            | command                 |
        | --format json | 1            | command                 |
        | --format text | Results (1): | - testing-review        |

    # Profiles are searchable content too — a composition is something a
    # person looks for by name as readily as a fragment is.
    Scenario Outline: Restricting to profiles finds a composition and nothing else
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "dev-notes" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When I run "ctxloom search --local dev"
      Then the command succeeds
      And the output reports "count" as "2"
      When Alice narrows the same search to profiles:
        """
        ctxloom search --local --type profile dev <flags>
        """
      Then the command succeeds
      And the output reports "count" as "<result count>"
      And the output reports "local[name=dev].type" as "<the kind that survived>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | result count | the kind that survived |
        |               | 1            | profile                 |
        | --format json | 1            | profile                 |
        | --format text | Results (1): | - dev                   |

  Rule: A tag search is answered from the tags, not from the name

    # `--tag` matches what an author LABELLED, which is the whole point:
    # content named nothing like the query is still found, and content named
    # exactly like it is still left out. The fixture's built-in "example"
    # fragment (seeded on init, tagged "example") carries the tag; "testing"
    # carries none and is findable by name in the same fixture — which is
    # what makes its absence from the tag search mean something. The narrowed
    # count of 1 naming "example" is what proves "testing" did not survive.
    Scenario Outline: A tag search returns what carries the tag and leaves out what does not
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When I run "ctxloom search --local testing"
      Then the command succeeds
      And the output reports "local[name=testing].type" as "fragment"
      When Alice searches by the label instead of the name:
        """
        ctxloom search --local --tag example <flags>
        """
      Then the command succeeds
      And the output reports "count" as "<result count>"
      And the output reports "local[name=example].tags" containing "<the tag that matched>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | result count | the tag that matched  |
        |               | 1            | example                |
        | --format json | 1            | example                |
        | --format text | Results (1): | - example [example]    |

  Rule: --local and --remote choose the source, and browsing is not installing

    # A remote's catalog is readable without pulling anything: the remote half
    # of a search reports bundles this project has NOT installed, so it can
    # answer "should I install this?" before the lockfile has any opinion.
    # The two scopes are proven against each other in one fixture — a local
    # match and a remote match under the same query — each narrowing's count
    # dropping to the single survivor it names is what proves the other
    # scope's match was actually excluded, not merely unmentioned.
    @remote
    Scenario Outline: One query answers from both sources, and each flag narrows it to one
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And a bundle "notes" exists
      And a fragment "demo-local-note" in bundle "notes" exists
      When Alice asks what she has and what she could have:
        """
        ctxloom search demo <flags>
        """
      Then the command succeeds
      And the output reports "count" as "<both count>"
      And the output reports "local[name=demo-local-note].type" as "<the local proof>"
      When Alice asks only about what is already installed:
        """
        ctxloom search --local demo <flags>
        """
      Then the command succeeds
      And the output reports "count" as "<local-only count>"
      And the output reports "local[name=demo-local-note].type" as "<the local proof>"
      When Alice asks only about what upstream publishes:
        """
        ctxloom search --remote demo <flags>
        """
      Then the command succeeds
      And the output reports "count" as "<remote-only count>"
      And the output reports "remote[name=demo].type" as "<the remote proof>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | both count   | local-only count | remote-only count | the local proof   | the remote proof |
        |               | 2            | 1                 | 1                  | fragment          | bundle           |
        | --format json | 2            | 1                 | 1                  | fragment          | bundle           |
        | --format text | Results (2): | Results (1):      | Results (1):       | - demo-local-note | demo             |

    # `--type bundle` is the one kind that exists ONLY upstream: a bundle is
    # how content is distributed, so narrowing to it narrows to the remote
    # scope without anyone naming a scope. The baseline count of 2 (fixed,
    # not tabled) is what the narrowed count of 1 has to have dropped from.
    @remote
    Scenario Outline: Narrowing to bundles searches upstream alone
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And a bundle "notes" exists
      And a fragment "demo-local-note" in bundle "notes" exists
      When I run "ctxloom search demo"
      Then the command succeeds
      And the output reports "count" as "2"
      When Alice narrows the search to distributable bundles:
        """
        ctxloom search --type bundle demo <flags>
        """
      Then the command succeeds
      And the output reports "count" as "<result count>"
      And the output reports "remote[name=demo].type" as "<the kind that survived>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | result count | the kind that survived |
        |               | 1            | bundle                  |
        | --format json | 1            | bundle                  |
        | --format text | Results (1): | demo                    |

  Rule: A search that matches nothing says so

    "No such content" and "the search never ran" must not look the same, so
    an empty result is reported as one, never as no output at all.

    Scenario: A search that matches nothing says so, rather than printing nothing
      # new: the empty answer, which is the one a diagnosis reaches for first.
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When I run "ctxloom search --local no-such-content-anywhere"
      Then the command succeeds
      And the output reports "count" as "0"
      And the search said, in so many words, that nothing matched
