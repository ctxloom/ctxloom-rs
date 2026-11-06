# Ported from cli/manage.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   manage.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from cli/manage.feature`.

@doc
@pending
Feature: manage — wiring ctxloom into a project, and taking it back out

  `ctxloom manage` is the noun that owns a project's HARNESS: the `.ctxloom`
  scaffold, the gitignore entries that keep ctxloom's private state out of
  source control, and — only when asked for explicitly — the engine's own
  hooks and statusline and MCP registration in the project tree. It is the
  first command anyone runs and the one they run again when they want to
  leave.

  A `ctxloom run` session carries every runtime surface in its own session
  home, so `manage install` writes no engine file; an engine launched
  directly in the project tree gets no ctxloom hooks and no ctxloom MCP
  server. The project-side copy exists only where `ctxloom materialize`
  was asked for, and `manage uninstall` is how it leaves.

  This is the comprehensive per-noun spec: what the noun DOES, leaf by leaf,
  including the refusals and the shapes that only matter to a machine. The
  narrative version — Alice's first hour, and satisfying herself she can back
  out — is journeys/j000100_adopt_and_back_out.feature, which asserts what a PERSON
  sees.

  Rule: Installing scaffolds the project for one engine, and validates which

    `manage install` scaffolds `.ctxloom`, records the chosen engine,
    git-ignores ctxloom's private state, and pulls the remote dependencies
    the profiles reference (`--no-pull` skips that, and keeps every cell
    here off the network). It writes NO engine file: the
    engine's own native configuration is delivered into a `ctxloom run`
    session's home, and reaches the project tree only through an explicit
    `ctxloom materialize`. Which files THAT writes differs per engine and is
    the whole point of the engine axis; what they must all do is put the
    project's context in front of that engine and register ctxloom as an MCP
    server.

    # THE COMMAND SURFACE, the third thing install writes. ctxloom ships
    # first-party commands, and every engine gets them in its own idiom: a flat
    # markdown file for the engines that have slash-commands, a SKILL.md
    # package directory for the engines whose equivalent is an Agent Skill.
    # The path column is where the idioms differ; the claim does not.
    #
    # ONE marker for every row, and it is the command's BODY — not its name,
    # not its frontmatter. A materializer that creates the right paths and
    # writes only frontmatter into them satisfies a name check and delivers a
    # command that does nothing.
    #
    # mock has no row: it is a registered engine with context and skills
    # surfaces but no command surface of its own, and asserting an absent
    # surface would be asserting nothing.
    #
    # Two engines carry a command surface, which is what makes "in its own
    # idiom" a claim two rows can disagree about.
    # (ruled 2026-09-21: sessions carry their surfaces — the project-side
    # command files are an explicit materialize's)
    Scenario Outline: Every engine gets ctxloom's shipped commands in its own idiom
      Given an empty project directory
      When Alice installs ctxloom for <engine> and wires its files in:
        """
        ctxloom manage install --engine <engine> --no-pull
        ctxloom materialize --yes --degraded
        """
      Then the command succeeds
      And the file "<command_surface>" contains "Scan the current project and discover matching ctxloom content"

      # An engine whose prompts are home-global — delivered into a session's
      # own home at launch — has no row: a static install writes none.
      Examples: a flat command file
        | engine      | command_surface              |
        | claude-code | .claude/commands/discover.md |
        | mock        | .mock/commands/discover.md   |

  Rule: Hooks are materialized, listed, and genuinely released

    # Each hook must name ONE place to go. A coarse "local" label covering every
    # declaration site at once leaves a user searching all of them, so the
    # fixture declares hooks in two DIFFERENT kinds of place and requires them to
    # be labelled distinctly: a profile the project authored, and a companion
    # binary found on PATH. Naming the profile is not enough on its own — the
    # label has to carry the KIND, since "which file do I open" and "which
    # profile is it" are different questions.
    Scenario Outline: The hook list names the specific place each hook was declared
      Given an initialized ctxloom project
      And a companion "ctxloom-companion-hooky" shipping a "pre_tool" hook is on PATH
      And the project already has the file ".ctxloom/content/bundles/v2/project/profiles/dir-prov.yaml":
        """
        hooks:
          unified:
            pre_tool:
              - type: command
                command: echo PROV-FROM-DIRECTORY-PROFILE
        """
      When Alice asks where each hook came from:
        """
        ctxloom manage hooks list --event pre_tool --profile dir-prov <flags>
        """
      Then the command succeeds
      # Selected by source_kind: this fixture declares exactly one
      # profile-directory hook, so the selection cannot be ambiguous. The
      # companion-sourced hook is the fixture's OWN, installed above — it used
      # to be whatever first-party companions this machine happened to have,
      # which made the assertion a fact about the developer's laptop and
      # shifted every hook ordinal depending on what was installed.
      And the output reports "events[event=pre_tool].hooks[source_kind=profile-directory].command" as "<names the hook>"
      And the output reports "events[event=pre_tool].hooks[source_kind=profile-directory].source" as "<names the profile>"
      And the output contains "<a companion-sourced hook is present too>"
      And the output does not contain "[local]"
      # The machine form carries the same specific kind, plus the position the
      # MERGE gave each hook — so a later reordering is visible as a move
      # rather than as an unexplained final number.
      When Alice asks for the same answer in machine form:
        """
        ctxloom manage hooks list --event pre_tool --profile dir-prov --format json
        """
      Then the command succeeds
      And the output contains "profile-directory"
      And the output contains "declared"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | names the hook                    | names the profile | a companion-sourced hook is present too |
        |               | echo PROV-FROM-DIRECTORY-PROFILE  | dir-prov            | companion                                 |
        | --format json | echo PROV-FROM-DIRECTORY-PROFILE  | dir-prov            | companion                                 |
        | --format text | PROV-FROM-DIRECTORY-PROFILE       | dir-prov            | companion                                 |

  Rule: Uninstalling removes what ctxloom wired and keeps what the team authored

    # THE GATE FOR THE DELIVERY LAYER: uninstall is the empty plan over the
    # ownership record, so nothing of the harness is left behind; a run
    # afterwards delivers into its own session and the project stays as the
    # uninstall left it. Where the run delivered is read off the mock's
    # record, written while it ran: the run's teardown releases a delivery
    # into the project, so the install's files being absent afterwards shows
    # only that nothing was left behind. Ruled 2026-09-21: sessions carry their surfaces; the project
    # is written only under a root the binding selects, and the project-side
    # files this scenario removes are an explicit materialize's.
    Scenario: After an uninstall, a run delivers into its session and the project stays clean
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      # A run delivers a kind only when it has items (engine.Base.Delegate), and
      # the record step requires every surface to be named; the skill is what
      # puts the skills surface on the record.
      And I run "ctxloom skill create demo reviewer -d SKILL-MARKER-reviewer"
      And a profile "dev" with bundle "demo"
      And the mock LLM responds "MOCK-REPLY"
      When Alice wires the mock in, takes it back out, and runs:
        """
        ctxloom manage install --engine mock
        ctxloom materialize --yes
        ctxloom manage uninstall
        ctxloom run --one-shot --profile dev unicorn-prompt
        """
      Then the command succeeds
      And the output contains "MOCK-REPLY"
      And the mock was handed every surface from its session, none from the project
      And the file "MOCK_CONTEXT.md" does not exist
      And the file ".mock/mcp.json" does not exist
      And the file ".mock/commands/discover.md" does not exist
      And the file ".ctxloom/config.yaml" exists
