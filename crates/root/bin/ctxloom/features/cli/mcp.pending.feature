# Ported from cli/mcp.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   mcp.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from cli/mcp.feature`.

@doc
@pending
Feature: mcp — the MCP servers ctxloom hands to every engine

  An MCP server is a tool process — a filesystem walker, a database client, a
  search index — that an AI assistant can call. Every one of them lives in a
  BUNDLE: composing a bundle that declares an `mcp:` server is what registers
  it, and that one registration is what every backend's own MCP configuration
  is generated from, and what an agent sees listed as a resource inside the
  running session. `ctxloom mcp server` reads that roster; its writes, `edit`
  and `set`, change the bundle the server lives in.

  This is the comprehensive per-noun spec: what the noun DOES, leaf by leaf,
  including the refusals and the shapes that only matter to a machine. The
  narrative version — Carol's team sharing one profile that reaches four
  engines in their own native format — is
  journeys/j000400_multi_engine.feature, which asserts what a PERSON sees.

  Rule: ctxloom's own MCP server ships in ctxloom's own companion loadout, so it needs no configuration

    # The entry whose absence costs the user every ctxloom tool. Nothing in
    # the project asks for it: ctxloom is its own companion, and a companion's
    # servers are registered in every session unconditionally, which is what
    # makes "the loadout registers the server" true by default. The SOURCE is
    # asserted alongside the name — a name alone would still be satisfied by an
    # implementation that hard-coded the entry back into the writers.
    Scenario: A project that configures nothing still registers ctxloom's own server
      Given an initialized ctxloom project
      When I run "ctxloom mcp server list"
      Then the command succeeds
      And the output contains "ctxloom"
      And the output contains "ctxloom+companion:ctxloom"

  Rule: Showing a server surfaces how it is reached, not just its name

    # `show` prints the server's own name as a header, echoed straight from
    # the argument the scenario already passed — so an entry with nothing in
    # it would still satisfy an assertion that only checks for the name. The
    # bundle identity exists nowhere in the output except because the lookup
    # really resolved the registered server.
    #
    # ctxloom's own entry is the companion's DYNAMIC declaration: served by
    # the running session's endpoint, with nothing executable on it. At rest
    # `show` DESCRIBES that — there is no command to print, and printing one
    # would be describing a server that does not exist.
    # Tabled by format: `mcp server show` is wired to emit(), so off a
    # terminal (which this harness always is) the no-flag row gets the JSON
    # GetMCPServerResult, not the text lines.
    Scenario Outline: Showing an MCP server's configuration
      Given an initialized ctxloom project
      When Alice inspects the server's configuration:
        """
        ctxloom <flags> mcp server show ctxloom
        """
      Then the command succeeds
      And the output reports "entries.0.source" as "<names the bundle>"
      And the output reports "entries.0.served_by" as "<names the endpoint>"
      And the output does not contain "mcp serve"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | names the bundle            | names the endpoint                        |
        |               | ctxloom+companion:ctxloom   | session-endpoint                          |
        | --format json | ctxloom+companion:ctxloom   | session-endpoint                          |
        | --format text | ctxloom+companion:ctxloom   | Served by: the running session's endpoint |

  Rule: There is no config-level MCP store to create in or remove from

    # The ruling this noun is shaped by: an MCP server lives in a bundle and
    # nowhere else. `create` and `remove` are not hidden or deprecated, they
    # are GONE — a project adds a server by composing a bundle and withholds
    # one with a profile's exclude_mcp.
    Scenario Outline: The config-level write leaves do not exist
      Given an initialized ctxloom project
      When I run "ctxloom mcp server <leaf> tools"
      Then the command fails
      And the output contains "unknown command"

      Examples:
        | leaf   |
        | create |
        | remove |

    # ctxloom's own registration was a config flag with two commands behind
    # it. Withholding the bundle is the replacement, so the toggles go too.
    Scenario Outline: The auto-registration toggles do not exist
      Given an initialized ctxloom project
      When I run "ctxloom mcp <leaf>"
      Then the command fails
      And the output contains "unknown command"

      Examples:
        | leaf       |
        | register   |
        | unregister |

  Rule: A config carrying the retired native mcp: block is refused, not ignored

    # A setting that looks applied and is not is the worse outcome, so a
    # config that still names the retired key fails at load with the key named
    # — the same treatment any key ctxloom does not know gets, because after
    # this ruling those are the same thing. Asserting the KEY, not just the
    # failure, is what separates this from any other reason a load might fail.
    Scenario: A native mcp: block fails the load and names the key
      Given an initialized ctxloom project
      And the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        mcp:
            auto_register_ctxloom: true
            servers:
                tools:
                    command: echo
        """
      When I run "ctxloom mcp server list"
      Then the command fails
      And the output contains "unknown key `mcp`"

  Rule: Editing a bundle's server is a round trip through the user's editor

    # `mcp server edit` is the only leaf here that hands control to an
    # external program and takes the result back, so the claim is the ROUND
    # TRIP: what the editor wrote must reach the bundle on disk. A scenario
    # asserting exit 0 would pass against an editor that was never launched.
    #
    # The fixture authors the bundle's mcp section directly because that IS
    # how a server comes to exist: there is no other store to create one in.
    Scenario: Editing a bundle's MCP server writes the editor's result back
      Given a ctxloom project with a command-rewriting editor
      And the project already has the bundle "demo":
        """
        version: 1.0.0
        description: mcp edit fixture
        mcp:
            tools:
                command: echo
                args:
                    - KEEP-THIS-ARGUMENT
        """
      When Alice edits a bundle-scoped MCP server:
        """
        ctxloom mcp server edit demo#mcp/tools
        """
      Then the command succeeds
      And the output contains "Updated MCP server"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" contains "EDITED-BY-TEST"
      # The rest of the entry must survive the round trip. An implementation
      # that rewrote the manifest from just the edited field would satisfy
      # the line above while silently dropping the arguments.
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" contains "KEEP-THIS-ARGUMENT"

    # The refusal path. A ref naming a server that is not there must fail
    # rather than launch an editor on an empty buffer and write a new entry
    # from it — which is how a typo'd ref becomes a silently created server.
    Scenario: Editing an MCP server that does not exist fails instead of creating one
      Given a ctxloom project with a command-rewriting editor
      When I run "ctxloom mcp server edit demo#mcp/absent"
      Then the command fails
      And the output contains "not found"

  Rule: Setting a bundle's server from flags changes exactly the fields passed

    # `mcp server set` is a PATCH: a typed flag reaches the entry, an untyped
    # one leaves the stored field alone, and an empty value clears. Each
    # scenario therefore asserts both halves — the passed fields landed in the
    # bundle file on disk, and a field nobody passed survived — because an
    # implementation that rewrote the whole entry from the flags would satisfy
    # the first half alone. A server's tags, notes and installation live in
    # its `.<name>.meta.yaml` sidecar beside the runtime file. The output is
    # SetBundleMCPResult (JSON off a terminal), whose status separates a new
    # entry from a changed one.
    Background:
      Given an initialized ctxloom project
      And the project already has the bundle "demo":
        """
        version: 1.0.0
        description: mcp set fixture
        mcp:
            tools:
                command: echo
                args:
                    - KEEP-THIS-ARGUMENT
                notes: KEEP-THESE-NOTES
        """

    @json-default
    Scenario: A name the bundle lacks is created as a remote server
      When I run "ctxloom mcp server set demo#mcp/search --url https://mcp.example.com/v1 --header X-Team=core --tag search-index --notes 'Team search index' --installation 'Ask core for a seat'"
      Then the command succeeds
      And the output reports "status" as "created"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/search.yaml" contains "url: https://mcp.example.com/v1"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/search.yaml" contains "X-Team: core"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/.search.meta.yaml" contains "search-index"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/.search.meta.yaml" contains "Team search index"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/.search.meta.yaml" contains "Ask core for a seat"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" contains "KEEP-THIS-ARGUMENT"

    # A repeatable flag REPLACES the stored list or map: the fixture's
    # argument is gone, the notes nobody passed are not.
    @json-default
    Scenario: Updating an existing server replaces its arguments and keeps what was not passed
      When I run "ctxloom mcp server set demo#mcp/tools --command search-mcp --arg=--stdio --env LOG_LEVEL=debug"
      Then the command succeeds
      And the output reports "status" as "updated"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" contains "command: search-mcp"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" contains "--stdio"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" contains "LOG_LEVEL: debug"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" does not contain "KEEP-THIS-ARGUMENT"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/.tools.meta.yaml" contains "KEEP-THESE-NOTES"

    # Moving to another target means clearing the old one in the same call:
    # an empty value is a clear, not an absence.
    Scenario: Clearing the old target in the same call moves a server to the session endpoint
      When I run "ctxloom mcp server set demo#mcp/tools --command= --arg= --served-by session-endpoint"
      Then the command succeeds
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" contains "served_by: session-endpoint"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" does not contain "echo"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" does not contain "KEEP-THIS-ARGUMENT"

    # A server has exactly one target. A set that would leave two is refused
    # and nothing is saved — the stored command is still the only target.
    Scenario: A set that would leave two targets is refused and saves nothing
      When I run "ctxloom mcp server set demo#mcp/tools --url https://mcp.example.com/v1"
      Then the command fails
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" does not contain "mcp.example.com"
      And the file ".ctxloom/content/bundles/v2/demo/mcp/tools.yaml" contains "command: echo"

  Rule: The bare noun answers a person and refuses a protocol client

    `ctxloom mcp` on its own lists the MCP servers this project registers, the
    way every other noun answers its own bare form. A caller that is NOT a
    person at a terminal is almost always an engine that has opened a pipe and
    is waiting for JSON-RPC, and a server listing written into that pipe is
    indistinguishable from a hang: nothing frames, nothing errors, the session
    comes up with no ctxloom tools and no cause named anywhere. So off a
    terminal the bare noun refuses, and says what IS the server: the running
    session's endpoint — no ctxloom command speaks the protocol.

    # Every command in this suite is a subprocess on pipes, which IS the
    # machine side — the harness has no terminal to offer. The human half, the
    # listing itself, is driven in internal/adapters/cli's mcp_bare_test.go, where the
    # terminal predicate can be presented either way.
    Scenario: A client pointed at the bare noun is told there is no stdio server to point at
      Given an initialized ctxloom project
      When I run "ctxloom mcp"
      Then the command fails
      And the output contains "no stdio MCP server"
      And the output contains "session's endpoint"

    # The shape a script reaches for next. Asking for JSON does not make a
    # listing safe to hand a caller that wanted a protocol stream, so the
    # refusal holds and names the leaf that produces the servers as data.
    Scenario: Asking the bare noun for JSON is refused and points at the listing leaf
      Given an initialized ctxloom project
      When I run "ctxloom --format json mcp"
      Then the command fails
      And the output contains "ctxloom mcp server list"
