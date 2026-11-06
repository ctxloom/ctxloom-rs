# Ported from journeys/j002300_cross_engine_delegation.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j002300_cross_engine_delegation.feature with its steps (gherkin_lint.test.rs).

@pending
Feature: Delegation — each child sees only its own context, over a real two-way bus

  ctxloom's differentiator is not "an agent can spawn another agent" — every
  competitor with a coordinator loop can do that. It is that each child sees
  ONLY its own composed profile (not the coordinator's, not a sibling's), and
  that coordinator and child talk to each other over a real, durable message
  bus, not a synchronous return value. j002100_delegation.feature proved the
  privilege half of delegation (MCP servers, permission modes, the journaled
  audit trail) with `agent_run` alone. This journey proves the half
  `agent_run` cannot: that two children genuinely see DIFFERENT content
  (asserted on the payload a child itself emits, never a config diff), that
  the bus carries real words between coordinator and child in both
  directions, and that a delegated child on a real engine completes that
  round trip, isolated and not.

  # WHAT THE HERMETIC TIER READS, and who writes it. Both observables below
  # are produced by the child's OWN runner process, never by the coordinator:
  # each child's canonical transcript
  # (~/.ctxloom/sessions/<harp>/transcripts/transcript.jsonl —
  # internal/adapters/transcript/record.go's documented, first-party schema, not a
  # scrape) proves distinct context and the coordinator->child half of the
  # bus (a REAL agent_send call, content verified in the child's own recorded
  # next turn); the coordinator's own spool, read on disk, proves
  # the child->coordinator half through the runner's automatic turn report
  # (coord.EngineHost, spoolturnresult.go) — no model reasoning involved. Only
  # agent_send-by-model-decision needs a real engine, and that is the @live
  # tier's job. The @negative-probe scenario withholds the runner and shows
  # neither observable survives its absence.

  # LOCKED — requirement 3 (distinct context): each child's OWN reported
  # turn is read straight off its canonical transcript, never off an
  # in-process struct and never off the sibling's. The guidance is in that
  # turn because the mock's OPENING turn echoes the context its context
  # surface delivered (the first turn itself is the prompt alone, onectx);
  # a profile that never reached the child cannot appear. BREAK-POINT: if a future
  # change made the mock's composed fragments leak across agents (e.g.
  # resolving from the caller's profile instead of the callee's),
  # "librarian"'s reported turn would start carrying "cartographer"'s
  # guidance and this scenario goes red for exactly that reason.
  Scenario: Two children delegated to the same engine each report guidance found only in their OWN composed profile
    Given Alice's coordinator can delegate to two agents, "librarian" and "cartographer", each carrying its own distinct guidance in its own profile
    And a session owner is standing
    When the agent calls tool "agent_run" with:
      | role         | librarian |
      | input.prompt | go        |
    Then the tool call succeeds
    And "librarian"'s session harp is remembered
    And "librarian"'s reported turn carries its own guidance, not "cartographer"'s
    When the agent calls tool "agent_run" with:
      | role         | cartographer |
      | input.prompt | go           |
    Then the tool call succeeds
    And "cartographer"'s session harp is remembered
    And "cartographer"'s reported turn carries its own guidance, not "librarian"'s

  # The last step is F4 of row worried-chief: a delegated child is handed its
  # mail ONCE, as its turn, by its runner. The turn-start mail-drain hook is
  # the session OWNER's reader; declared for a child it would claim the same
  # file from the child's in/ during that turn and hand it a second time.
  # Only that hook's claim creates in/claimed/, so its absence is the proof.
  # LOCKED — the coordinator->child half of requirement 4, on real
  # infrastructure: a genuine `agent_send` MCP tool call, addressed by the
  # child's own runtime-minted harp, delivered as its next turn
  # (coordinator.go's peerSend/driveQueued), with the exact sent content
  # verified in what the child itself reported next. The ECHO token exists
  # only in this scenario's Gherkin text, so its appearance in the child's
  # next reported turn cannot be anything but that specific agent_send
  # having reached that specific child session.
  # The opencode cell: a child delegated to an agent on a `type: opencode`
  # label runs through the opencode engine's own runner (session-home
  # delivery, Exec, the per-turn driver, opencode's JSON event codec) against
  # a SCRIPTED opencode named by the label's binary_path — no model, no real
  # CLI. The script answers with the guidance it finds in the AGENTS.md
  # ctxloom delivered to its config dir, so the child's own transcript proves
  # the composed context reached opencode; its record proves the protocol
  # argv, the relocated home and that the endpoint key rode the environment
  # alone.
  @opencode
  Scenario: A child delegated to an opencode label runs on opencode's runner and reports what its delivered context says
    Given Alice's coordinator can delegate to "qwen-worker", an agent on an opencode label served by a scripted opencode, carrying its own guidance
    And a session owner is standing with the opencode endpoint key exported
    When the agent calls tool "agent_run" with:
      | role         | qwen-worker |
      | input.prompt | go          |
    Then the tool call succeeds
    And "qwen-worker"'s session harp is remembered
    And "qwen-worker"'s reported turn carries the guidance opencode read from its delivered context
    And the scripted opencode ran the structured protocol in the session home, with the key only in its environment

  @reach-back @R1
  Scenario: A message the coordinator sends via agent_send reaches its child, verified in the child's own next reported turn
    Given Alice's coordinator can delegate to two agents, "librarian" and "cartographer", each carrying its own distinct guidance in its own profile
    And a session owner is standing
    When the agent calls tool "agent_run" with:
      | role         | librarian |
      | input.prompt | go        |
    Then the tool call succeeds
    And "librarian"'s session harp is remembered
    And "librarian"'s reported turn carries its own guidance, not "cartographer"'s
    When the agent calls tool "agent_send" addressed to "librarian"'s session with body "J002300-ROUNDTRIP-ECHO-TOKEN-6d2e73"
    Then the tool call succeeds
    And the tool result field "delivery" is set
    And "librarian"'s next reported turn carries "J002300-ROUNDTRIP-ECHO-TOKEN-6d2e73"
    And "librarian"'s spool was never claimed by a turn-start hook

  # LOCKED — the CHILD->coordinator half of requirement 4, hermetically, and
  # the end of the long poll (row worried-chief). A chat child never calls
  # agent_send itself, but its runner writes every turn's output to the
  # parent's spool as the automatic turn report (coord.EngineHost's
  # ReportTurnResult, spoolturnresult.go). The coordinator dispatches and then
  # does NOTHING: it never receives, and the harness types nothing more into
  # it. The report's arrival in the owner's in/ makes the owner's runner fire
  # its wake (Home.wakeOwner → the mock's own socket), the wake line starts a
  # turn, and that turn's turn-start hook (`ctxloom hook mail-drain`, the one
  # reader of the owner's in/) delivers the report and acknowledges it.
  # Proven on the owner's TERMINAL (the wake line was taken as a turn) and on
  # DISK (the report's id is in the owner's delivered record, and the report
  # carries the child's own guidance, not its sibling's). After the readiness sentinel, only a wake can start an
  # owner turn, so the two together are the woken turn's delivery.
  #
  # BREAK-POINT: this is the regression gate for an EMPTY COORDINATOR HARP.
  # The harp IS the coordinator's mailbox address, and a bare `ctxloom mcp`
  # coordinator has no ambient session to supply one, so with it empty the
  # child's turn report is refused (queueMailPayloadID's `to == ""` guard)
  # while agent_run keeps returning success. Revert selfIdentityFromEnv's
  # minted-harp fallback and this goes red for exactly that reason — nothing
  # reaches the owner's spool, so nothing wakes it.
  #
  # CONTAINER AXIS NOT EXERCISED HERE: a containerized engine is carried
  # ctxloom's hooks like a host one (pulmonary-eternity), so the same
  # turn-start delivery reaches a container-hosted coordinator. The
  # coordinator here is a host process, which is the axis this claim covers.
  @reach-back @R2
  Scenario: An idle coordinator is woken by its child's report, and its turn-start hook delivers it, with no receive
    Given Alice's coordinator can delegate to two agents, "librarian" and "cartographer", each carrying its own distinct guidance in its own profile
    And a session owner is standing
    When the agent calls tool "agent_run" with:
      | role         | librarian |
      | input.prompt | go        |
    Then the tool call succeeds
    And "librarian"'s session harp is remembered
    And the session owner is woken
    And the coordinator's own spool shows "librarian"'s report delivered, carrying its own guidance, not "cartographer"'s

  # THE NEGATIVE PROBE for the two hermetic bus scenarios above. Both are
  # green only because a REAL runner process stands for the child: the
  # coordinator self-execs one per delegated run, and it is that process — not
  # the coordinator — that opens the child's transcript and writes the
  # automatic turn report the coordinator's mailbox receives
  # (coord.EngineHost, spoolturnresult.go). A green run cannot show that
  # dependency on its own: a shim answering in-process for the runner would
  # produce the same bytes. So this scenario WITHHOLDS the runner and asserts
  # that the same agent_run, on the same fixture, then yields NEITHER
  # observable — the mailbox carries the launch failure with the withheld
  # runner's own dying words in the body and no result at all, and the child's
  # transcript never gains a turn.
  #
  # HOW THE RUNNER IS WITHHELD, with no product seam: the session owner (the
  # `ctxloom run` hosting the coordinator) is started from a copy of the
  # binary that is unlinked once the owner is standing — its OWN runner is
  # already up by then — so the coordinator's self-lookup (selfexec.Path) for
  # every CHILD runner takes its documented upgrade-in-place fallback — a PATH
  # lookup — and PATH is led by a decoy `ctxloom` that prints a marker and
  # exits non-zero. Everything else about the fixture is R1/R2's.
  #
  # FORCED, NOT AWAITED: the failure notice is queued by the run's terminal,
  # after which nothing can write that child's transcript, so "recorded no
  # turn" reads a settled state rather than racing one.
  @negative-probe
  Scenario: With the runner withheld, the same delegation yields a launch failure in the coordinator's mailbox and no recorded turn
    Given Alice's coordinator can delegate to two agents, "librarian" and "cartographer", each carrying its own distinct guidance in its own profile
    And the coordinator's runner is withheld
    When the agent calls tool "agent_run" with:
      | role         | librarian |
      | input.prompt | go        |
    Then the tool call succeeds
    And "librarian"'s session harp is remembered
    And the coordinator's own spool receives a message from "librarian"
    And the received message from "librarian" is the withheld runner's launch failure, and no result carrying its guidance arrived
    And "librarian" recorded no turn

  # THE PER-ENGINE FLOOR — one live row per engine ctxloom 0.7 can delegate to.
  #
  # WHY IT EXISTS. Every scenario above proves delegation against the mock
  # (hermetic). That does not answer the question an operator actually asks
  # before trusting `agent_run` on their own box: "does a delegated child on
  # MY engine really launch, really receive its composed context, and really
  # get a word back to its coordinator?" A per-engine row, in the suite's own
  # live lane, is the difference between "the code path exists" and "the
  # engine came back".
  #
  # WHAT EACH ROW PROVES, AND WHY IT CANNOT BE FAKED. The marker phrase exists
  # in exactly ONE place: a fragment in the child's OWN bundle, written fresh
  # by the gate step into this scenario's isolated project. It is NOT in the
  # prompt (read it — the prompt only says "the one distinctive marker phrase
  # in your context"), so an engine that echoes what it was sent cannot
  # produce it. It reaches the coordinator only if (a) the child process
  # really launched on that engine, (b) ctxloom really delivered the composed
  # profile context through the engine's context surface, (c) the engine really reasoned over
  # that context, and (d) the child really reached back through its forwarder
  # MCP server to call agent_send(to:"parent"). The assertion is on the BODY
  # BYTES that arrive in the coordinator's mailbox, never on an exit code and
  # never on agent_run's own success — which this journey's own history
  # already showed is worth nothing on its own (agent_run kept returning
  # success for weeks while the empty-coordinator-harp defect silently ate
  # every reply). A child that launches and dies still delivers a message —
  # the run's terminal notice reaches the same mailbox — so
  # "a message arrived from the child" is deliberately NOT the assertion; the
  # marker in the body is.
  #
  # GATING. Each row probes ITS OWN engine through the same
  # live_engine_registry.go decision every other @live step uses, and a row
  # whose engine is missing or unauthenticated skips with the engine and the
  # reason printed by name — never silently. CTXLOOM_LIVE_REQUIRE turns any
  # named engine's skip into a hard failure (checkRequiredEngines), which is
  # how a credential expiry is stopped from quietly deleting a row.
  #
  # ONE ROW AT A TIME. Each Examples block carries its own @<engine> tag —
  # the addressing mechanism isolation_probe.feature established, and used
  # here for the same reason: `just live-delegation <engine>` runs exactly one
  # engine's row, which is what a live, paid, minutes-long turn wants. Tags
  # attach to an Examples: block, not to a row inside one, so each engine gets
  # its own single-row block.
  @live @delegation
  Scenario Outline: A delegated child on a real <engine> reports back a marker only its own composed context could supply
    Given a real "<engine>" engine is available for a delegated child carrying marker "<marker>"
    And a session owner is standing
    When the agent calls tool "agent_run" with:
      | role         | delegate |
      | input.prompt | Look at the additional context available to you in this session (not this message) for the one distinctive marker phrase it contains. Call the MCP tool agent_send with to="parent" and body set to EXACTLY that marker phrase, verbatim and in full, nothing else. Do this now. |
    Then the tool call succeeds
    And "delegate"'s session harp is remembered
    And the coordinator's own spool receives a body containing "<marker>" from "delegate"

    @claude-code
    Examples:
      | engine      | marker                                       |
      | claude-code | J002300-DELEGATE-MARKER-CLAUDE-CODE-1d4c07ab |

    @opencode
    Examples:
      | engine   | marker                                    |
      | opencode | J002300-DELEGATE-MARKER-OPENCODE-6e1b3f52 |

  # THE AGENT_SEND CELL — the floor above, held to the stricter channel.
  #
  # WHY THE FLOOR ALONE DOES NOT PROVE agent_send. Two paths carry a child's
  # words to its coordinator's spool, both as messages from the child's harp:
  # the child's own agent_send, and its runner's automatic turn report, which
  # carries whatever the model SAID (runner.Home.ReportTurnResult). A model
  # that answers "the marker is X" in text, calling no tool, lands X in the
  # mailbox through the report, and the floor's assertion — the marker bytes
  # in any body from the child — passes. The floor is left exactly as it is:
  # it is the bus proof, and it is meant to hold whichever path carried the
  # words.
  #
  # WHAT TELLS THE PATHS APART, with no product change. The runner marks every
  # automatic report in its structured companion (coord.AutoReportStructured,
  # the `auto_report` key), and a child that sent its parent anything during a
  # turn gets no automatic report for that turn at all. The kind does NOT tell
  # them apart: a report is result-kind, and an agent_send caller picks its own
  # kind. So this cell passes ONLY on an UNMARKED body from the child carrying
  # the marker — a message the child chose to send — and fails at once when
  # the marker arrives in a marked report instead (j002300ScanSends).
  #
  # ACCEPTED COST (owner ruling 2026-10-12): one more paid turn per live lane,
  # and this cell can flake when the model answers in text instead of calling
  # the tool. That flake is exactly what it measures, and it is isolated from
  # the bus proof above, which stays green either way.
  #
  # Address it alone with
  #   ACCEPTANCE_TAGS="@live && @delegation-agent-send && @claude-code"
  # `just live-delegation <engine>` runs it beside the floor.
  @live @delegation @delegation-agent-send
  Scenario Outline: A delegated child on a real <engine> sends its coordinator, by agent_send, a marker only its own composed context could supply
    Given a real "<engine>" engine is available for a delegated child carrying marker "<marker>"
    And a session owner is standing
    When the agent calls tool "agent_run" with:
      | role         | delegate |
      | input.prompt | Look at the additional context available to you in this session (not this message) for the one distinctive marker phrase it contains. Call the MCP tool agent_send with to="parent" and body set to EXACTLY that marker phrase, verbatim and in full, nothing else. Do this now. |
    Then the tool call succeeds
    And "delegate"'s session harp is remembered
    And the coordinator's own spool receives "<marker>" in an agent_send body from "delegate", not in its automatic turn report

    @claude-code
    Examples:
      | engine      | marker                                   |
      | claude-code | J002300-SEND-MARKER-CLAUDE-CODE-7f3a90c2 |

    # OPENCODE, on its free hosted model (liveOpencodeModel), keyless.
    # Measured 2026-10-13 on opencode 1.18.35, throwaway HOME/XDG/repo:
    # `ACCEPTANCE_TAGS="@live && @delegation-agent-send && @opencode"`,
    # 1 scenario / 6 steps green in 15.3s on the first attempt. The marker
    # came in an unmarked body, so the child chose agent_send. No built-in
    # tool competes the way claude's SendMessage did: `opencode debug agent
    # build` lists bash, edit, glob, grep, invalid, question, read, skill,
    # task, todowrite, webfetch, websearch and write, and none of them sends
    # a message to a peer. So an opencode child needs no deny list.
    @opencode
    Examples:
      | engine   | marker                                |
      | opencode | J002300-SEND-MARKER-OPENCODE-2c9d5e17 |

  # P6 — THE STEER ECHO. Capability-probe ladder rung p6-steer-echo
  # (tests/acceptance/capability_probe_registry.go), living here rather than in a
  # file of its own because it extends this journey's machinery rather than
  # duplicating it: the gate, the harp-remembering step and the owner-spool
  # reader above are all reused verbatim, and the LOCKED scenarios are left
  # exactly as they are.
  #
  # WHAT IT ADDS TO THE FLOOR ABOVE. The per-engine floor proves the
  # CHILD -> COORDINATOR direction: a marker that exists only in the child's
  # own composed context reaches the coordinator's mailbox. This outline
  # proves the other direction on a real engine — the coordinator reaching
  # INTO a live session mid-flight, and the child acting on what it was
  # handed.
  #
  # THE CHANNEL IS THE BUS MESSAGE BODY, AND ONLY THAT. The value the child must
  # produce is a harp minted per cell at fixture time (probeHarps.Mint) and
  # handed to it in ONE place: the body of an agent_send made AFTER it is
  # already running. It is not in the composed context, not in the spawn prompt,
  # not in the environment. So a child that returns it has necessarily received
  # a mid-session message — which is the whole claim — and no amount of prompt
  # compliance can fake it.
  #
  # WHY EACH ROW STILL PLANTS A CONTEXT MARKER. The wake-up half (the marker
  # column, the same technique the floor above uses) is not redundant: it proves
  # the child launched, received its context and can reach its coordinator
  # BEFORE a steer is spent, so a red on the echo cannot be confused with a
  # child that never woke up. The two values are deliberately different, and the
  # verdict function's own unit tests include the false-green where a child
  # repeats its context marker instead of the steer (probe_p6_steer_echo_test.go,
  # TestP6AssertEcho_ForeignChannelCannotFalseGreenIt).
  #
  # THE SPOOL ASSERTION IS THE MAIL PLANE'S BEHAVIOURAL PROOF IN THIS SUITE.
  # The file spool is the only carrier, so the final step asserts, on PAYLOAD
  # BYTES, that the coordinator's steer exists as a FILE in the child's own
  # spool in plane. "The echo came back" is not enough on its own: a carrier
  # that delivered by some path nobody meant, while writing nothing, is this
  # project's characteristic bug wearing the spool's clothes.
  #
  # MEASURED 2026-08-13, EVERY ROW GREEN. The
  # claude-code row re-proves what the LOCKED scenario above already proves,
  # against a coordinator->child mid-session steer that had been claimed and
  # never demonstrated (capability inventory row 13 read "marker only; no
  # mid-session steer"). Each child returned the minted harp as
  # its whole message body. Each row's assertion was then mutated on the
  # ASSERTION SIDE ONLY — the verdict made to look for harp+"-MUTANT" while the
  # fixture still sent the real one — and each went RED with a BUS-DELIVERY
  # shape, so no row is passing because the check cannot fail.
  #
  # The OUT plane carries the child's traffic too, harp and all. That is
  # recorded in the census rather than asserted on purpose: P6's claim is the
  # coordinator's steer, and a cell must not silently become the regression
  # gate for a neighbouring subsystem's scope. If child->parent file delivery
  # is meant to be guaranteed, that belongs in its own assertion.
  #
  # TIMING: an echo turn has landed as much as 79 seconds after the steer on a
  # slow engine. The spool waits run to the suite deadline, so a live run's
  # test timeout must leave room for that; do not size it on a fast engine.
  #
  # ONE ROW AT A TIME, two paid turns each (the wake-up and the steer). Address
  # exactly one cell with the registry's own tag expression:
  #   ACCEPTANCE_TAGS="@live && @probe-p6-steer-echo && @claude-code && @host && @ws-none"
  # (probeCell.TagExpression renders it; the `just capability-probe` wrapper is
  # slice S10's, not this one's).
  @live @probe-p6-steer-echo
  Scenario Outline: A delegated child on a real <engine> echoes back a harp the coordinator steered into its live session
    Given a real "<engine>" engine on runtime "<runtime>" with workspace "<workspace>" is available for a steer-echo child carrying wake marker "<marker>"
    And a session owner is standing
    When the agent calls tool "agent_run" with:
      | role         | delegate |
      | input.prompt | Look at the additional context available to you in this session (not this message) for the one distinctive marker phrase it contains. Call the MCP tool agent_send with to="parent" and body set to EXACTLY that marker phrase, verbatim and in full, nothing else. Do this now. |
    Then the tool call succeeds
    And "delegate"'s session harp is remembered
    And the coordinator's own spool receives a body containing "<marker>" from "delegate"
    When the agent calls tool "agent_send" addressed to "delegate"'s session carrying this cell's minted steer harp
    Then the tool call succeeds
    And the coordinator's own spool receives "delegate"'s echo of this cell's minted steer harp
    And the coordinator's steer is on disk in "delegate"'s own spool, in a file carrying that harp

    @claude-code @host @ws-none
    Examples:
      | engine      | runtime | workspace | marker                                |
      | claude-code | host    | none      | P6-WAKE-MARKER-CLAUDE-CODE-5b1e07c4   |

    # THE ISOLATED CELL. Every row above runs host/none — isolated on NEITHER
    # axis — so they prove the round trip works, not that it survives the
    # isolation boundary. This row is the same steps with the child's
    # PROCESS in a rootless container and its FILES in a worktree: the two axes
    # are independent (see j002200_isolation.feature's header), and answering
    # "does delegation still work when isolated" needs both set at once, not
    # either alone.
    #
    # It is NOT a relabelling of the rows above. steps_p6_steer_echo.go refuses
    # any pair p6BuildableCells does not declare, so this row runs a genuinely
    # different fixture — `runtime: container-rootless` on the agent binding,
    # `workspace: worktree` at the top level, and the fixture COMMITTED, because
    # a worktree spawn refuses a dirty checkout.
    #
    # container-rootful is absent rather than declared-and-skipped: no box this
    # suite has run on has had a reachable rootful daemon, and a row that can
    # only ever skip looks like coverage.
    @claude-code @container-rootless @ws-worktree @reach-back @R4
    Examples:
      | engine      | runtime            | workspace | marker                                    |
      | claude-code | container-rootless | worktree  | P6-WAKE-MARKER-CLAUDE-CODE-CTRWT-9d4f21ab |

    # THE TWO MIXED CORNERS, completing the matrix. host/none and
    # container-rootless/worktree prove the bus with BOTH boundaries off and
    # BOTH on; these prove it with exactly one present. The axes are
    # independent, so a bus that survives the combination could still fail on a
    # single boundary — worktree alone relocates the child's files and its
    # config home, container alone relocates its process and its credentials,
    # and each could break the mail plane on its own.
    @claude-code @host @ws-worktree
    Examples:
      | engine      | runtime | workspace | marker                                   |
      | claude-code | host    | worktree  | P6-WAKE-MARKER-CLAUDE-CODE-HOSTWT-3e7c15 |

    @claude-code @container-rootless @ws-none @reach-back @R3
    Examples:
      | engine      | runtime            | workspace | marker                                   |
      | claude-code | container-rootless | none      | P6-WAKE-MARKER-CLAUDE-CODE-CTRNONE-b82a4 |

    # OPENCODE, on a free hosted model (liveOpencodeModel): the host rows are
    # live-verified.
    @opencode @host @ws-none
    Examples:
      | engine   | runtime | workspace | marker                              |
      | opencode | host    | none      | P6-WAKE-MARKER-OPENCODE-7c2e91d4    |

    @opencode @host @ws-worktree
    Examples:
      | engine   | runtime | workspace | marker                                 |
      | opencode | host    | worktree  | P6-WAKE-MARKER-OPENCODE-HOSTWT-4a8f03 |

    # opencode.Opencode.Container installs the release (installFragment). Both
    # rootless cells are LIVE-VERIFIED (2026-10-13; the evidence is in
    # capability_probe_registry.go).
    # On the live-container lane they skip loudly: its image ships no opencode
    # binary on the harness side (engineAvailable).
    @opencode @container-rootless @ws-worktree @reach-back @R4
    Examples:
      | engine   | runtime            | workspace | marker                                 |
      | opencode | container-rootless | worktree  | P6-WAKE-MARKER-OPENCODE-CTRWT-5b9e27c1 |

    @opencode @container-rootless @ws-none @reach-back @R3
    Examples:
      | engine   | runtime            | workspace | marker                                  |
      | opencode | container-rootless | none      | P6-WAKE-MARKER-OPENCODE-CTRNONE-e40d6a |

  # Back to: tests/acceptance/features/journeys/j002100_delegation.feature (the privilege
  # half of delegation this journey complements).

  # MID-TURN MAIL — alpha issue 1, live. A parent's binding reply once sat
  # unread through a delegated child's whole turn, because a child's mail was
  # only ever handed to it as its NEXT turn. The post_tool mail-peek hook
  # (`ctxloom hook mail-peek`) hands the child mail waiting behind the turn in
  # flight at its next tool call, asking the child's runner — the spool's one
  # reader — over the session endpoint's /mail.
  #
  # The child is given ONE long, multi-tool turn (a dozen sequential sleeps)
  # and announces it has started; only then does the parent send a code. The
  # child acknowledges the code by agent_send. Mail reaches the model only as
  # a turn's prompt or as hook context inside a turn, so the acknowledgement's
  # tool call appearing in the child's canonical transcript BEFORE its first
  # turn's `complete` record proves the code was handed over mid-turn, at a
  # tool call (steps_midturn_mail.go).
  #
  # ONE ROW AT A TIME, one paid haiku turn each (liveClaudeModel):
  #   ACCEPTANCE_TAGS="@live && @midturn-mail && @claude-code && @host && @ws-none"
  @live @midturn-mail
  Scenario Outline: A delegated child on a real <engine> in one long multi-tool turn acts on its parent's message before that turn ends
    Given a real "<engine>" engine on runtime "<runtime>" with workspace "<workspace>" is available for a mid-turn-mail child
    And a session owner is standing
    When the agent calls tool "agent_run" with:
      | role         | delegate |
      | input.prompt | This is ONE long task; do all of it in this single turn and do not end your turn early. Step 1: call the MCP tool agent_send to your parent (to_role "parent", kind MESSAGE_KIND_MESSAGE) with text exactly MIDTURN-STARTED. Step 2: run the shell command `sleep 5` with your Bash tool, ONE call at a time, 12 times in a row: never in parallel and never combined into one command. While you work, a message from your parent will appear in your context carrying a code that starts with MIDTURN-CODE-. As soon as you see it, stop the sleeps, call agent_send to your parent with text exactly MIDTURN-ACK followed by a space and that full code, and then end your turn. |
    Then the tool call succeeds
    And "delegate"'s session harp is remembered
    And the coordinator's own spool receives a body containing "MIDTURN-STARTED" from "delegate"
    When the agent calls tool "agent_send" addressed to "delegate"'s session carrying this cell's mid-turn code
    Then the tool call succeeds
    And the coordinator's own spool receives "delegate"'s acknowledgement of this cell's mid-turn code
    And "delegate" acknowledged this cell's mid-turn code inside the turn it was already running

    @claude-code @host @ws-none
    Examples:
      | engine      | runtime | workspace |
      | claude-code | host    | none      |

    @claude-code @container-rootless @ws-none
    Examples:
      | engine      | runtime            | workspace |
      | claude-code | container-rootless | none      |
