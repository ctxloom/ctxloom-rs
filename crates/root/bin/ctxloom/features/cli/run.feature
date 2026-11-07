# world: base
# Ported from cli/run.feature (rev f36e8dfb2)
# Ported from journeys/j001200_recall.feature (rev f36e8dfb2): the Rule on resuming a session
# Ported from journeys/j002200_isolation.feature (rev f36e8dfb2): the last three Rules
# NOT ported: the scenarios tagged @pending (the mock engine's recorded input,
#   one-shot, dry-run, tag and agent selection, deny_tools, the opencode TUI):
#   they need `run` to launch an engine. Plan override: clap's usage error says
#   "unexpected argument" where cobra said "unknown flag"; the step maps one to
#   the other (plan 8.2).
@doc
Feature: run — assembling a project's context and handing it to an engine

  `run` is the core operation, and the only one whose whole job is to put
  something in front of a model. It resolves which context this invocation
  gets — a profile, loose fragments, a tag selection, or a named agent binding
  — assembles it, and launches the configured engine with that context and a
  prompt.

  Surfaces covered here:

    ctxloom run [prompt...]              (interactive: a session)
    ctxloom run --one-shot [prompt...]   (one turn, then exit)
    ctxloom run --dry-run                (render what would be sent; send nothing)
    ctxloom run --profile/-p <name>
    ctxloom run --tag/-t <tag>
    ctxloom run --command/-r <name>      (a saved command as the prompt)
    ctxloom run --agent <name>

  A RUN IS EITHER A SESSION OR A SINGLE TURN, and `--one-shot` is how a caller
  says which. The flag names the MODE, not its output: every mode prints, and
  what actually differs is that a one-shot takes one turn and exits. Bare
  `ctxloom run` is the session — interactive and terminal-owning; the
  journeys drive it over a pty (j000200_setup.feature), and so does the one
  opencode scenario at the end, whose scripted engine records what it was
  launched with and exits.

  EVERY OTHER SCENARIO HERE DRIVES THE MOCK ENGINE, never a real one. That is not a
  compromise — it is what makes the full path OBSERVABLE. The mock records the
  setup payload that actually crossed the launch wire, so a scenario can assert
  what the model RECEIVED rather than that the command exited 0. `the mock
  recorded input contains` is the strongest assertion available on this
  surface, and the reason the deny_tools regression below is catchable at all:
  a per-hop test cannot see a field silently dropped crossing the proto wire,
  because the converter and its mirror struct read as a matched pair.

  Rule: The mode has one spelling, and asking for two at once is a refusal

    A caller reaching for a mode by a name it does not have must not get a run
    they did not ask for. An unknown flag is an error, and that error is what
    makes the real spelling discoverable; an alias quietly accepted alongside
    it would leave two names for one mode forever — in help, in scripts, and in
    every future reader's head.

    Scenario: A flag that is not the mode's name is refused rather than guessed at
      Given an initialized ctxloom project
      When Alice reaches for the mode by a name it does not have:
        """
        ctxloom run --print hello
        """
      Then the command fails
      And the output contains "unknown flag"

    # `--agent` names a BINDING that already carries its own composed profiles,
    # engine and runtime, so combining it with the flags that assemble context
    # by hand is a caller asking for two different answers to the same
    # question.
    @pending
    Scenario: An agent binding and a hand-assembled profile cannot both decide the context
      Given an initialized ctxloom project
      And a profile "dev" exists
      When Alice names both a binding and a profile:
        """
        ctxloom run --agent developer --profile dev hello
        """
      Then the command fails
      And the output contains "none of the others can be"

  @pending
  Rule: A one-shot takes exactly one turn, and refuses to take an empty one

    Scenario: Run hands the assembled context to the model and returns its reply
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the mock LLM responds "MOCK-REPLY"
      When Alice asks a question with her project's context attached:
        """
        ctxloom run --one-shot --profile dev unicorn-prompt
        """
      Then the command succeeds
      And the output contains "MOCK-REPLY"
      And the mock recorded input contains "FRAGMENT-BODY-testing"
      And the run's session record carries no MCP endpoint

    # THE CHARACTERISTIC BUG, in its purest form. A one-shot gets exactly one
    # turn, so an empty prompt asks nothing at all: the run would exit 0, print
    # a reply to a question nobody posed, and `ctxloom run --one-shot > out.txt`
    # would leave a file whose emptiness looks like the model's answer. It is a
    # refusal instead, and the message names every door a prompt could have
    # arrived through so the caller knows which one they forgot.
    Scenario: A one-shot with no prompt from any door refuses to run
      Given an initialized ctxloom project
      And the mock LLM responds "MOCK-REPLY-NEVER-ASKED"
      When Alice starts a one-shot without asking anything:
        """
        ctxloom run --one-shot
        """
      Then the command fails
      And the output contains "nothing to run"
      And the output does not contain "MOCK-REPLY-NEVER-ASKED"

    # EVERY RUN MINTS A HARP, and the mint is the first thing that can fail.
    # A run that cannot be named has no session dir, no coordinator, no place
    # for its transcript, and no address a child could reach back to — so a
    # run that shrugged past the failed mint would spawn an engine whose every
    # later phase fails for a cause three phases behind it. The refusal is
    # here, at the mint, before any engine process exists. What the binary
    # can show is WHERE the failure was reported from: a run that reached a
    # plugin relays that plugin's failure, a run refused at the mint has no
    # plugin to relay. (The mock's record cannot testify here — a runner that
    # was spawned and then refused inside setup leaves no record either; the
    # cli package's unit test observes the spawn seam itself.)
    Scenario: A run whose session cannot be minted refuses before spawning the engine
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the mock LLM responds "MOCK-REPLY-UNNAMED-RUN"
      And the session store cannot be opened
      When Alice starts a one-shot that has nowhere to record its session:
        """
        ctxloom run --one-shot --profile dev unicorn-prompt
        """
      Then the command fails
      And the output contains "session"
      And the output does not contain "AI plugin failed"

    # ONE-SHOT AS A UNIVERSAL REDUCER: with no prompt on the command line, a
    # one-shot reads it from piped stdin, so anything a shell can produce can
    # be summarized, classified or rewritten with the project's own context
    # attached. The assertion is on what CROSSED THE WIRE — the piped text
    # reaching the model — because stdout carrying the mock's canned reply
    # would look identical whether the pipe was read or silently discarded.
    Scenario: A one-shot reads its prompt from piped input
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the mock LLM responds "MOCK-REPLY"
      When I run "ctxloom run --one-shot --profile dev" with input:
        """
        PIPED-PROMPT-summarize-this
        """
      Then the command succeeds
      And the output contains "MOCK-REPLY"
      And the mock recorded input contains "PIPED-PROMPT-summarize-this"

    # A saved command is content, stored in a bundle and resolved by name, so
    # `-r` is how a team's reviewed prompt becomes the thing that runs. The
    # BODY has to reach the model: a run that resolved the command, dropped its
    # content and launched with an empty prompt would still exit 0.
    Scenario: A saved command becomes the prompt
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a command "review" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the mock LLM responds "MOCK-REPLY"
      When Alice runs the review prompt her team saved:
        """
        ctxloom run --one-shot --profile dev -r review
        """
      Then the command succeeds
      And the output contains "MOCK-REPLY"
      And the mock recorded input contains "COMMAND-BODY-review"

  @pending
  Rule: A dry run renders what would be sent, and sends nothing

    `--dry-run` is the inspection surface: it stops before anything stateful or
    interactive happens — no session-index write, no coordinator, no task
    seeding, no isolation, no engine launch — and prints the composed context.

    # "the output contains dev" was satisfied by the dry-run header, which
    # names the profile whether or not assembly produced anything: a run that
    # loaded zero fragments and assembled an empty context passed. What a dry
    # run is FOR is showing what would be sent, so the fragment's REF and its
    # BODY are both asserted.
    #
    # The mock is configured and its reply must NOT appear. That is the half
    # that says "sends nothing": without it, a dry run that rendered the
    # context AND launched the engine anyway would pass every other assertion
    # here.
    Scenario: A dry run shows the composed context without launching a model
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the mock LLM responds "MOCK-REPLY-NEVER-SENT"
      When Alice checks what would be sent before sending it:
        """
        ctxloom run --dry-run --profile dev hello
        """
      Then the command succeeds
      And the output contains "dev"
      And the output contains "demo#fragments/testing"
      And the output contains "FRAGMENT-BODY-testing"
      And the output does not contain "MOCK-REPLY-NEVER-SENT"

    # U082-F03 regression: an explicit -t selection matching zero fragments
    # previously assembled Context: "" with a nil error and NO warning at all —
    # `ctxloom run -t <tag-that-matches-nothing>` exited 0 having delivered no
    # context whatsoever, indistinguishable from a working, empty-by-design
    # session. No per-hop test caught it: AssembleContext's own unit tests
    # asserted the (correct) empty Context string and stopped there, never
    # asking whether the CALLER could tell that apart from "nothing was asked
    # for". This asserts the CLI's own observable behavior — a non-zero exit
    # naming the tag — not just that assembly returned successfully.
    Scenario: An explicit tag selection matching nothing fails loudly, not silently
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      When Alice selects by a tag nothing carries:
        """
        ctxloom run --dry-run -t no-such-tag-anywhere hello
        """
      Then the command fails
      And the output contains "no-such-tag-anywhere"

  @pending
  Rule: What the profile declares reaches the launched backend

    A profile carries more than fragments: tool denials, permission posture,
    the engine it prefers. None of that is worth anything if it is lost between
    the command line and the process that runs.

    # deny_tools must survive every hop from the command line to the launched
    # engine. A per-hop test cannot see a field dropped between two hops whose
    # converter and mirror struct read as a matched pair; only tracing the
    # payload tip-to-tail (argv -> profile resolution -> the launch carrier ->
    # the runner's managed payload) does. This asserts the WIRE PAYLOAD the backend actually
    # received, not just that the command exited 0.
    Scenario: A tool denial configured on a profile reaches the launched backend
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a profile "guarded" with bundle "demo" and deny_tools "WebFetch"
      And the mock LLM responds "MOCK-REPLY"
      When Alice runs under a profile that forbids a tool:
        """
        ctxloom run --one-shot --profile guarded unicorn-prompt
        """
      Then the command succeeds
      And the mock recorded input contains "WebFetch"

  @pending
  Rule: What the launch learned about its own environment reaches the agent

    A launch computes findings about the ground the agent is about to stand
    on — a config key it does not know and therefore IGNORED, an isolation axis
    that degraded, a companion it expected and withheld. Until now those went
    to stderr, which the human may not be watching and the agent never sees;
    an agent whose config half-loaded then works confidently on the wrong
    premise and reports success. So the findings ride INTO the started agent's
    context, as doctor's own rows, and a flag opts out for the scripted runs
    where that is measured waste.

    # Both scenarios assert on the mock's RECORD of what crossed the launch
    # wire, never on the CLI's own stderr: the claim is that the AGENT received
    # the finding, and stderr is exactly the surface this rule exists to stop
    # relying on. Degraded mode is what lets a config finding reach a launch
    # at all — in strict mode the startup gate aborts on it first, which is
    # the other, already-covered half of the same doctrine. It is spelled as
    # the environment switch rather than the flag because the engine is a
    # child ctxloom process with a startup gate of its own, and only the
    # environment reaches it.
    Scenario: A config finding the launch proceeded past is delivered into the agent's context
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        runt1me: host
        """
      And the mock LLM responds "MOCK-REPLY"
      And the environment variable "CTXLOOM_DEGRADED" is set to "1"
      When I run "ctxloom run --one-shot --profile dev unicorn-prompt"
      Then the command succeeds
      And the mock recorded input contains "FRAGMENT-BODY-testing"
      And the mock recorded input contains "DOCTOR-CHECK-STARTUP-FINDINGS"
      And the mock recorded input contains "unknown key `runt1me`"

    # A profile that VETOES a server with exclude_mcp means it, and the items
    # linked to that server — a hook among them — are withheld with it. The
    # agent is still told, since an item it cannot see otherwise reads as one
    # never authored; but a withhold is not a fault, so this launch runs in
    # STRICT mode (no CTXLOOM_DEGRADED) and must start, with the withhold listed.
    Scenario: A server vetoed with exclude_mcp withholds its linked items, and a strict launch starts and lists them
      Given an initialized ctxloom project
      And the project already has the bundle "linked":
        """
        version: "1.0"
        mcp:
          think:
            command: think-server
            tags: [ctxloom:link_id=think]
        fragments:
          guide:
            content: "LINKED-GUIDE"
            tags: [ctxloom:link_id=think]
          plain:
            content: "PLAIN-FRAGMENT"
        hooks:
          session_start:
            - command: think-warmup
              tags: [ctxloom:link_id=think]
        """
      And the project already has the file ".ctxloom/content/bundles/v2/project/profiles/vetoed.yaml":
        """
        description: vetoes the think server
        bundles:
          - linked
        exclude_mcp:
          - think
        """
      And the mock LLM responds "MOCK-REPLY"
      When I run "ctxloom run --one-shot --profile vetoed unicorn-prompt"
      Then the command succeeds
      And the mock recorded input contains "PLAIN-FRAGMENT"
      And the mock recorded input does not contain "LINKED-GUIDE"
      And the mock recorded input contains "DOCTOR-CHECK-WITHHELD-ITEMS"
      And the mock recorded input contains "linked#fragments/guide"
      And the mock recorded input contains "hook ctxloom+local:linked#hooks/session_start/0"
      And the mock recorded input contains "which this run was not granted"

    # The opt-out, paired with the positive scenario above and with the
    # assembled context still present in the record — so an absent finding is
    # evidence of the flag, not of a launch that delivered nothing.
    Scenario: --no-startup-findings withholds the findings and nothing else
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        runt1me: host
        """
      And the mock LLM responds "MOCK-REPLY"
      And the environment variable "CTXLOOM_DEGRADED" is set to "1"
      When I run "ctxloom run --one-shot --no-startup-findings --profile dev unicorn-prompt"
      Then the command succeeds
      And the mock recorded input contains "FRAGMENT-BODY-testing"
      And the mock recorded input does not contain "DOCTOR-CHECK-STARTUP-FINDINGS"
      And the mock recorded input does not contain "unknown key `runt1me`"

  @pending
  Rule: An opencode label runs opencode's own TUI on ctxloom's terminal

    # A SCRIPTED opencode (the label's binary_path) stands in for the real
    # CLI: no model, no network. It records what it was handed and exits, so
    # the record proves the interactive launch end to end — the TUI's argv
    # (the prompt a flag, not stdin), a terminal on both ends, the assembled
    # guidance in the AGENTS.md delivered to its config dir, Alice's own
    # tui.json carried in beside it (the relocated config would otherwise
    # drop her theme), the snapshot default a TUI's /undo needs, and the
    # bridge plugin that carries her session's hooks, mail and wake.
    @opencode
    Scenario: An opencode label's run is opencode's TUI on a terminal, with the guidance and Alice's TUI preferences
      Given Alice has an opencode label served by a scripted opencode TUI, and a fragment of guidance
      And Alice keeps her own opencode TUI preferences
      When Alice runs the opencode label interactively with the prompt "hello tui"
      Then the scripted opencode TUI ran on a terminal with the prompt as a flag, the guidance and Alice's preferences in its config dir
      And the bridge plugin in its config dir fires ctxloom's session-start and mail-drain for opencode, and the TUI is told its wake socket

  @pending
  Rule: Resuming a session folds in its recorded conversation, and leaves the record as it was

    `run --session <harp>` reads the session's recorded conversation, the one
    ctxloom keeps whatever engine held it, and folds it into the run's
    context. The fixture's session carries one marker in its title and
    another only in its conversation, so "found by its title" and "resumed
    with its conversation" are told apart.

    Scenario: Resuming the March session puts that conversation back in front of a model
      # substitution: the journey's March fixture is a Given of the scenario, not a Background.
      Given a design question everyone remembers deciding and nobody remembers why
      When I run "ctxloom run --session amber-quiet-heron --dry-run -p default"
      Then the assembled context carries the conversation she had in March

    # A resume that consumed, truncated or rewrote what it read would fail the
    # second person to ask. The record is read back by its bytes.
    Scenario: Recalling a session does not consume it
      # substitution: the journey's March fixture is a Given of the scenario, not a Background.
      Given a design question everyone remembers deciding and nobody remembers why
      When I run "ctxloom run --session amber-quiet-heron --dry-run -p default"
      Then the canonical transcript she recalled from is still on disk, untouched

  @pending
  Rule: A worktree run hides its per-agent config from the project's git

    # A worktree run's per-agent config edits are kept out of the shared
    # project tree through .git/info/exclude, not left as new files a
    # developer could merge back by accident.
    Scenario: A worktree run leaves the project tree clean
      # substitution: the journey's Background is the scenario's first Given
      Given Alice has a git-backed project with a mock agent
      When Alice runs the mock agent under workspace "worktree" with prompt "cleanup-check"
      Then none of the per-agent worktree config artifacts appear in the project's git status
      And the shared git exclude file carries the ctxloom per-agent worktree config block

  @pending
  Rule: Every gate that loses a requested container refuses the run, --degraded or not

    The fixture's agent asks for a container. One flag cannot mean both "I
    accept a thinner context" and "I accept no sandbox", so --degraded never
    rescues a lost container: each outline's two rows differ only in the flag,
    and a single row could not tell "--degraded is ignored" from "--degraded
    was never passed". Each gate keeps its own outcome step, which asserts its
    own finding and the other gates' absence, since every row exits 3 and the
    wording is all that tells the gates apart.

    # A dry run does not stop at the refusal: it renders the plan it could
    # compute, lists the finding with its fix, and exits as the run would,
    # whatever the format. Both rows read the probed environment off the plan
    # (runtime unavailable, reach unknown), never the host the refused run
    # does not fall back to.
    Scenario Outline: Previewing a container with no runtime shows the plan, then refuses as the run would
      # substitution: the journey's Background is the scenario's first Given
      Given Alice has a git-backed project with a mock agent
      When Alice previews the container-bound agent with flags "<flags>"
      Then the run aborts with an isolation finding
      And the preview rendered its plan and the refusal's fix

      Examples:
        | flags         |
        |               |
        | --format json |

    # The START gate: the runtime is reachable, so a container is selected,
    # and the image cannot be produced (an override image is run as-is and
    # never built). The finding says the session is NOT sandboxed.
    Scenario Outline: A container that was selected and cannot start REFUSES, and --degraded does not rescue it
      # substitution: the journey's Background is the scenario's first Given
      Given Alice has a git-backed project with a mock agent
      When Alice runs a container-bound agent whose image cannot be produced, with flags "<flags>"
      Then the run aborts at the container START gate

      Examples:
        | flags      |
        |            |
        | --degraded |

    # The same gate with both axes asked for: the container tier is the
    # worktree-backed one, and the tier below it is a plain host worktree,
    # which looks like isolation and is the degrade most tempting to accept.
    # A check that recognised only the live-dir container would pass the row
    # above and let this run land on the host.
    Scenario Outline: A containerized worktree that cannot start REFUSES rather than keeping the worktree on the host
      # substitution: the journey's Background is the scenario's first Given
      Given Alice has a git-backed project with a mock agent
      When Alice runs a container-bound agent whose image cannot be produced, with flags "--workspace worktree <flags>"
      Then the run aborts at the container START gate

      Examples:
        | flags      |
        |            |
        | --degraded |

    # The transport-start gate: the daemon accepts the run and the container
    # never reaches running. The session was already told it had a boundary,
    # so falling back would run on the host something that believes it is
    # sandboxed. The outcome step also asserts the engine never ran: an abort
    # that still launched it would pass on the exit code alone.
    Scenario Outline: A container accepted by the daemon that never runs is fatal in BOTH modes
      # substitution: the journey's Background is the scenario's first Given
      Given Alice has a git-backed project with a mock agent
      When Alice runs a container-bound agent whose container dies at the daemon, with flags "<flags>"
      Then the run aborts because the container never reached running state

      Examples:
        | flags      |
        |            |
        | --degraded |

  @pending
  Rule: A run whose engine cannot authenticate, or cannot load its MCP servers, is refused before the engine starts

    # session_home: host leaves claude's MCP servers in the project's own
    # .mcp.json, which claude loads only in a repository Alice trusted in her
    # own claude: in one she never trusted, the run is refused.
    Scenario: An in-tree AGENT run that declares session_home: host in a project Alice never trusted is refused
      # new: Go's scenario of this name says engine_home; the vocabulary ruling (2026-11-06) renamed the setting to session_home, so the scenario carries the new name
      Given Alice has a git-backed project
      And Alice has whatever credentials "claude-code" needs to authenticate
      And Alice has not trusted the project in her own claude
      And Alice's agent declares session_home "host"
      When Alice runs the isolated "claude-code" agent under workspace "none"
      Then the run is refused because an untrusted repository's session cannot load the project's MCP file

    # A login session needs the login: with no claude login on the host, it
    # is refused before any engine is spawned, naming what to do. The host and
    # the container refuse alike.
    Scenario: Alice's login session with no login on the host is refused, naming auth token
      # substitution: the agent declares session_home where Go's step says engine_home (vocabulary ruling, 2026-11-06)
      Given Alice has a git-backed project
      And Alice has no "claude-code" credentials or API key on the host
      And Alice has no "claude-code" credentials on the host
      And Alice's agent declares session_home "session"
      And Alice's own session authenticates with "login"
      When Alice runs the isolated "claude-code" agent under workspace "none"
      Then the run is refused without any isolation finding, naming "auth: token"

    # A token session authenticates with a token the human mints with `claude
    # setup-token` and exports; ctxloom never captures or stores one. With
    # none exported the run is refused before any engine is spawned, naming
    # how to mint and export it. An exported API key does not stand in.
    Scenario: A token session with no exported token is refused, naming claude setup-token
      # substitution: the agent declares session_home where Go's step says engine_home (vocabulary ruling, 2026-11-06)
      Given Alice has a git-backed project
      And Alice has no "claude-code" credentials or API key on the host
      And Alice has set the "claude-code" API key in the environment
      And Alice's agent declares session_home "session"
      When Alice runs the isolated "claude-code" agent under workspace "none"
      Then the run is refused without any isolation finding, naming "claude setup-token"

  @pending
  Rule: --engine runs the engine it names, with that engine's defaults

    A run says what runs in one of two ways: `--agent <name>` runs an agent,
    and `--engine <engine>` runs that engine with its engine defaults, with
    no agent named. An engine ctxloom does not have is refused by name, and
    nothing runs.

    Scenario: A run naming an engine runs that engine with its defaults
      # new: run takes --engine.
      Given an initialized ctxloom project
      And the mock LLM responds "MOCK-REPLY"
      When Alice runs the mock engine by name:
        """
        ctxloom run --one-shot --engine mock hello
        """
      Then the command succeeds
      And the run was on the mock engine, with its engine defaults

    Scenario: A run naming an engine ctxloom does not have is refused, naming it
      # new: run refuses an unknown engine.
      Given an initialized ctxloom project
      When Alice names an engine ctxloom does not have:
        """
        ctxloom run --one-shot --engine bogus-engine hello
        """
      Then the command fails
      And the output contains "bogus-engine"
      And no model was asked anything
