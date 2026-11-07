# world: surfaces
# Ported from cli/manage.feature (rev f36e8dfb2)
# NOT ported: these Go scenarios, tagged @pending, and why:
#   - "Every engine gets ctxloom's shipped commands in its own idiom": it reads
#     the body of the `discover` command, which names `search_library`, an MCP
#     tool the D8 cut list has not ruled on (D24).
#   - "The hook list names the specific place each hook was declared": needs a
#     companion's loadout read (S6).
#   - "After an uninstall, a run delivers into its session ...": needs a
#     running mock engine (S6).
#   The mock rows of "Check", "A team's hook" etc. are as Go wrote them; only
#   the engines this port registers run.
@doc
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
  sees; she sets ctxloom up with `init` and never wires her assistant's files
  in, so the wiring, its report and its removal are asserted only here.

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

    # ABSENCE, ON THE FILESYSTEM. Install's exit code and its report both
    # say "no engine file was written"; only the tree can prove it. Every
    # engine's own well-known files are named, so an install that wrote one
    # of them for its recorded engine fails here. A session's surfaces live
    # in its session home, never in the project.
    Scenario Outline: Installing scaffolds the project and writes no engine file
      # substitution: the nested .gitignore is also asserted to name the private cache, the rule that keeps ctxloom's working state out of source control.
      Given an empty project directory
      When Alice installs ctxloom for <engine>:
        """
        ctxloom manage install --engine <engine> --no-pull
        """
      Then the command succeeds
      And the file ".ctxloom/config.yaml" is valid YAML
      And the file ".ctxloom/config.yaml" contains "<engine>"
      And the file ".ctxloom/.gitignore" contains "ctxloom"
      And the file ".ctxloom/.gitignore" contains "/cache"
      And the file ".claude/settings.json" does not exist
      And the file ".mcp.json" does not exist
      And the file "CLAUDE.md" does not exist
      And the file "MOCK_CONTEXT.md" does not exist
      And the file ".mock/mcp.json" does not exist
      And the file ".mock/commands/discover.md" does not exist

      Examples:
        | engine      |
        | claude-code |
        | mock        |

    # PAYLOAD, NOT EXISTENCE. An empty file exists just as convincingly as a
    # wired one, so every row names content the surface must actually carry.
    #
    # EVERY ROW IS UNTAGGED because every row is hermetic: `ctxloom
    # materialize` WRITES an engine's config files and never launches the engine,
    # so no credential is needed and nothing gates behind @live. `mock` is a
    # first-class row — a registered engine with real surfaces of its own.
    #
    # AN EXPLICIT MATERIALIZE WRITES EVERY ENGINE'S CONTEXT FILE, claude's
    # CLAUDE.md included: it is for an agent launched with ctxloom OUT of the
    # loop, which has no system prompt from a `ctxloom run`. (The post-sync
    # refresh is the one at-rest delivery that leaves a launch-borne context
    # out.) claude also gets ctxloom's session-start hook (the session-start
    # notices), which names no context.
    # The project-side copy is an explicit materialize's, so it is spelled
    # out after the scaffold.
    #
    # --degraded: the scaffold's seed profile inherits ctxloom's published
    # default profile, a remote parent `manage install` pulls — but
    # --no-pull keeps this hermetic cell off the network, so it is never
    # installed; a strict materialize refuses to deliver past it, before
    # writing anything (`manage install` alone writes no engine file to
    # refuse).
    Scenario Outline: Alice wires an empty project for her engine
      Given an empty project directory
      When Alice installs ctxloom for <engine> and wires its files in:
        """
        ctxloom manage install --engine <engine> --no-pull
        ctxloom materialize --yes --degraded
        """
      Then the command succeeds
      And the file ".ctxloom/config.yaml" is valid YAML
      And the file "<context_surface>" contains "<context_marker>"
      And the file ".ctxloom/.gitignore" contains "ctxloom"

      # The marker is a heading from ctxloom's own shipped guidance.
      Examples: every engine's context file carries the assembled context
        | engine      | context_surface                 | context_marker                |
        | claude-code | CLAUDE.md                       | Isolation: specify both axes  |
        | mock        | MOCK_CONTEXT.md                 | Isolation: specify both axes  |

      Examples: claude's settings carry the session-start hook, which names no context
        | engine      | context_surface       | context_marker      |
        | claude-code | .claude/settings.json | session-start       |

    # ONE CLAIM, MANY SHAPES. ctxloom's own MCP server is the companion's
    # DYNAMIC declaration — served by the running session's endpoint, injected
    # into the SESSION's registry with that session's URL and bearer at
    # session start. At rest there is no session to inject, so an explicit
    # materialize registers NO server under ctxloom's name, in any
    # engine's dialect: an entry there would name a command that speaks no
    # protocol, and the engine launching it would come up with none of
    # ctxloom's tools and nothing saying why.
    #
    # ABSENCE UNDER THE KEY, asserted by PARSING the engine's own registry
    # when one exists — a bare "file does not exist" would also be satisfied
    # by an install that wrote nothing at all, which the context rows above
    # already rule out.
    # ctxloom's MCP server is registered only in a running session's home.
    Scenario Outline: No engine gets ctxloom registered as an MCP server at rest
      Given an empty project directory
      When Alice installs ctxloom for <engine> and wires its files in:
        """
        ctxloom manage install --engine <engine> --no-pull
        ctxloom materialize --yes --degraded
        """
      Then the command succeeds
      And the file "<mcp_surface>" registers no MCP server named "ctxloom"

      Examples: a file of its own
        | engine      | mcp_surface     |
        | claude-code | .mcp.json       |
        | mock        | .mock/mcp.json  |

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
    # The project-side command files are an explicit materialize's.
    @pending
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

    # NOTHING IS WRITTEN UNTIL THE ARGUMENT IS UNDERSTOOD. The "does not
    # exist" checks are the load-bearing half — a wrong-but-loud message
    # satisfies the first three assertions on its own, and only the absent
    # files catch an install that scaffolded a project around an engine nobody
    # supports.
    Scenario: A misspelled engine is refused by name, and nothing is scaffolded
      Given an empty project directory
      When Alice mistypes the engine name:
        """
        ctxloom manage install --engine totally-bogus-engine
        """
      Then the command fails
      And the output contains "unknown engine"
      And the output contains "totally-bogus-engine"
      And the output contains "claude-code"
      And the file ".ctxloom/config.yaml" does not exist
      And the file ".mcp.json" does not exist

  Rule: Config create selects the engine without writing any engine files

    `config create` scaffolds `.ctxloom` and records which engine the project
    uses — it writes no engine-native files at all. Every engine therefore
    produces a byte-identical tree apart from config.yaml, so the config's
    CONTENTS are the only place the flag is observable and a file-exists check
    proves nothing about which engine was asked for.

    Scenario: Config create scaffolds a config in a bare project
      Given an empty project directory
      When Alice scaffolds a config without wiring any engine:
        """
        ctxloom config create --engine claude-code
        """
      Then the command succeeds
      And the file ".ctxloom/config.yaml" exists

    Scenario Outline: Config create records which engine the project uses
      Given an empty project directory
      When Alice scaffolds a config for <engine>:
        """
        ctxloom config create --engine <engine>
        """
      Then the command succeeds
      And the file ".ctxloom/config.yaml" contains "llm: <engine>"
      And the file ".ctxloom/config.yaml" contains "type: <engine>"
      And the file ".ctxloom/config.yaml" contains "primary: <engine>"

      Examples:
        | engine |
        | mock   |

  Rule: The wiring can be inspected, and reports per-surface state

    # The engine NAME appears in both "claude-code: not configured" and
    # "claude-code: hooks=true ...", so matching it says nothing about whether
    # anything was wired. The per-surface flags are what "wired" means, so the
    # assertion is a regex over them.
    #
    # mcp is NOT wired at rest, and the report says so: ctxloom's own server
    # is served by the running session's endpoint and injected into the
    # session's registry at start, so an explicit materialize registers no
    # managed MCP server in the project — a "true" here would claim a wiring
    # the engine could not launch: ctxloom's MCP server is registered only in a
    # running session's home.
    Scenario Outline: Check reports which surfaces are actually wired
      # substitution: the Examples column headers "hooks are wired" etc. are spelled with underscores (cucumber-rs does not substitute a placeholder that has spaces).
      Given an initialized ctxloom project
      When Alice wires the hooks in and asks what is configured:
        """
        ctxloom materialize --yes
        ctxloom manage check <flags>
        """
      Then the command succeeds
      And the output reports "backends[backend=claude-code].hooks_present" as "<hooks_are_wired>"
      And the output reports "backends[backend=claude-code].mcp_present" as "<mcp_is_wired>"
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | hooks_are_wired | mcp_is_wired |
        |               | true              | false         |
        | --format json | true              | false         |
        | --format text | hooks=true        | mcp=false     |

  Rule: A materialized hook reaches every engine in its own native shape

    # ONE hook, TWO files, TWO formats — PARSED in its own format and
    # asserted on the actual command field under the right event, never a
    # bare file-exists or a substring of a key name.
    Scenario Outline: A team's hook reaches every engine's own hook surface
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When Alice materializes the team profile for <engine>
      Then the materialized <engine> hook configuration carries the shared hook's command, in its own native shape

      Examples:
        | engine      |
        | claude-code |

  Rule: Hooks are materialized, listed, and genuinely released

    # A claim scoped to hooks.SessionStart specifically, parsed rather than a
    # whole-file substring: ctxloom's own statusLine command also carries the
    # substring "ctxloom hook" (`ctxloom hook hud`), so a bare `contains`
    # check here is satisfied by the statusline whether or not any hook was
    # ever installed or removed.
    #
    # The project had no settings.json before the materialize: ctxloom
    # CREATED it, so the release — the empty plan over the ownership record — removes
    # the file with its last entries rather than leaving an empty husk. A
    # settings.json the user wrote is restored to their bytes instead (the
    # hand-written-numbers scenario below).
    Scenario: Hooks can be installed, inspected, and actually removed
      Given an initialized ctxloom project
      When Alice installs the hooks:
        """
        ctxloom materialize --yes
        """
      Then the command succeeds
      And the file ".claude/settings.json" registers a SessionStart hook whose command contains "hook session-start"
      When Alice inspects and then removes them:
        """
        ctxloom manage hooks check
        ctxloom materialize --release --yes
        """
      Then the command succeeds
      And the file ".claude/settings.json" does not exist

    # Asserted on a hook CTXLOOM ITSELF installs, not on one a companion
    # contributes. `session-bind` ships in taskloom's loadout, so this counted
    # a hook that was present only because the developer happened to have
    # taskloom installed — the scenario passed or vanished depending on the
    # machine, and its subject (append twice, appear once) never needed a
    # companion at all.
    #
    # Matched on the verb alone: ctxloom's own hooks are written in exec form
    # (`"command": "ctxloom", "args": ["hook", "skill-mates"]`), so the binary
    # name and the verb are separate JSON strings.
    Scenario: Re-applying hooks does not duplicate them
      Given an initialized ctxloom project
      When Alice installs the hooks twice:
        """
        ctxloom materialize --yes
        ctxloom materialize --yes
        """
      Then the command succeeds
      And the file ".claude/settings.json" contains "skill-mates" exactly 1 times

    # Hooks merge across sources by pure APPEND, and each bundle's `order:`
    # sequences only its own hooks within an event. The fixture makes
    # DECLARATION ORDER DISAGREE with `order:` on purpose — "second" is written
    # first in the YAML and ordered last — so a build that ignored the field
    # prints them the other way round and the regex catches it.
    Scenario Outline: Listing hooks shows the resolved order and where each came from
      # substitution: the Examples column headers "names the origin kind" etc. are spelled with underscores (cucumber-rs does not substitute a placeholder that has spaces).
      Given an initialized ctxloom project
      And the project already has the bundle "hooked":
        """
        name: hooked
        description: ships two pre_tool hooks whose order contradicts their position
        hooks:
          pre_tool:
            - type: command
              command: echo HOOKLIST-SECOND
              order: 200
            - type: command
              command: echo HOOKLIST-FIRST
              order: 100
        """
      And a profile "hooky" with bundle "hooked"
      When Alice asks which hooks run on a tool call:
        """
        ctxloom manage hooks list --event pre_tool --profile hooky <flags>
        """
      Then the command succeeds
      # The order claim stays a plain ordering check: this dev machine's real
      # companions (ltk, taskloom, reprise) also declare pre_tool hooks and
      # are merged in ahead of these two, so a POSITION selector is not safe
      # here — position 1 may be a companion's hook, not either of this
      # fixture's. Selecting by each hook's own unique command text is.
      And the output contains "HOOKLIST-FIRST"
      And the output contains "HOOKLIST-SECOND"
      And the output matches "HOOKLIST-FIRST[\s\S]*HOOKLIST-SECOND"
      # The origin claim, addressed as the two fields a substring of a
      # rendered "bundle X" marker would conflate.
      And the output reports "events[event=pre_tool].hooks[command=echo HOOKLIST-FIRST].source_kind" as "<names_the_origin_kind>"
      And the output reports "events[event=pre_tool].hooks[command=echo HOOKLIST-FIRST].source" as "<names_the_origin>"
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | names_the_origin_kind        | names_the_origin              |
        |               | bundle                         | ctxloom+local:hooked            |
        | --format json | bundle                         | ctxloom+local:hooked            |
        | --format text | bundle ctxloom+local:hooked    | bundle ctxloom+local:hooked     |

    # Each hook must name ONE place to go. A coarse "local" label covering every
    # declaration site at once leaves a user searching all of them, so the
    # fixture declares hooks in two DIFFERENT kinds of place and requires them to
    # be labelled distinctly: a profile the project authored, and a companion
    # binary found on PATH. Naming the profile is not enough on its own — the
    # label has to carry the KIND, since "which file do I open" and "which
    # profile is it" are different questions.
    @pending
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

    # Parsing is the claim. Event names alone appear in the human listing too,
    # so matching them would pass against a command that ignored --format json
    # and rendered text.
    Scenario: The hook list has a machine-readable form
      Given an initialized ctxloom project
      When Alice asks for the hook list as JSON:
        """
        ctxloom manage hooks list --format json
        """
      Then the command succeeds
      And the output is valid JSON
      And the JSON output array "events" contains an object whose "event" is "post_file_edit"
      And the JSON output array "events" contains an object whose "event" is "session_start"
      And every object in the JSON output array "events" has a non-empty "event"

    # A typo must not resolve to "nothing runs on that event" — a confident
    # wrong answer from an inspect command is worse than no inspect command.
    Scenario: An unknown event name is refused rather than answered empty
      Given an initialized ctxloom project
      When Alice mistypes an event name:
        """
        ctxloom manage hooks list --event pre_toll
        """
      Then the command fails
      And the output contains "pre_toll"

  Rule: The statusline is controlled separately from the hooks

    Scenario: Statusline can be disabled and re-enabled
      Given an initialized ctxloom project
      When Alice turns the statusline off and re-materializes:
        """
        ctxloom manage statusline uninstall
        ctxloom materialize --yes
        """
      Then the command succeeds
      And the file ".claude/settings.json" does not contain "hook hud"
      When Alice turns it back on and re-applies:
        """
        ctxloom manage statusline install
        ctxloom materialize --yes
        """
      Then the command succeeds
      And the file ".claude/settings.json" contains "hook hud"

  Rule: A file the user wrote by hand is never damaged

    # A rewrite of settings.json only ADDS ctxloom's own keys: whatever the
    # user wrote by hand comes back byte-for-byte, including integers no
    # float64 can hold exactly. The assertion is on the persisted digits,
    # because a round-trip through a generic JSON decoder loses them while
    # still reporting success.
    Scenario: Materializing preserves the exact numbers the user wrote by hand
      Given an initialized ctxloom project
      And the project already has the file ".claude/settings.json":
        """
        {
          "awkwardNumber": 1234567890123456789,
          "nested": { "id": -9223372036854775808 },
          "permissions": { "allow": ["Read"], "quota": 18446744073709551615 }
        }
        """
      When Alice materializes over her hand-written settings:
        """
        ctxloom materialize --yes
        """
      Then the command succeeds
      And the file ".claude/settings.json" registers a SessionStart hook whose command contains "hook session-start"
      And the file ".claude/settings.json" contains "1234567890123456789"
      And the file ".claude/settings.json" contains "-9223372036854775808"
      And the file ".claude/settings.json" contains "18446744073709551615"
      And the file ".claude/settings.json" contains "Read"

    # ctxloom is the wrong party to decide the fate of a setting it just failed
    # to read: the write is refused and the user's file is left exactly as it
    # was.
    #
    # "the command fails" is the load-bearing line here, not the message. This
    # scenario once asserted only the diagnostic and the untouched file --
    # because the exit code could NOT be relied on: a refused engine apply
    # warned and then exited 0, so a script or CI step gating on the exit code
    # believed the hooks were installed. The test was shaped around the bug.
    Scenario: A statusline ctxloom cannot read refuses the write instead of replacing it
      Given an initialized ctxloom project
      And the project already has the file ".claude/settings.json":
        """
        {
          "env": { "A": "b" },
          "statusLine": { "type": "command", "command": { "exec": "ccusage" } }
        }
        """
      When Alice materializes over a statusline ctxloom cannot parse:
        """
        ctxloom materialize --yes
        """
      Then the command fails
      And the output contains "refusing to write settings.json"
      And the file ".claude/settings.json" contains "ccusage"
      And the file ".claude/settings.json" registers no SessionStart hook whose command contains "hook session-start"

  Rule: ctxloom's private state stays out of source control

    # The private-state rules live in a NESTED, tracked .ctxloom/.gitignore, so
    # they travel with the directory and across worktrees. Root must NOT carry a
    # blanket .ctxloom rule: git does not descend into an ignored directory, so a
    # blanket there would make the nested file unreadable and every rule in it
    # silently dead. Root keeps only the patterns naming paths OUTSIDE .ctxloom,
    # which no nested file can express -- and one of them is a credential, so
    # asserting it stays is not decoration.
    Scenario: Gitignore install excludes ctxloom's private state
      # substitution: told as the user's flow, with the commands she runs.
      Given an initialized ctxloom project
      When Alice adds ctxloom's gitignore entries:
        """
        ctxloom manage gitignore install
        """
      Then the command succeeds
      And the file ".ctxloom/.gitignore" contains "/cache"
      And the file ".ctxloom/.gitignore" contains "/state"
      And the file ".ctxloom/.gitignore" contains "/sessions"
      And the file ".gitignore" does not exist

    # The shipped defect this replaced: the writer emitted a fresh comment header
    # above only the MISSING patterns, so every run added another header and the
    # copies accumulated, one of them captioning an unrelated entry. Installing
    # twice is what makes the header count meaningful -- a single install would
    # read 1 either way.
    Scenario: Repeated gitignore installs do not stack comment headers
      Given an initialized ctxloom project
      When Alice adds ctxloom's gitignore entries:
        """
        ctxloom manage gitignore install
        """
      And Alice adds ctxloom's gitignore entries:
        """
        ctxloom manage gitignore install
        """
      Then the command succeeds
      And the file ".ctxloom/.gitignore" contains "ctxloom's private working state" exactly 1 times

  Rule: Uninstalling removes what ctxloom wired and keeps what the team authored

    # BOTH halves are asserted on payload: the files ctxloom created in this
    # empty project are gone, and .ctxloom survives. A no-op uninstall that
    # prints its success line goes red on the "does not exist" assertions;
    # one that deletes .ctxloom goes red on the survival assertion. The
    # success message alone distinguishes neither.
    #
    # Uninstall is the EMPTY plan delivered against the project: the
    # ownership record says what ctxloom put there and only that is taken
    # back out. What the team authored is untouched — the hand-written
    # settings scenario above pins that half on a file the user owns.
    #
    # The precondition names the hook and the statusLine SEPARATELY, because
    # `ctxloom hook` reaches only the statusLine — the context hook is exec
    # form, its executable and its verb separate JSON strings.
    # The MCP claim PARSES the file rather than checking for the bare
    # substring "ctxloom".
    # The harness this strips is what an explicit materialize wrote.
    Scenario: Uninstall strips the harness but keeps the project's own content
      # substitution: the context file materialize wrote is asserted gone too, and the project's default profile asserted kept beside its config.
      Given an empty project directory
      When Alice wires ctxloom in:
        """
        ctxloom manage install --engine claude-code --no-pull
        ctxloom materialize --yes --degraded
        """
      Then the command succeeds
      And the file ".claude/settings.json" registers a SessionStart hook whose command contains "hook session-start"
      And the file ".claude/settings.json" contains "ctxloom hook hud"
      And the file ".mcp.json" registers no MCP server named "ctxloom"
      When Alice takes it back out:
        """
        ctxloom manage uninstall
        """
      Then the command succeeds
      # ctxloom created the settings file in this empty project, so the empty
      # plan removes it outright: nothing of the harness is left to strip.
      And the file ".claude/settings.json" does not exist
      And the file "CLAUDE.md" does not exist
      And the file ".mcp.json" registers no MCP server named "ctxloom"
      And the file ".ctxloom/config.yaml" exists
      And the file ".ctxloom/content/bundles/v2/project/profiles/default.yaml" exists

    # THE GATE FOR THE DELIVERY LAYER: uninstall is the empty plan over the
    # ownership record, so nothing of the harness is left behind; a run
    # afterwards delivers into its own session and the project stays as the
    # uninstall left it. Where the run delivered is read off the mock's
    # record, written while it ran: the run's teardown releases a delivery
    # into the project, so the install's files being absent afterwards shows
    # only that nothing was left behind. The project-side files this scenario
    # removes are an explicit materialize's.
    @pending
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
