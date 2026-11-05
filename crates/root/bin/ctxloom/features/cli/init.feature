# world: base
# Ported from cli/init.feature
# NOT ported: the token rule (S5's credentials), and the first-init rule (it
#   clones the seeded remote over the network and launches the engine: S9's
#   launch).
@doc
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

  Rule: Re-running init on an existing project changes nothing it did not ask to

    A second `init` is bookkeeping, not a rebuild. It says the directory is
    already there and leaves the project's configuration exactly as it found
    it.

    # THE CONFIG IS ASSERTED BY ITS BYTES, not by the exit code and not by the
    # message. `schema_version: 8` is the fixture's own content (it tracks
    # config.CurrentConfigVersion — bump both together) and `engine:` is a key
    # only the scaffold writes — so the pair says "your config survived" AND
    # "no scaffold ran over it", which a file-exists check cannot distinguish.
    #
    # The lockfile is the NETWORK assertion, and the reason it is here: on the
    # fresh branch init clones remotes and pulls the seeded dependencies, which
    # is what writes `.ctxloom/lock.yaml`. Its absence — with the "Seeded
    # remote" line absent too — is how this scenario proves it stayed on the
    # hermetic branch rather than merely happening to pass offline.
    Scenario: A second init reports the directory and leaves the configuration alone
      Given an initialized ctxloom project
      And the file ".ctxloom/config.yaml" contains "schema_version: 8"
      And the environment variable "CLAUDE_CODE_OAUTH_TOKEN" is set to "sk-ant-oat01-acceptance-fixture-not-real"
      When Alice runs setup again on a project that already has it:
        """
        ctxloom init
        """
      Then the command succeeds
      And the output contains "ctxloom directory already exists"
      And the output does not contain "Seeded remote"
      And the file ".ctxloom/config.yaml" contains "schema_version: 8"
      And the file ".ctxloom/config.yaml" does not contain "engine:"
      And the file ".ctxloom/lock.yaml" does not exist

    # A REGRESSION WORTH ITS OWN SCENARIO. `--remote` and `--forge` used to be
    # read and then thrown away on this branch: the only consumer of them lived
    # inside the fresh-init arm, so `ctxloom init --remote <repo>` against an
    # existing `.ctxloom` printed "already exists", exited 0, and added zero
    # remotes. Nothing about that invocation looked like it had failed.
    #
    # Registering a remote records it in `.ctxloom/remotes.yaml` and tries a
    # clone, whose failure only warns (see cli/remote.feature), so this stays
    # offline; the registry file is the effect, and the listing is read back
    # because a write that landed in the wrong place would satisfy a
    # stdout-only assertion.
    Scenario: A personal remote named on the command line is registered, not discarded
      Given an initialized ctxloom project
      And the environment variable "CLAUDE_CODE_OAUTH_TOKEN" is set to "sk-ant-oat01-acceptance-fixture-not-real"
      When Alice adds her own content repository while re-running setup:
        """
        ctxloom init --remote file:///tmp/acceptance-remote.git --forge git
        """
      Then the command succeeds
      And the output contains "Added remote"
      And the file ".ctxloom/remotes.yaml" contains "personal"
      And the file ".ctxloom/remotes.yaml" contains "acceptance-remote"
      When I run "ctxloom remote list"
      Then the command succeeds
      And the output contains "personal"

  Rule: A mistyped subcommand must not be mistaken for a bare init

    `init` is RUNNABLE as well as a namespace, and that combination made it the
    most destructive instance of the silent-namespace defect in the whole CLI.
    `ctxloom init prmopt` did not print help: it took the bare-init path,
    scaffolded `.ctxloom`, seeded remotes and cloned them, then exited 0 having
    ignored the argument entirely. Someone who mistyped one letter got a
    project set up around them and no indication that was not what they asked
    for.

    # The absence assertions are the point, so the same fixture proves it CAN
    # produce those files: `config create` scaffolds them right after, on the
    # identical empty directory. Without that, "no config.yaml" would be
    # equally consistent with the guard working and with the fixture never
    # being able to make one.
    Scenario: A mistyped subcommand fails and scaffolds nothing
      Given an empty project directory
      When Alice mistypes the one subcommand init has:
        """
        ctxloom init prmopt
        """
      Then the command fails
      And the output contains "unknown command"
      And the output contains "prmopt"
      And the file ".ctxloom/config.yaml" does not exist
      And the file ".ctxloom/remotes.yaml" does not exist
      When I run "ctxloom config create"
      Then the command succeeds
      And the file ".ctxloom/config.yaml" exists
      And the file ".ctxloom/remotes.yaml" exists

  Rule: The interview body is available on its own, for a shell or a script

    `init prompt` emits the same five-phase setup body `ctxloom init` hands the
    engine at bootstrap and `/ctxloom-init` loads in any ordinary session. It
    is a re-entry pointer, not a second copy: one body, reachable three ways,
    so they can never drift. Skipped the interview, or want to reconfigure? It
    is how you get back into it without re-running setup.

    # PHASE HEADINGS, NOT A WORD. Asserting a single token like "SCAN" is
    # satisfied by any fragment of the body that happens to survive a truncated
    # or partially-composed emit — and this command's failure mode is emitting
    # a prompt that is real but incomplete. Naming headings from opposite ends
    # of the body is what says the whole thing arrived.
    Scenario: Init prompt emits the whole setup interview body
      Given an initialized ctxloom project
      When Alice asks for the setup interview without launching one:
        """
        ctxloom init prompt
        """
      Then the command succeeds
      And the output contains "Phase 1 — Open, orient, scan"
      And the output contains "Phase 2 — Companions"
      And the output contains "Phase 4 — Agents"
      And the output contains "Phase 5 — Close"

