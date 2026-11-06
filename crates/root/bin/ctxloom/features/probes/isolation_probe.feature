# Ported from probes/isolation_probe.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@live
@pending
Feature: Isolation probe — live proof against real vendor engines

  j002200_isolation.feature's own matrix proves ctxloom's SIDE of isolation — the right
  env var, pointed at the right scratch dir, seeded with the right bytes — against
  a cooperative recording spy, never a real engine binary. That is fast, hermetic,
  and correctly scoped for a regression test. It cannot prove the other half: that
  a real vendor engine actually reads the variable it was handed, and actually
  writes only where the boundary says it may.

  This feature is that other half, built to be run on its own — for ONE engine and
  ONE axis at a time — because its job is not "pass once in this repo's CI" but
  "answer the same question again, unattended, every time an engine
  ships a new version." `just isolation-probe ENGINE AXIS` runs a single row; how
  to read a failure (vendor regression vs ctxloom regression — they read
  differently) is below, and tests/acceptance/isolation_probe.go's package doc
  covers the two live-observation
  problems this feature solves (the worktree scratch and the container's own
  writable layer both disappear the instant the run ends, so both must be caught
  DURING the run, not after).

  ONE LIVE TURN PER CELL. Every scenario below makes AT MOST one real, paid engine
  call — no retries on failure (a flaky live call is evidence, not noise to retry
  away) — because this runs against real subscriptions/API keys on every engine
  release, and volume against a subscription's own limits is the one real residual
  cost of running it at all.

  READING A FAILURE: if the response never arrives (assertion a fails), the
  credential or the engine itself is the suspect — check auth first, this is not
  an isolation bug. If the response arrives but assertions b/c/d fail, isolation
  itself is the suspect: a write landed somewhere the boundary should have stopped
  it, or ctxloom's own bookkeeping (the config-home var, the container mount plan)
  didn't do what it claims. A row's leak assertion failing IS a bug (either a
  vendor regression or a ctxloom one — the scenario's own Then step names which
  half it is asserting).

  Background:
    Given Alice has a git-backed project

  # Every engine this repo drives, on each axis. A cell's run is an agent run,
  # and an agent run authenticates only with the engine's token, taken from the
  # launching environment: by value on the host, through the read-only secret
  # mount in a container. Self-skips LOUDLY, per cell, naming the missing token
  # or engine AND the axis — see isolation_probe.go's probeTargetAuth.
  # Each Examples block below carries its own @<engine> @<axis> tag pair —
  # not decoration, the addressing mechanism: `just isolation-probe <engine>
  # <axis>` sets ACCEPTANCE_TAGS="@live && @<engine> && @<axis>" to run
  # exactly this one cell, the single-row invocation a per-engine-release
  # regression check needs. Ten separate one-row Examples blocks (rather than
  # one ten-row table) is the standard Gherkin shape for per-row tagging —
  # tags attach to an Examples: block, not to an individual row within one.
  Scenario Outline: The isolation probe proves credentials and isolation hold for <engine> under the <axis> axis
    Given the isolation probe targets "<engine>" under the "<axis>" axis
    When the probe runs it live, writing a unique token in one turn
    Then the probe's core guarantees hold for "<engine>" under the "<axis>" axis

    @claude-code @worktree
    Examples:
      | engine      | axis     |
      | claude-code | worktree |

    # container-rootless is the only container ownership this probe (or any
    # box it has ever run on) has been able to reach — see
    # isolation_probe.go's probeAxis doc and task unwatched-discharge, which
    # retired the undifferentiated "container" spelling this row used to
    # write (`runtime: container`, no longer in config-schema.json's enum).
    @claude-code @container-rootless
    Examples:
      | engine      | axis               |
      | claude-code | container-rootless |

    # container-rootful: WIRED, UNVERIFIED. No box this suite has run on has
    # had a reachable rootful docker daemon (this one runs rootless 29.3.0)
    # or podman invoked rootful, so this row has never gone green — it
    # self-skips loudly via probeContainerRuntimeForAxis until run on a
    # runner that has one.
    @claude-code @container-rootful
    Examples:
      | engine      | axis              |
      | claude-code | container-rootful |

    # opencode, on a free hosted model (liveOpencodeModel).
    @opencode @worktree
    Examples:
      | engine   | axis     |
      | opencode | worktree |

    # opencode's container axis: LIVE-VERIFIED. Measured 2026-10-13 (opencode
    # 1.18.35, which installFragment installs in the agent image, on the free
    # model, keyless, throwaway HOME/XDG/repo, rootless docker): `just
    # isolation-probe opencode container-rootless`, 1 scenario / 4 steps green
    # in 14.9s, "ISOLATION PROBE: engine=opencode axis=container-rootless
    # auth=(keyless: opencode's Auth needs no credential) result=PASSED". On
    # the live-container lane it skips loudly: that image ships no opencode
    # binary on the harness side (engineAvailable).
    @opencode @container-rootless
    Examples:
      | engine   | axis               |
      | opencode | container-rootless |

  # Back to: tests/acceptance/features/journeys/j002200_isolation.feature (the hermetic layer
  # this feature complements).
