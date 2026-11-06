# Ported from probes/capability_turn_posture.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   capability_turn_posture.feature with its steps (gherkin_lint.test.rs).

@live @probe-p18-turn-posture
@pending
Feature: P18 — a resumed claude -p turn starts in, and behaves as, the mode its inline --settings names

  A headless child's posture is carried per turn, never by the session: each
  turn's inline JSON --settings names its mode as defaultMode, because claude
  does not carry a mode across --resume. A plan approval and an allowed mode
  change both move the run's posture between turns, and both rest on claude
  starting the RESUMED turn in the mode that turn's settings name.

    just capability-probe p18-turn-posture probes/capability_turn_posture.feature claude-code host none <variant>

  HOW A CELL WORKS. The vendor binary is run directly in a throwaway
  CLAUDE_CONFIG_DIR and HOME, with a fresh git repo as cwd and
  --setting-sources user. The config home registers one PermissionRequest
  hook that captures every ask. Each cell is two turns of one session, the
  second under --resume. Green means each turn's init frame reports the mode
  its settings named and the files and asks agree with that mode.

  Self-skips LOUDLY when claude is absent or no credential is exported.
  Two paid haiku turns per cell.
  Scenario Outline: A <engine> session's posture moves between turns: <variant>
    Given the turn-posture probe targets "<engine>" under runtime "<runtime>" and workspace "<workspace>" for variant "<variant>"
    When it runs two turns of one session, the second resumed under its own settings
    Then each turn starts in, and behaves as, the mode its settings named

    # PLAN-RESUMED (conformance P1): a plan-first child's every turn names
    # defaultMode plan, including turns that resume. acceptEdits then plan:
    # the first turn's write landing is the control, the plan turn's ordered
    # write must not land.
    @claude-code @host @ws-none @var-plan-resumed
    Examples:
      | engine      | runtime | workspace | variant      |
      | claude-code | host    | none      | plan-resumed |

    # PLAN-APPROVED (conformance D2): plan, then the approved posture. The
    # plan turn writes claude's native plan and not the file; the resumed
    # acceptEdits turn, told only that its plan was approved, creates it.
    @claude-code @host @ws-none @var-plan-approved
    Examples:
      | engine      | runtime | workspace | variant       |
      | claude-code | host    | none      | plan-approved |

    # SETMODE-HELD (conformance P2): default, where the hook answers the
    # write's ask with production's allow + session setMode acceptEdits; then
    # acceptEdits under --resume, where the next write lands with no ask.
    @claude-code @host @ws-none @var-setmode-held
    Examples:
      | engine      | runtime | workspace | variant      |
      | claude-code | host    | none      | setmode-held |
