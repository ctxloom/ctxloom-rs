# Ported from cli/config.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@doc
@pending
Feature: config — the project's one configuration document, read and scaffolded

  Covers: `ctxloom config show [section]`, `config create`, `config edit`,
  and the bare `ctxloom config` form.

  Configuration is a SINGLETON, not a collection. A project has exactly one
  `.ctxloom/config.yaml`, so there is no `list` for the bare noun to fall back
  on — the read-one view IS the default, and `ctxloom config` renders the whole
  document the way `git config --list` does. That is the shape of this noun:
  one document, one place, addressable in whole or by section.

  THE DOCUMENT IS SHARED, WHICH IS WHY THE SECTION VIEW MATTERS. `agent set`,
  `manage install` and the init interview all write into the same file, and `config show <section>` is how a person reads
  back what another command just wrote. A section view that silently rendered
  the whole document would look right and answer the wrong question, so every
  scenario here pins the narrowing by what must be ABSENT as well as present.

  WHAT `create` IS AND IS NOT. It scaffolds the minimum a project needs to be a
  ctxloom project — config.yaml, remotes.yaml, the seeded default profile — and
  refuses to overwrite a config that already exists. It is deliberately NOT the
  setup interview: `ctxloom init` owns that (see cli/init.feature), reaches the
  network on a fresh project, and wires engine hooks; `config create` writes
  files and stops. Which ENGINE the scaffold records is asserted per engine in
  cli/manage.feature, beside `manage install`, because that is where the
  engine-axis matrix lives; this file asserts what create writes and what it
  refuses.

  Rule: The bare noun renders the whole document

    Someone typing a noun on its own wants to know what they have. `ctxloom
    config` answers with the configuration rather than teaching which
    subcommands exist — the same bare-noun-answers seam `ctxloom remote` and
    `ctxloom deps` use — and `--help` remains the teaching surface.

    # PAYLOAD, NOT EXISTENCE. This scenario used to assert `the output matches
    # "."` — one arbitrary character, which a single space, a stray newline or
    # a warning banner satisfies exactly as well as the whole config does. That
    # is a vacuous assertion over precisely the surface ctxloom's
    # characteristic bug (exit 0, success message, zero real payload) hides in,
    # so it names keys the rendered document must actually carry.
    Scenario Outline: Show renders the whole configuration document
      Given an initialized ctxloom project
      When Alice reads her project's configuration:
        """
        ctxloom config show <flags>
        """
      Then the command succeeds
      And the output contains "claude-code"
      And the output reports "llm.configs.claude-code.type" as "<names the llm section>"
      And the output reports "llm.configs.claude-fast.type" as "<names the configs block>"
      And the output reports "llm.defaults.primary" as "<names the defaults block>"

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         | names the llm section | names the configs block | names the defaults block |
      |               | claude-code           | claude-code             | claude-code              |
      | --format json | claude-code           | claude-code             | claude-code              |
      | --format text | llm:                  | configs:                | defaults:                |

    # The bare noun answers rather than teaches, through the same seam
    # `ctxloom remote` uses. "Available Commands:" is cobra's help heading —
    # its absence is what says a document was rendered rather than a menu.
    Scenario Outline: Bare config shows the document instead of printing help
      Given an initialized ctxloom project
      When I run "ctxloom config <flags>"
      Then the command succeeds
      And the output reports "llm.configs.claude-code.type" as "<names the llm section>"
      And the output reports "llm.defaults.primary" as "<names the defaults block>"
      And the output does not contain "Available Commands:"

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         | names the llm section | names the defaults block |
      |               | claude-code           | claude-code              |
      | --format json | claude-code           | claude-code              |
      | --format text | llm:                  | defaults:                |

    # THE ZERO-PAYLOAD TRAP, and the reason this scenario exists at all.
    # Config's fields are unexported and it renders through a custom
    # MarshalYAML, so handing the struct straight to a reflective encoder
    # yields "{}" — a well-formed, entirely empty document with a 0 exit. Valid
    # JSON is therefore NOT the assertion; valid JSON that still carries a
    # value only the real config has is.
    Scenario: The machine-readable rendering carries the document, not an empty object
      Given an initialized ctxloom project
      When Alice asks for the configuration in a form a script can parse:
        """
        ctxloom --format json config show
        """
      Then the command succeeds
      And the output is valid JSON
      And the output contains "claude-code"

  Rule: A section view narrows, and narrowing is asserted by what is absent

    `config show <section>` claims to answer a smaller question than bare `show`.
    Nothing proves that by presence alone — a full dump contains every
    section's contents too — so each scenario names a key that IS in the whole
    document and must NOT be in the section view.

    Scenario Outline: The llm section carries the engine registry and nothing above it
      Given an initialized ctxloom project
      When Alice asks which engines this project is wired for:
        """
        ctxloom config show llm <flags>
        """
      Then the command succeeds
      And the output contains "claude-code"
      And the output reports "configs.claude-code.type" as "<names the configs block>"
      And the output reports "defaults.primary" as "<names the defaults block>"
      And the output does not contain "editor:"
      And the output does not contain "llm:"

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         | names the configs block | names the defaults block |
      |               | claude-code             | claude-code              |
      | --format json | claude-code             | claude-code              |
      | --format text | configs:                | defaults:                |

    # A key the config schema does not describe — a typo, or a block that once
    # meant something, such as an inline `profiles:` — fails the command and
    # is named, rather than silently ignored with its settings gone.
    Scenario: A config carrying an inline profiles block is refused, naming the key
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "dev" is defined inline in config with bundle "demo"
      When Alice reads a section of the document:
        """
        ctxloom config show llm
        """
      Then the command fails
      And the output contains "unknown key `profiles`"

    # A key the schema DOES describe, holding a value of the wrong type, is
    # refused the same way: by file, key path and the type that was expected.
    # It used to be a warning, and the command ran on a setting the schema
    # could not vouch for.
    Scenario: A config value of the wrong type is refused, naming the file, key and expected type
      Given an initialized ctxloom project
      And the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        session_reap_age: 5
        """
      When Alice reads a section of the document:
        """
        ctxloom config show llm
        """
      Then the command fails
      And the output contains ".ctxloom/config.yaml"
      And the output contains "`session_reap_age`"
      And the output contains "expected string"

    # --degraded is the escape hatch for every fatal startup finding, this one
    # included: the violation is still reported, and the command runs.
    Scenario: Under --degraded a config value of the wrong type is warned about and the config loads
      Given an initialized ctxloom project
      And the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        session_reap_age: 5
        """
      When Alice reads a section of the document:
        """
        ctxloom --degraded config show llm
        """
      Then the command succeeds
      And the output contains "claude-code"
      And the output contains "`session_reap_age`"
      And the output contains "expected string"

    # The refusal has to be USEFUL, not merely correct: a caller who guessed
    # wrong recovers from the message or not at all, so it names every section
    # that does exist.
    Scenario: An unknown section is refused and the real ones are named
      Given an initialized ctxloom project
      When Alice mistypes the section she wanted:
        """
        ctxloom config show nonsense
        """
      Then the command fails
      And the output contains "nonsense"
      And the output contains "config"
      And the output contains "llm"

    # A namespace that printed its own help on a misspelling and exited 0 is
    # this project's characteristic silent no-op wearing the CLI's dispatch as
    # a disguise: the caller's question is never answered and the exit status
    # says everything went fine.
    Scenario: A misspelled subcommand fails instead of printing help
      Given an initialized ctxloom project
      When I run "ctxloom config shwo"
      Then the command fails
      And the output contains "unknown command"
      And the output contains "shwo"

  Rule: Create writes the scaffold, and never over an existing config

    `config create` materializes what a project needs to BE a ctxloom project
    and nothing that belongs to an engine: config.yaml, the default remotes
    registry, and the seeded default profile. Every one of those is asserted by
    its content, because a scaffold that exits 0 having written empty files is
    indistinguishable from a working one by exit code alone.

    Scenario: Create scaffolds the config, the remotes registry and the seeded profile
      Given an empty project directory
      When Alice turns a bare directory into a ctxloom project:
        """
        ctxloom config create
        """
      Then the command succeeds
      And the file ".ctxloom/config.yaml" is valid YAML
      And the file ".ctxloom/config.yaml" contains "llm: claude-code"
      And the file ".ctxloom/remotes.yaml" contains "ctxloom-default"
      And the file ".ctxloom/content/bundles/v2/project/profiles/default.yaml" exists
      When I run "ctxloom config show"
      Then the command succeeds
      And the output contains "claude-code"

    # THE REFUSAL IS ABOUT THE BYTES, NOT THE EXIT CODE. A create that
    # reported "already exists" and had nonetheless rewritten the file would
    # pass a message-only assertion while having destroyed the project's
    # configuration. The existing document is read back to prove it survived
    # verbatim: `schema_version: 8` is the fixture's own content (it tracks
    # config.CurrentConfigVersion — bump both together), and `engine:` is a
    # key only the scaffold writes — its absence is what says no scaffold ran.
    Scenario: Create refuses an existing config and leaves it byte-for-byte alone
      Given an initialized ctxloom project
      And the file ".ctxloom/config.yaml" contains "schema_version: 8"
      When Alice scaffolds over a project that already has a configuration:
        """
        ctxloom config create
        """
      Then the command fails
      And the output contains "config already exists"
      And the file ".ctxloom/config.yaml" contains "schema_version: 8"
      And the file ".ctxloom/config.yaml" does not contain "engine:"

  Rule: Edit refuses to open an editor on a config that is not there

    `config edit` hands the file to $EDITOR, which is why its success path has
    no hermetic fixture (it is listed in completeness_test.go's excludedLeaves
    for exactly that reason, and the editor round trip is exercised where an
    editor can be faked — see cli/fragment.feature and cli/mcp.feature). What
    IS specifiable here is the guard in front of it: the command must not
    launch an editor on a path it has nothing to edit, and must name the
    command that creates one.

    # The positive half of this claim lives in the create scenarios above: a
    # scaffolded project genuinely HAS the file this refusal is about, so
    # "absent" here means absent, not unwritten-by-the-fixture.
    Scenario: Editing a project with no config says so and names the fix
      Given an empty project directory
      When Alice reaches for the editor before the project has a config:
        """
        ctxloom config edit
        """
      Then the command fails
      And the output contains "no config at"
      And the output contains "ctxloom config create"
      And the file ".ctxloom/config.yaml" does not exist
