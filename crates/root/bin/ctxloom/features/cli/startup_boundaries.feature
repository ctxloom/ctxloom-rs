# Ported from cli/startup_boundaries.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@pending
Feature: What a start writes, and whose pins it reads

  Starting a session is not a read-only act. Startup sweeps worktrees,
  containers and authored session files, syncs remote dependencies, and
  delivers the engine's managed surfaces — settings, hooks, MCP config and
  context — into the SESSION's own home (ruled 2026-09-21: sessions carry
  their surfaces; the project is written only by the explicit hooks
  install). That is ctxloom's job, not a side effect.

  Two boundaries around that start are asserted here. Both are NEGATIVE claims
  — "this was not written", "this did not abort" — and a negative claim is
  exactly what this project's characteristic bug satisfies for free: a command
  that exits 0 having done nothing leaves the same evidence as one correctly
  declining to act. So neither scenario stands alone. Each runs the SAME
  command a second time with the one thing that suppressed the effect removed,
  and asserts that it DOES happen. The control is the assertion; the negative
  half only means something because the positive half is next to it.

  Rule: `run --dry-run` resolves the launch without touching a managed surface

    # The flag gates the delivery, the sync and the reapers, and deliberately
    # gates NOTHING before them: a start still scaffolds a project (.ctxloom/,
    # .gitignore) and still resolves the assembled-context cache, because
    # those happen ahead of the gate. So the first dry run below is a
    # WARM-UP, not an assertion — it settles the scaffolding the flag never
    # promised to suppress, and the snapshot is taken after it. What the
    # comparison then isolates is the start itself, which is the only thing
    # the flag actually claims.
    #
    # The reapers are why this is not merely tidy. They DELETE worktrees, KILL
    # containers and MOVE authored session files — a "--dry-run" that still
    # reaped would be more destructive than its name admits, against exactly the
    # artifacts that are hardest to get back.
    Scenario: A dry-run start leaves every managed surface exactly as it found it
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the mock LLM responds "MOCK-REPLY"
      And the environment variable "CTXLOOM_NO_COMPANIONS" is set to "1"
      # The reapers are the destructive half of the gate, and unlike the sync
      # they announce nothing when there is nothing to do — so a --dry-run that
      # reaped would look identical to one that did not. This plants something
      # to reap: a real linked worktree whose owner is confirmed dead and whose
      # tree is clean, which is the ONE shape the sweep removes. Anything less
      # exact is spared or skipped for reasons that have nothing to do with the
      # flag, and would pass this scenario no matter what the flag did.
      And a crashed run left a clean orphaned per-agent worktree
      When I run "ctxloom run --dry-run --profile dev hello"
      Then no run delivered the file "MOCK_CONTEXT.md" into a session home
      When I record the project tree
      And I run "ctxloom run --dry-run --profile dev hello"
      Then the project tree is unchanged
      And no run delivered the file "MOCK_CONTEXT.md" into a session home
      # The gate guards more than the delivery: the remote sync sits behind it
      # too, and unlike the reapers it announces itself, so its suppression is
      # observable for free. The run prints this line unconditionally once
      # ShouldAutoSync() is true, and that is the DEFAULT — it returns true
      # for a nil or unset sync config, which is what this fixture has.
      # Asserted as a pair with the control below, for the usual reason: on
      # its own, "the line is absent" is also what a run that never started
      # looks like.
      And the output does not contain "syncing remote bundles and profiles from config"
      # The reaper half of the same claim. Asserted as SURVIVAL, never as
      # absence: the directory being gone is what a reap looks like, so the
      # positive form is the only one a wrong implementation cannot satisfy.
      And the orphaned per-agent worktree is still on disk
      # THE CONTROL. Same command, same binary, same snapshot — only the flag is
      # gone. Without this the assertions above are satisfied by a run that
      # never started, and by a session that was never going to be delivered
      # into in the first place. The managed surface lands in the SESSION's
      # home (the mock's context file), and the project gains exactly one
      # file: the identity marker a real run establishes and a dry run only
      # looks up — so it is absent from the snapshot both dry runs left.
      # Sessions carry their surfaces; nothing else of the start lands here.
      When I run "ctxloom run --one-shot --profile dev hello"
      Then the command succeeds
      And the output contains "MOCK-REPLY"
      And the run delivered the file "MOCK_CONTEXT.md" into a session home
      And the project tree changed only by adding ".ctxloom/project-id"
      And the output contains "syncing remote bundles and profiles from config"
      # Without this line the assertion above is satisfied by a worktree that
      # was never reapable — a fixture git could not remove, an owner that
      # could not be proven dead, a tree that was quietly dirty. This is what
      # makes "it survived the dry run" a statement about the FLAG.
      And the orphaned per-agent worktree has been reaped

  Rule: Dependency pins ride a project, never home

    # Config LAYERS home under project because settings merge sensibly. Two
    # dependency closures do not merge — their union is a set neither side
    # asked for — so home may supply CONFIG while only a project supplies a
    # CLOSURE. A home lockfile is also read-but-never-written, because `deps`
    # writes at project scope and would never revisit one, and a file in that
    # state always rots.
    #
    # The failure this prevents is total rather than partial: with pins read
    # from home, a home lockfile whose pinned revisions stop parsing aborts
    # EVERY project-less launch on the machine, on findings no supported
    # command can clear — the project-scope arm below is what that abort looks
    # like.
    #
    # The two arms differ in ONE thing: which layer the identical bytes sit at.
    # The project arm is what makes the home arm mean something — it proves the
    # lockfile is genuinely poisonous, so the quiet start is a scope decision
    # and not an inert fixture.
    #
    # THE LAUNCH REACHES THE GATE on both arms by the same route: it selects
    # ctxloom's OWN companion fragment (`isolation-axes`, from the loadout the
    # running binary carries), which needs no project and no bundle of the
    # scenario's — so the launch resolves, and the startup gate is where a
    # lockfile finding is judged. An arm that could not resolve a launch at
    # all would be refused BEFORE the gate, and both verdicts below would be
    # about nothing. (No CTXLOOM_NO_COMPANIONS here: the companion IS the
    # launch's source.)
    Scenario: A project-less start ignores a home lockfile that a project would abort on
      Given an empty project directory
      And the home config declares the mock engine label "session-owner"
      And the home already has the file ".ctxloom/lock.yaml":
        """
        schema_version: 2
        bundles:
          broken:
            sha: "abc
            pinned: true
        """
      When I run "ctxloom run --dry-run --llm session-owner -f isolation-axes hello"
      Then the startup does not abort on a fatal bundle finding
      And the command succeeds
      # THE CONTROL. The same unparseable bytes, moved to the project layer,
      # where a closure IS declared. Writing .ctxloom/lock.yaml is also what
      # makes this directory resolve as a project at all.
      Given the project already has the file ".ctxloom/lock.yaml":
        """
        schema_version: 2
        bundles:
          broken:
            sha: "abc
            pinned: true
        """
      When I run "ctxloom run --dry-run --llm session-owner -f isolation-axes hello"
      Then the startup aborts on a fatal bundle finding
      And the output contains "failed to parse lockfile"

  # A run refused before its engine launches still leaves the session the
  # resolver minted. The terminal output that said why is gone with the
  # terminal, so the session's own record must carry the reason: without it the
  # session is a directory and two timestamps. The assertion is on the
  # RECORD's bytes (class, exit status and the printed finding), because a
  # run that exited 3 and wrote nothing is this project's characteristic bug.
  @end-reason
  Scenario: A run refused by a fatal startup finding records why its session ended
    Given an empty project directory
    And the home config declares the mock engine label "session-owner"
    And the project already has the file ".ctxloom/lock.yaml":
      """
      schema_version: 2
      bundles:
        broken:
          sha: "abc
          pinned: true
      """
    When I run "ctxloom run --one-shot --llm session-owner -f isolation-axes hello"
    Then the startup aborts on a fatal bundle finding
    And the minted session's record says it ended before launch as "fatal-startup" with exit 3
    And the session record's findings include "failed to parse lockfile"
