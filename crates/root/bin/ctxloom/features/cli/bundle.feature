# Ported from cli/bundle.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@doc
@pending
Feature: bundle — the container authored content lives in, and everything that reads or moves one

  Covers: `ctxloom bundle list`, `bundle show`, `bundle view` (including the
  `#path` payload read), `bundle create`, `bundle edit`, `bundle remove` (and
  its `rm`/`del` aliases), `bundle export`, `bundle import`, `bundle push`,
  `bundle move`, and the bare `ctxloom bundle` form.

  A bundle is a CONTAINER. One directory under `.ctxloom/content/bundles/v2/`
  holds fragments, commands, MCP servers, skills and profiles together — its
  bundle.yaml envelope beside a file per item — and that tree is the unit
  everything else in ctxloom addresses: a profile names a
  bundle, the lockfile pins a bundle, and every
  item ref in the system is spelled `<bundle>#<kind>/<name>`. Nothing here
  reaches the network — authoring a bundle is a local file edit, and that is
  what makes every verb below safe to run offline.

  THE CONTAINER IS NOT THE CONTENTS, and the difference decides which command
  answers a question. `list` summarizes every bundle by count. `show` renders
  one bundle's structure — the sections, the tags, the distill markers — but
  never an item's body. `view` is the only verb that hands back BYTES, and the
  `#path` suffix is how it drills from the container into one item. A reader
  looking for a fragment's text who reaches for `show` gets a header and a
  70-character preview, which is the single most common wrong turn on this
  noun.

  SOME OF THIS NOUN'S LEAVES BELONG TO MECHANISMS BIGGER THAN THE CONTAINER,
  and are specified where that mechanism is, rather than duplicated here:

  | leaf                               | specified in                     |
  | bundle distill                     | cli/content_distill.feature      |

  It is a state machine that spans more than one noun — a distillation that
  also governs `fragment distill` and `command distill` — and splitting it
  across two files would leave neither able to assert the transition that
  matters.

  `bundle push` publishes to any git remote. A plain git remote is reached
  through the user's own git, so a file:// repository is a complete hermetic
  destination, and this file publishes to one.

  Where content comes FROM is a different noun (cli/remote.feature); what this
  project has INSTALLED from there is a third (cli/deps.feature).

  Rule: A created bundle is a real file with real content in it

    `bundle create` seeds a skeleton — one example fragment, one example
    command — so a new bundle is editable rather than empty. Every assertion
    below reads the FILE, because "Created bundle: …" is printed from the
    argument and is equally true of a create that wrote zero bytes.

    Scenario: Creating a bundle writes a manifest carrying its seeded content
      Given an initialized ctxloom project
      When Alice starts a bundle for the team's shared guidance:
        """
        ctxloom bundle create demo -d acceptance-fixture-bundle
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" exists
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" is valid YAML
      # The seeded bodies, not the name: a create that wrote only the scalar
      # header would satisfy a name check and leave an unusable skeleton.
      And the file ".ctxloom/content/bundles/v2/demo/fragments/example.md" contains "# Example Fragment"
      And the file ".ctxloom/content/bundles/v2/demo/prompts/example.md" contains "Example prompt content. Describe what this prompt does."
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" contains "acceptance-fixture-bundle"
      When I run "ctxloom bundle list"
      Then the command succeeds
      And the output contains "demo"

    # The listing's whole job is the summary: name, version, and what is
    # inside. "the output contains demo" is satisfied by the name line alone,
    # which a renderer that lost every count would still print — so the counts
    # are asserted, and asserted again after the manifest changes underneath
    # them, so a hard-coded summary cannot pass twice. `bundle list`'s payload
    # is a bare array of bundle objects, so cardinality is read off the array
    # itself rather than a "count" field neither format has here.
    Scenario Outline: The listing summarizes each bundle by version and by what it holds
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When Alice asks what this project has authored:
        """
        ctxloom bundle list --no-companions <flags>
        """
      Then the command succeeds
      And the output reports "$" having "<bundle count>" entries
      And the output reports "[name=demo].version" as "<the version>"
      And the output reports "[name=demo].fragment_count" as "<the fragment count>"
      And the output reports "[name=demo].command_count" as "<the command count>"
      # Move the manifest and the summary has to move with it.
      When I run "ctxloom bundle edit demo --version 2.0.0 --add-fragment testing"
      Then the command succeeds
      When I run "ctxloom bundle list <flags>"
      Then the output reports "[name=demo].version" as "<the version after edit>"
      And the output reports "[name=demo].fragment_count" as "<the fragment count after edit>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | bundle count            | the version   | the fragment count    | the command count | the version after edit | the fragment count after edit |
        |               | 1                        | 1.0.0         | 1                      | 1                  | 2.0.0                   | 2                              |
        | --format json | 1                        | 1.0.0         | 1                      | 1                  | 2.0.0                   | 2                              |
        | --format text | Installed bundles (1):  | demo (v1.0.0) | Contains: 1 fragment  | 1 command          | demo (v2.0.0)           | Contains: 2 fragments          |

    # The bare noun answers the question somebody typing it has, through the
    # same seam `ctxloom remote` and `ctxloom deps` use.
    Scenario Outline: Bare bundle lists what is installed
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When I run "ctxloom bundle --no-companions <flags>"
      Then the command succeeds
      And the output reports "$" having "<bundle count>" entries
      And the output reports "[name=demo].version" as "<the version>"
      And the output does not contain "Available Commands:"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | bundle count           | the version   |
        |               | 1                       | 1.0.0         |
        | --format json | 1                       | 1.0.0         |
        | --format text | Installed bundles (1): | demo (v1.0.0) |

  Rule: Show renders the container's structure, never an item's body

    `show` is the map: which sections exist, how many entries each holds, what
    each entry is tagged, and whether it carries a distilled rendering. It
    deliberately stops at a first-line preview of a fragment — reading an
    item's bytes is `view`'s job, and the two are separated so a `show` cannot
    quietly become a way to dump a whole bundle's content.

    # "the output contains demo" is satisfied by the `Bundle: demo` header
    # alone, so a render that stopped right after the header — suppressing
    # every section, which IS the bundle's structure — passed. The sections
    # this scenario is named after are what it now reads. `show`'s payload has
    # no "count" field of its own for either section, so cardinality is read
    # off each map directly (the structural analogue of "Fragments (N):"),
    # and each fragment is also named individually — the map's cardinality
    # alone would not prove WHICH two fragments are in it.
    Scenario Outline: Showing a bundle names every section it carries
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When Alice looks at what the bundle is made of:
        """
        ctxloom bundle show demo <flags>
        """
      Then the command succeeds
      And the output reports "name" as "<names the bundle>"
      And the output reports "description" as "<the description>"
      And the output reports "fragments" having "<fragment count>" entries
      And the output reports "fragments.example.no_distill" as "<the seeded fragment's distill marker>"
      And the output reports "fragments.testing.no_distill" as "<the added fragment's distill marker>"
      And the output reports "commands" having "<command count>" entries
      # The command's own authored description, stored in the manifest and
      # printed from it — not a word the command line ever passed in.
      And the output reports "commands.example.description" as "<the command's description>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | names the bundle | the description                   | fragment count   | the seeded fragment's distill marker | the added fragment's distill marker | command count  | the command's description |
        |               | demo              | acceptance fixture bundle          | 2                 | true                                  | false                                | 1               | Example prompt             |
        | --format json | demo              | acceptance fixture bundle          | 2                 | true                                  | false                                | 1               | Example prompt             |
        | --format text | Bundle: demo      | Description: acceptance fixture bundle | Fragments (2): | - example [example] (no_distill)      | - testing                            | Commands (1):   | Example prompt             |

    # THE ONE WRONG TURN THIS NOUN INVITES, pinned so it stays a documented
    # boundary rather than an accident. `show` takes a bundle NAME and nothing
    # else: the `#path` selector is `view`'s grammar, and passing one to `show`
    # is a lookup for a bundle whose name happens to contain a `#`, which
    # cannot exist. The refusal is the correct behaviour and the message names
    # the whole argument back, so a reader can see what was actually looked up.
    #
    # The positive case runs FIRST, in the same fixture, so "cannot be read
    # this way" is read off a bundle that demonstrably CAN be read.
    Scenario Outline: Show does not take the item selector — that grammar belongs to view
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When I run "ctxloom bundle show demo <flags>"
      Then the command succeeds
      And the output reports "fragments" having "<fragment count>" entries
      When Alice tries to drill into an item the way view lets her:
        """
        ctxloom bundle show demo#fragments/testing
        """
      Then the command fails
      And the output contains "bundle not found: demo#fragments/testing"
      When I run "ctxloom bundle view demo#fragments/testing"
      Then the command succeeds
      And the output contains "FRAGMENT-BODY-testing"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | fragment count |
        |               | 2              |
        | --format json | 2              |
        | --format text | Fragments (2): |

  Rule: View hands back bytes, and `#path` drills from the container into one item

    Without a path, `view` dumps the bundle's stored YAML verbatim. With one,
    it returns exactly that item's content and nothing else — which is what
    makes it the payload read, and what makes an assertion on it capable of
    catching a renderer that emits structure without content.

    # `bundle view <name>` dumps the whole bundle YAML, and "testing" is the
    # fragment's YAML KEY — so re-marshalling through a struct that omits each
    # item's content left this green while view showed nothing of the fragment
    # it is named after. The marker exists only inside the fragment's stored
    # bytes.
    Scenario: Viewing a bundle dumps the manifest with its items' bodies in it
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When Alice reads the bundle as it is stored:
        """
        ctxloom bundle view demo
        """
      Then the command succeeds
      And the output contains "testing"
      And the output contains "FRAGMENT-BODY-testing"
      And the output contains "Example prompt content. Describe what this prompt does."

    # Drilling in is a claim about what is left OUT as much as what comes back.
    # A `view` that ignored the selector and dumped the whole manifest would
    # satisfy every "contains" assertion here; the command body's absence is
    # what proves the selector narrowed anything, and the whole-bundle read
    # above it is what proves that body was there to be excluded.
    Scenario: A path selector returns one item's bytes and leaves the rest behind
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When I run "ctxloom bundle view demo"
      Then the output contains "FRAGMENT-BODY-testing"
      And the output contains "Example prompt content. Describe what this prompt does."
      When Alice drills into the fragment alone:
        """
        ctxloom bundle view demo#fragments/testing
        """
      Then the command succeeds
      And the output contains "FRAGMENT-BODY-testing"
      And the output does not contain "Example prompt content. Describe what this prompt does."
      When Alice drills into the command instead:
        """
        ctxloom bundle view demo#commands/example
        """
      Then the command succeeds
      And the output contains "Example prompt content. Describe what this prompt does."
      And the output does not contain "FRAGMENT-BODY-testing"

    # The third selector kind, and the one whose payload is a structure rather
    # than prose: an MCP entry is re-marshalled to YAML under its own heading.
    # A server with no `command:` is not a server, so the command line is what
    # this asserts.
    Scenario: The selector reaches an MCP server's configuration too
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And I run "ctxloom bundle edit demo --add-mcp tree-sitter"
      When Alice reads the server the bundle declares:
        """
        ctxloom bundle view demo#mcp/tree-sitter
        """
      Then the command succeeds
      And the output contains "# MCP Server: tree-sitter"
      And the output contains "command: tree-sitter"

    # `--distilled` is a PREFERENCE, not a guarantee: an item with no distilled
    # rendering is returned raw rather than empty. That fallback is the whole
    # point — a distiller that has never run must not make content unreadable —
    # and it is easy to break into a silent empty read. The case where a
    # distilled form DOES exist is specified in content_distill.feature, which
    # owns the distiller fixture.
    Scenario: Asking for the distilled rendering of an item that has none falls back to the raw bytes
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When I run "ctxloom bundle view demo#fragments/testing"
      Then the command succeeds
      And the output contains "FRAGMENT-BODY-testing"
      When Alice asks for the compressed rendering that was never made:
        """
        ctxloom bundle view demo#fragments/testing --distilled
        """
      Then the command succeeds
      And the output contains "FRAGMENT-BODY-testing"

    # A malformed selector must say what is wrong with it. Both refusals name
    # the vocabulary, because the everyday way to reach either is a typo, and
    # an error that only says "not found" sends the reader looking for a
    # missing item rather than a mistyped path.
    Scenario: A selector that is not `type/name`, or names a type that does not exist, is refused by name
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When I run "ctxloom bundle view demo#fragments"
      Then the command fails
      And the output contains "invalid path format"
      When I run "ctxloom bundle view demo#widgets/testing"
      Then the command fails
      And the output contains "unknown item kind"
      And the output contains "widgets"
      And the output contains "expected fragments, commands, mcp, skills, or profiles"
      # And the well-formed spelling still works, so the refusals above are
      # refusals about the PATH rather than about this fixture's bundle.
      When I run "ctxloom bundle view demo#fragments/testing"
      Then the command succeeds
      And the output contains "FRAGMENT-BODY-testing"

  Rule: Editing a bundle changes the manifest, and a no-op edit says it changed nothing

    The flag form of `edit` is the scriptable one: it sets metadata and
    attaches or detaches items by name. Every attachment is asserted on the
    stored file, and every detachment is preceded by the attachment it undoes.

    Scenario: Metadata edits land in the file
      Given an initialized ctxloom project
      And a bundle "demo" exists
      # The description the bundle was created with, read first so the
      # "no longer there" assertion below is made against a fixture that
      # demonstrably had it.
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" contains "acceptance fixture bundle"
      When Alice re-describes the bundle:
        """
        ctxloom bundle edit demo -d updated-desc
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" contains "updated-desc"
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" does not contain "acceptance fixture bundle"

    # Attach then detach, in one fixture, on the payload. A `--remove-*` that
    # reported success and removed nothing passes any exit-code check, and a
    # `--add-*` that wrote an empty entry passes any name check — the
    # placeholder body `edit` actually writes is what distinguishes both.
    Scenario: Items are attached and detached by name, and the manifest follows
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When Alice attaches a fragment, a command and a server:
        """
        ctxloom bundle edit demo --add-fragment coding-standards --add-prompt review --add-mcp tree-sitter --add-tag golang
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/fragments/coding-standards.md" exists
      And the file ".ctxloom/content/bundles/v2/demo/prompts/review.md" contains "Add prompt content here."
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tree-sitter.yaml" exists
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" contains "golang"
      When Alice detaches the two she did not want after all:
        """
        ctxloom bundle edit demo --remove-fragment coding-standards --remove-mcp tree-sitter --remove-tag golang
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/fragments/coding-standards.md" does not exist
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tree-sitter.yaml" does not exist
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" does not contain "golang"
      # The command that was NOT detached is still there, so the removals above
      # took exactly what they named rather than emptying the manifest.
      And the file ".ctxloom/content/bundles/v2/demo/prompts/review.md" contains "Add prompt content here."

    # An edit with no flags is a person who does not yet know what to type, and
    # the answer is help plus a plain statement that nothing happened — not a
    # silent exit 0 that reads as a successful edit.
    Scenario Outline: An edit that was told to change nothing says so
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When I run "ctxloom bundle edit demo <flags>"
      Then the command succeeds
      And the output reports "status" as "<no changes signal>"
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" contains "acceptance fixture bundle"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | no changes signal |
        |               | no_changes         |
        | --format json | no_changes         |
        | --format text | No changes made.   |

  Rule: A bundle travels as its tree, and an import never clobbers without being told to

    Export copies the bundle as-is; import copies it back. "Observable" is the
    claim, and existence plus a name did not observe it: an export that
    marshalled only the scalar header and dropped the fragments:/commands: maps
    was green — the file existed, import reconstructed a bundle called demo,
    and `bundle list` printed it, with the entire payload lost in transit. Both
    ends now name a body.

    Scenario: Export, remove, and re-import a bundle
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When Alice exports the bundle to share it:
        """
        ctxloom bundle export demo exported
        """
      Then the command succeeds
      And the file "exported/demo/prompts/example.md" contains "Example prompt content. Describe what this prompt does."
      And the file "exported/demo/fragments/example.md" contains "# Example Fragment"
      When I run "ctxloom bundle remove demo --yes"
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" does not exist
      When Alice imports the copy back:
        """
        ctxloom bundle import exported/demo -f
        """
      Then the command succeeds
      When I run "ctxloom bundle list"
      Then the output contains "demo"
      And the file ".ctxloom/content/bundles/v2/demo/prompts/example.md" contains "Example prompt content. Describe what this prompt does."
      And the file ".ctxloom/content/bundles/v2/demo/fragments/example.md" contains "# Example Fragment"

    # `-o` writes to an exact file path instead of a directory. Same payload
    # claim, different destination shape — and the only place this flag is
    # exercised at all.
    Scenario: Export can name the destination file outright
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When Alice exports to a path of her own choosing:
        """
        ctxloom bundle export demo -o handoff
        """
      Then the command succeeds
      And the file "handoff/fragments/example.md" contains "# Example Fragment"

    # THE DATA-LOSS GUARD. An import lands on a path that may already hold
    # somebody's work, so the refusal has to be proven by what SURVIVES it, not
    # by the error text: a guard that printed the message and overwrote anyway
    # would pass an exit-code check and still have eaten the local edit. The
    # forced retry that follows proves the guard was the only thing in the way,
    # so the survival above is not simply a broken import.
    Scenario: Importing over an existing bundle refuses, and the local edit survives
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And I run "ctxloom bundle export demo exported"
      And I run "ctxloom bundle edit demo -d LOCAL-EDIT-MARKER"
      When Alice imports the shared copy over her own:
        """
        ctxloom bundle import exported/demo
        """
      Then the command fails
      And the output contains "bundle already exists"
      And the output contains "use --force to overwrite"
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" contains "LOCAL-EDIT-MARKER"
      When Alice decides she meant it:
        """
        ctxloom bundle import exported/demo --force
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" does not contain "LOCAL-EDIT-MARKER"
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" contains "acceptance fixture bundle"

  Rule: A push lands the bundle's whole tree in the remote's repository

    `bundle push` writes every file of the authored tree to the remote's
    default branch as ONE commit. A remote that is not a forge with an API is
    reached through the user's own git, which owns their identity and
    credentials. ctxloom supplies neither, so the scenario gives git an
    identity the way a user's own configuration would. Every assertion reads
    the REMOTE's branch, because "Pushed …" is printed by a push and is
    equally printed by one that moved nothing.

    Scenario: Pushing a bundle to a plain git remote lands its tree there
      Given an initialized ctxloom project
      And a bundle "authored" exists
      And a git remote "team" serving a ctxloom bundle
      And the environment variable "GIT_AUTHOR_NAME" is set to "Alice"
      And the environment variable "GIT_AUTHOR_EMAIL" is set to "alice@example.test"
      And the environment variable "GIT_COMMITTER_NAME" is set to "Alice"
      And the environment variable "GIT_COMMITTER_EMAIL" is set to "alice@example.test"
      When Alice publishes her bundle to the team's repository:
        """
        ctxloom bundle push authored team
        """
      Then the command succeeds
      And the remote "team" holds file "bundle.yaml" of bundle "authored" containing "acceptance fixture bundle"
      And the remote "team" holds file "fragments/example.md" of bundle "authored" containing "# Example Fragment"
      # The bundle the remote already served is still there: a publish adds to
      # the repository's history rather than replacing it.
      And the remote "team" holds file "fragments/demo-frag.md" of bundle "demo" containing "Demo fragment content."

  Rule: A move relocates the bundle, and the source goes only once the destination holds it

    `bundle move --to` takes a configured remote name or an existing
    directory, and a directory that is a ctxloom project checkout receives the
    bundle in its own committed content tree. Both halves of a move are read
    from DISK: "Moved …" is printed by a move that copied nothing, and a copy
    that left the source behind is an export, not a move.

    @json-default
    Scenario: Moving a bundle into another project lands it in that project's content tree
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And the project already has the file "other-project/.ctxloom/.keep":
        """
        """
      When Alice hands her bundle over to the other project:
        """
        ctxloom bundle move demo --to other-project
        """
      Then the command succeeds
      And the output reports "status" as "moved"
      And the output reports "dest" as "other-project/.ctxloom/content/bundles/v2/demo"
      And the file "other-project/.ctxloom/content/bundles/v2/demo/bundle.yaml" contains "acceptance fixture bundle"
      And the file "other-project/.ctxloom/content/bundles/v2/demo/fragments/example.md" contains "# Example Fragment"
      And the file "other-project/.ctxloom/content/bundles/v2/demo/prompts/example.md" contains "Example prompt content. Describe what this prompt does."
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" does not exist
      When I run "ctxloom bundle list"
      Then the command succeeds
      And the output does not contain "demo"

    # --to never guesses: a name that is neither a configured remote nor an
    # existing directory is refused, and the refusal is proven by the source
    # SURVIVING it, not by the error text alone.
    Scenario: A destination that is neither a remote nor a directory is refused, and the source stays
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When Alice names a destination that does not exist:
        """
        ctxloom bundle move demo --to nowhere
        """
      Then the command fails
      And the output contains "neither a configured remote nor an existing directory"
      And the file ".ctxloom/content/bundles/v2/demo/fragments/example.md" contains "# Example Fragment"

  Rule: An authored bundle is not an installed dependency

    A local bundle is authored here; a dependency is pinned in the lockfile
    from a remote. `deps hold` operates on the second kind, so pointing it at
    the first has nothing to freeze — and it says so plainly instead of
    recording a hold that would silently protect nothing.

    Scenario: Holding a local bundle reports it is not lockfile-tracked
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When Alice tries to freeze a bundle she wrote herself:
        """
        ctxloom deps hold demo
        """
      Then the command succeeds
      And the output contains "nothing to hold"
      When I run "ctxloom deps unhold demo"
      Then the command succeeds

  Rule: Removal reports first and destroys only when told to

    `remove` is the destroyer, and its bare form is a preview: it names what
    would go — the bundle and every item it carries — states outright that
    nothing was removed, and prints the exact command that would apply it.

    # A guard that quietly destroyed anyway would still pass a scenario that
    # only checked exit code or the report text. The file-exists check is what
    # actually catches that, and the counts are what prove the report was built
    # from the manifest rather than from the argument.
    Scenario Outline: Bare bundle remove reports what would go and destroys nothing
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When Alice asks what removing the bundle would cost:
        """
        ctxloom bundle remove demo <flags>
        """
      Then the command succeeds
      And the output reports "applied" as "<nothing was applied>"
      And the output reports "detail.0" as "<the cost named>"
      And the output reports "apply" as "<the invocation that would>"
      # The report side asserts the bundle still EXISTS — on disk and in the
      # listing both, because a remove that pruned the listing while leaving
      # the file (or the reverse) would satisfy exactly one of these.
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" exists
      When I run "ctxloom bundle list <flags>"
      Then the output reports "[name=demo].name" as "<the bundle that survived>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | nothing was applied  | the cost named               | the invocation that would          | the bundle that survived |
        |               | false                 | 2 fragment(s), 1 command(s)  | ctxloom bundle remove demo --yes   | demo                      |
        | --format json | false                 | 2 fragment(s), 1 command(s)  | ctxloom bundle remove demo --yes   | demo                      |
        | --format text | Nothing was removed   | 2 fragment(s), 1 command(s)  | ctxloom bundle remove demo --yes   | demo                      |

    Scenario Outline: Removing a bundle with --yes takes the file and the listing entry
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When Alice applies the removal:
        """
        ctxloom bundle remove demo --yes
        """
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" does not exist
      When I run "ctxloom bundle list --no-companions <flags>"
      Then the output reports "$" as empty, saying "No bundles installed."

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         |
        |               |
        | --format json |
        | --format text |

    # The aliases are not decoration: `rm` is what a person's fingers type, and
    # an alias that reached a different code path — a different default, a
    # missing preview — would be a destroyer with no guard on it. Both halves
    # are driven through the alias, in one fixture.
    Scenario Outline: The rm alias previews and destroys exactly as remove does
      Given an initialized ctxloom project
      And a bundle "demo" exists
      When I run "ctxloom bundle rm demo <flags>"
      Then the command succeeds
      And the output reports "applied" as "<nothing was applied>"
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" exists
      When I run "ctxloom bundle del demo --yes"
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/bundle.yaml" does not exist

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | nothing was applied |
        |               | false                |
        | --format json | false                |
        | --format text | Nothing was removed  |
