# Ported from cli/init.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   init.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from cli/init.feature`.

@doc
@pending
Feature: init — the setup interview, and what it does to a project that already has one

  Covers: `ctxloom init` and `ctxloom init prompt`.

  `init` is the command someone runs once. On a directory with no `.ctxloom` it
  scaffolds the project, seeds and CLONES the trusted `ctxloom-default` remote,
  pulls the dependencies the seeded default profile resolves through, and — on
  a terminal — hands the whole thing to your engine for one setup interview:
  companions, then profiles and content, then agents bound to them. It writes
  no engine file into the project: the interview, like every `ctxloom run`, is
  a session carrying ctxloom's hooks and MCP server in its own session home
  (ruled 2026-09-21: sessions carry their surfaces). It is a solo core verb
  with exactly one thing under it, `init prompt`, which re-emits that
  interview body for a shell or a script.

  WHAT THIS FILE COVERS, AND WHAT IT DELIBERATELY DOES NOT. A first-time init
  on a NEW `.ctxloom` clones the seeded default remote and pulls its
  dependencies, so it reaches the NETWORK. The one fresh-project scenario here
  answers init's questions over a real pty with the network refused and the
  setup session skipped (`--skip-launch`): it proves what the QUESTIONS write,
  not what the clone or the setup session do. Every other scenario runs
  against a project that ALREADY has a `.ctxloom` directory, which takes
  init's already-exists branch: no questions, no scaffold, no clone, no
  dependency pull. That branch is not a lesser case — it is what every re-run
  of `init` does, and it is where the flags that were once silently ignored
  live. The setup session itself is launched over a real pty, against a mock
  engine, in j000200_setup.feature — on an existing project, so without the
  questions.

  The interactive interview needs a terminal too. Off one, init resolves the
  engine and returns without launching anything, which is precisely what makes
  these scenarios hermetic. Before that it checks for the token every agent
  authenticates with: the re-run scenarios export a fixture one, and the rule
  below covers what happens without it.

  Rule: Init never takes the agent token; without one it fails, saying how to create and export it

    Every agent ctxloom launches authenticates with a long-lived token the
    human creates with the engine's own flow (claude: `claude setup-token`)
    and exports. ctxloom instructs; it never captures or stores the token. On
    a terminal with none exported, init runs that flow on the human's own
    terminal and prints the line to export it. Off a terminal there is nobody
    to run it for, so init names the steps — in the engine's own wording, the
    one `ctxloom auth` and `ctxloom run` show — and exits non-zero, as it does
    under --skip-launch and after the terminal flow too: a project whose
    agents cannot authenticate is not ready, whatever the mode. The config IS
    written and stays written, and init says so. Once the token is exported,
    re-running init launches the interview.

    Scenario: Off a terminal with no token exported, init names how to create and export it
      Given an initialized ctxloom project
      And the environment variable "CLAUDE_CODE_OAUTH_TOKEN" is set to ""
      When Alice re-runs setup with no agent token exported:
        """
        ctxloom init
        """
      Then the command exits with code 1
      And the output contains "config is written"
      And the output contains "claude setup-token"
      And the output contains "export CLAUDE_CODE_OAUTH_TOKEN"
      And the output contains "or store it in your secret manager"

  Rule: A first init asks its questions at the terminal, and says what it chose for the rest

    Init's questions are asked in-process, on the user's own terminal, before
    any engine is involved. Two things are not asked: init says, in one line,
    that a delegation commits uncommitted work (the parent's off-branch before
    a child's worktree is carved from it, reaching the parent's branch only
    once the child has started; the child's on its own branch when its run
    ends), and,
    in another, which headless posture the default agent takes and how to
    change it. The project it leaves has a default agent whose headless runs
    accept file edits without asking.

    # Hermetic: a stub `claude` makes claude-code the only installed engine
    # (so init announces it rather than asking), the https clone of the seeded
    # remote is refused, and --skip-launch stops before the auth probe and
    # the setup session. The stub records any execution, which is how "no
    # engine ran" is proven rather than assumed.
    Scenario: A first init writes the answers given at the terminal
      Given an empty project directory
      And claude-code is the only engine installed, and nothing reaches the network
      When Alice answers init's questions at a terminal, taking every recommendation
      Then init told her which engine it chose, "Using claude-code (only available engine)"
      And no engine ran during the interview
      And the file ".ctxloom/config.yaml" contains "type: claude-code"
      And the default agent's headless posture is "acceptEdits"
      And init's terminal output says "Delegation commits: before a delegated agent's worktree is carved"
      And init's terminal output says "ctxloom agent edit default --permissions <posture>"
