# world: surfaces
# Ported from cli/materialize.feature (rev f36e8dfb2)
# Ported from journeys/j000400_multi_engine.feature (rev f36e8dfb2): the Rule on one materialize delivering every surface
# Ported from journeys/j000800_onboarding.feature (rev f36e8dfb2)
# Ported from journeys/j001400_bundle_distribution.feature (rev f36e8dfb2)
# NOT ported: the Rules tagged @pending: "A run says so when the project holds
#   materialized context" needs a running mock engine (S6); "A pulled
#   bundle's surfaces reach an agent in every workspace" has no steps yet.
@doc
Feature: materialize — the profiles' surfaces written at rest, any subset of them

  `ctxloom materialize` writes the assembled profiles into a directory as
  each engine's NATIVE files — its context file, MCP registry, settings and
  hooks, commands and skills — so an engine launched there with ctxloom out of
  the loop inherits them. It is an export, not how ctxloom delivers: a session
  ctxloom starts is handed its context in its own session home and never reads
  these files. Materialize is for sharing the team's context with someone who
  does not use ctxloom, and what it writes is a snapshot that goes stale as
  the team's context moves on. Each engine's kinds are owned separately, so
  re-delivering one kind, or one engine, never takes another's files out, and
  `--release` takes back exactly what it names.

  Rule: A subset re-delivery leaves every other surface standing

    # The per-kind ownership is the whole point: a context-only run that took
    # the commands out with it would still satisfy "the context changed".
    Scenario: Re-materializing only the context keeps the commands and skills
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And I run "ctxloom skill create demo reviewer -d SKILL-MARKER-reviewer"
      And a profile "dev" with bundle "demo"
      When I run "ctxloom materialize dev --target out --backend mock"
      Then the command succeeds
      And the file "out/MOCK_CONTEXT.md" contains "FRAGMENT-BODY-testing"
      And the file "out/.mock/commands/demo/example.md" exists
      When I run "ctxloom materialize dev --target out --backend mock --surface context"
      Then the command succeeds
      And the file "out/MOCK_CONTEXT.md" contains "FRAGMENT-BODY-testing" exactly 1 times
      And the file "out/.mock/commands/demo/example.md" exists
      When I run "ctxloom materialize --target out --backend mock --release --surface commands"
      Then the command succeeds
      And the file "out/.mock/commands/demo/example.md" does not exist
      And the file "out/MOCK_CONTEXT.md" contains "FRAGMENT-BODY-testing"

  Rule: The context can land in any file inside the target, and leaves it as it was

    Scenario: Context written into the user's own file is released byte for byte
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the project already has the file "out/docs/AGENTS.md":
        """
        # Our agents

        Hand-written notes that stay ours.
        """
      When I run "ctxloom materialize dev --target out --backend mock --surface context=file:docs/AGENTS.md"
      Then the command succeeds
      And the file "out/docs/AGENTS.md" contains "Hand-written notes that stay ours."
      And the file "out/docs/AGENTS.md" contains "FRAGMENT-BODY-testing"
      And the file "out/MOCK_CONTEXT.md" does not exist
      When I run "ctxloom materialize --target out --backend mock --release --surface context"
      Then the command succeeds
      And the file "out/docs/AGENTS.md" does not contain "FRAGMENT-BODY-testing"
      And the file "out/docs/AGENTS.md" matches "\A# Our agents\n\nHand-written notes that stay ours.\n?\z"

  Rule: Two engines' files coexist in one directory

    Scenario: Releasing one engine leaves the other's files
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When I run "ctxloom materialize dev --target out --backend mock --backend claude-code"
      Then the command succeeds
      And the file "out/MOCK_CONTEXT.md" contains "FRAGMENT-BODY-testing"
      And the file "out/CLAUDE.md" contains "FRAGMENT-BODY-testing"
      When I run "ctxloom materialize --target out --backend mock --release"
      Then the command succeeds
      And the file "out/MOCK_CONTEXT.md" does not exist
      And the file "out/CLAUDE.md" contains "FRAGMENT-BODY-testing"

  Rule: The project directory is written only when asked for in so many words

    Scenario: With no target, materialize warns and refuses unless --yes is given
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When I run "ctxloom materialize dev --backend mock"
      Then the command fails
      And the output contains "no --target given"
      And the file "MOCK_CONTEXT.md" does not exist
      When I run "ctxloom materialize dev --backend mock --yes"
      Then the command succeeds
      And the file "MOCK_CONTEXT.md" contains "FRAGMENT-BODY-testing"

  @pending
  Rule: A run says so when the project holds materialized context

    Scenario: A run in a project with materialized context warns and names the remedy
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the mock LLM responds "MOCK-REPLY"
      When I run "ctxloom materialize dev --backend mock --surface context --yes"
      Then the command succeeds
      When I run "ctxloom run --one-shot --profile dev unicorn-prompt"
      Then the command succeeds
      And the output contains "MOCK-REPLY"
      And the output contains "ctxloom materialize --release --surface context"

  Rule: One materialize delivers every surface of a profile, each in its engine's own shape

    # What one materialize delivering all four surfaces together proves, no
    # per-surface spec can see: a regression that writes three surfaces and
    # drops the fourth passes cli/fragment, cli/mcp, cli/manage and
    # cli/command, and fails only here. Every row parses the generated file in
    # its own format and asserts the actual field, never a bare file-exists
    # and never a substring of a key name. A context surface writes the
    # assembled context, never a second assembly of the fragments that could
    # disagree with the first.
    Scenario Outline: The same profile materializes into each engine's own native surfaces
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When Alice materializes the team profile for <engine>
      Then the materialized <engine> context carries the shared fragment's marker, in its own native shape
      And the materialized <engine> MCP configuration carries the shared server's command, in its own native shape
      And the materialized <engine> hook configuration carries the shared hook's command, in its own native shape
      And the materialized <engine> command file carries the shared command's body, in its own native shape

      Examples:
        | engine      |
        | claude-code |

    # An engine whose hooks, MCP servers and commands have no home at rest
    # (mock-launch declares a context surface and no other) materializes its
    # context and reports each surface it does not carry, with the reason.
    # Absence is checked over the whole tree, so a fallback landing somewhere
    # unexpected is caught; the profile carries a hook, a server and a command,
    # so the absence steps cannot pass for want of anything to deliver. Both
    # halves are asserted: skipping without reporting is a silent loss, and
    # reporting without skipping names a surface absent while its file exists.
    Scenario: A home-keyed engine materializes its native context, and reports the surfaces it does not carry
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When Alice materializes the team profile for mock-launch
      Then the materialized mock-launch context carries the shared fragment's marker, in its own native shape
      And no mock-launch surface anywhere in the materialized tree carries the shared hook's command
      And no mock-launch surface anywhere in the materialized tree carries the shared MCP server's command
      And no mock-launch surface anywhere in the materialized tree carries the shared command's body
      And the materialize report names each surface mock-launch does not carry, with a reason

    # Content outside ctxloom's managed markers survives byte for byte, and
    # ctxloom's own content still lands beside it: a whole-file write in place
    # of the managed section turns this red, the hand-authored line gone.
    Scenario Outline: A hand-authored context file survives materialization byte-for-byte
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      And Alice's team already hand-authored <file> for <engine> with their own conventions
      When Alice materializes the team profile for <engine>
      Then <file> still carries Alice's hand-authored conventions, byte-for-byte
      And the materialized <engine> context carries the shared fragment's marker, in its own native shape

      Examples:
        | engine      | file            |
        | claude-code | CLAUDE.md       |
        | mock        | MOCK_CONTEXT.md |

  Rule: An engine that cannot carry a hook is named in the report, with why

    # The report must name the hook it dropped and the reason, not merely
    # mention hooks: mock-lossy has no session_start event, so the team's
    # session_start hook cannot reach it.
    Scenario: Materializing for an engine that cannot carry hooks says so
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When Alice materializes the team profile for mock-lossy
      Then the materialize report names the hook it could not deliver to mock-lossy

  @pending
  Rule: A pulled bundle's surfaces reach an agent in every workspace, with their modes

    Delivery writes the pulled bytes and modes where the agent in that
    configuration reads them: the project directory, or a detached worktree
    checkout outside it, and the same from a ctxloom running in a container.

    Background:
      Given Trent authors a directory-form bundle "atelier" carrying every surface kind

    # The mock engine is the vehicle: its context file and skills tree are
    # written by the same delivery every engine uses. A skill file is compared
    # whole, and its mode is the publisher's declaration (the script 0755, the
    # rest 0644). A fragment is merged into the engine's one context file, so
    # its published body must appear in that file verbatim, and its mode is
    # the context file's own: 0600, owner-only.
    Scenario Outline: The published artifacts reach a host agent in every workspace
      Given Trent publishes the "atelier" tree to his company repo
      And Alice references the company's "atelier" bundle and pulls it
      When the pulled surfaces are delivered to a "<runtime>" agent in its "<workspace>" workspace
      Then the "<artifact>" reaches that agent's workspace carrying its published bytes
      And the mode of "<artifact>" is "<mode>" where that agent can read it

      Examples: host runtime — the workspace axis alone
        | runtime | workspace | artifact                        | mode |
        | host    | none      | fragments/house-style.md        | 0600 |
        | host    | none      | skills/reviewer/SKILL.md        | 0644 |
        | host    | none      | skills/reviewer/scripts/run.sh  | 0755 |
        | host    | worktree  | fragments/house-style.md        | 0600 |
        | host    | worktree  | skills/reviewer/SKILL.md        | 0644 |
        | host    | worktree  | skills/reviewer/scripts/run.sh  | 0755 |

    # ctxloom runs inside a minimal container (no git, no network, no
    # /etc/passwd) whose only way to Alice's cache and project is a bind mount
    # of the environment at its own path. The target is checked empty before
    # the container starts. Only bytes are asserted: modes and ownership
    # across a bind mount are the container cell's own tests'. @container
    # needs a reachable daemon; @image-cell builds the image once, before the
    # scenarios.
    @container @image-cell
    Scenario Outline: A pulled bundle reaches an agent delivered from inside a container
      Given Trent publishes the "atelier" tree to his company repo
      And Alice references the company's "atelier" bundle and pulls it
      When the pulled surfaces are delivered to a "container" agent in its "none" workspace
      Then the "<artifact>" reaches that agent's workspace carrying its published bytes

      Examples: one row per kind of delivered surface
        | artifact                        |
        | fragments/house-style.md        |
        | skills/reviewer/SKILL.md        |
        | skills/reviewer/scripts/run.sh  |
