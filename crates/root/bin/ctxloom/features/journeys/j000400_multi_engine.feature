# world: surfaces
# Ported from journeys/j000400_multi_engine.feature
# NOT ported: these Go scenarios, and why:
#   - the @live outline (needs a real engine).
#   - the codex and opencode rows (not engines of this port).


Feature: One shared profile, reaching every engine in its own native format

  A team does not standardize on one assistant. Carol's team writes one shared
  profile — a fragment, a command, an MCP server, a hook — once. Alice's teammates
  use different engines, and every one of them needs that
  same profile to reach their own engine, in whatever native shape that engine
  actually reads. ctxloom's job is to be the one place the team's standard is
  authored, and to speak every engine's own dialect on the way out — nobody
  forks the profile per engine, and nobody hand-translates a fragment into
  different config formats.

  The engines do NOT converge on one shape, and this is not incidental —
  it is the whole point of proving it here rather than asserting it in prose.
  Verified straight from each engine's own surfaces declaration
  (each engine package's surfaces.go, plus backends/mock_surfaces.go):

    | engine      | context lands in            | MCP lands in   | hooks land in         | commands land in  |
    |-------------|-----------------------------|----------------|-----------------------|-------------------|
    | claude-code | CLAUDE.md (managed markers) | .mcp.json      | .claude/settings.json | .claude/commands/ |
    | mock        | MOCK_CONTEXT.md (managed markers) | .mock/mcp.json | .mock/settings.json | .mock/commands/   |

  ONE REAL VENDOR SHIPS TODAY, and the second row is the test double. That is
  stated rather than glossed: the mechanism below — one profile, each engine's
  own native shape — is genuinely exercised across two engines, but the
  stronger claim this journey was written for, that engines from DIFFERENT
  VENDORS diverge in shape, is not currently demonstrable. mock stands in for
  a second vendor and its surfaces are all project-local, so it cannot show
  the sharpest divergence there is: an engine whose hooks, MCP servers and
  prompts have no cwd-keyed home at all and are delivered per-session at
  launch. That engine's narrowed materialize is the parked scenario below.

  An absence a report states is a
  fact a team can plan around; an absence nothing mentions is the same tree with
  a lie on top of it.

  Proving Alice's bytes were WRITTEN in each engine's own shape is NOT the same
  claim as proving any engine READ them — that is what the second, much smaller
  table below exists to prove, for the engines where it can honestly be proven
  today.

  # THE UNIQUE CLAIM HERE IS THE FAN-OUT, and it is the reason this outline
  # survives alongside four specs that each look stronger than it in isolation.
  # Every individual surface is asserted comprehensively by the noun that owns
  # it — context in cli/fragment.feature, MCP in cli/mcp.feature, hooks in
  # cli/manage.feature, commands in cli/command.feature. What ONE materialize
  # delivering ALL FOUR TOGETHER proves, no per-surface spec can see: a
  # regression that writes three surfaces and silently drops the fourth passes
  # every one of those files and fails only here. Do not fold this into them.
  #
  # LOCKED — materialization only (hermetic, no engine binary required). Every
  # row PARSES the generated file in its own format (JSON, TOML, or plain
  # markdown) and asserts the actual field, never a bare file-exists and never
  # a substring of a key name: a key name is satisfied by the file merely
  # mentioning it, which is true whether or not any content landed.
  #
  # READ EVERY CONTEXT ROW CAREFULLY. A context approach must write from
  # engine.ContextInputs.Text — the assembled context, which materialize
  # populates — and never re-assemble it from the fragment items itself: a
  # context surface that did would be a second assembly that can disagree with
  # the first, exit 0 with the wrong bytes written. Adding an engine here is
  # adding a ROW, not new Go.
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

  # A HOME-KEYED engine gets its own scenario, not a row, because its answer is
  # genuinely different: one surface materializes and three are DECLARED as
  # launch-only. Keeping it as a row would mean either asserting files the
  # product deliberately does not write, or quietly dropping it from the
  # journey — and dropping it is how a narrowing becomes a regression nobody
  # notices.
  #
  # ABSENCE OVER THE WHOLE TREE, never "the file I guessed is missing": the
  # interesting failure is a fallback landing somewhere nobody thought to look,
  # which a single-path check cannot see. And the REPORT is asserted alongside,
  # because a narrowing a user is not told about is indistinguishable from a
  # loss.
  #
  # THE CLAIM IS NARROWING-IS-DECLARED, which is why the last step matters most:
  # a user must be told which surfaces did not land, or the report reads as
  # this engine silently losing them. The subject is mock-launch, whose
  # declaration carries a context surface and no other: the plan routes the
  # rest to nothing as accepted losses, and the report names each with the
  # reason — the engine declares no approach for it. Both halves, because
  # skipping without reporting is a silent no-op and reporting without
  # skipping names a surface as absent while its file sits in the tree.
  #
  # The first four steps must NOT be read as "passes because nothing exists".
  # The team profile genuinely carries a hook, a server and a command, so the
  # inputs have something to deliver; a profile that asked for nothing would
  # satisfy the absence steps without the question ever being posed.
  Scenario: A home-keyed engine materializes its native context, and reports the surfaces it does not carry
    Given Carol's team profile carries a shared fragment, command, MCP server, and hook
    When Alice materializes the team profile for mock-launch
    Then the materialized mock-launch context carries the shared fragment's marker, in its own native shape
    And no mock-launch surface anywhere in the materialized tree carries the shared hook's command
    And no mock-launch surface anywhere in the materialized tree carries the shared MCP server's command
    And no mock-launch surface anywhere in the materialized tree carries the shared command's body
    And the materialize report names each surface mock-launch does not carry, with a reason

  # Regression coverage for taskloom lanky-plop (P0 data loss): materializing a
  # profile for an engine with a native context file must never destroy a team's hand-authored
  # CLAUDE.md / AGENTS.md — content outside ctxloom's managed markers must
  # survive byte-for-byte, and ctxloom's own content must still land alongside
  # it. Turning the ownership record's appended-section claim
  # (present.AppendedSection; fsstatic's section transition) into a bare
  # whole-file write makes this scenario fail for exactly that reason — the
  # hand-authored line is gone, not merely unasserted.
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
