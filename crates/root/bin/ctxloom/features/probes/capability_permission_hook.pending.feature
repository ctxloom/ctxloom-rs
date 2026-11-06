# Ported from probes/capability_permission_hook.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   capability_permission_hook.feature with its steps (gherkin_lint.test.rs).

@live @probe-p12-permission-hook-no-host
@pending
Feature: P12 — with no permission host, claude -p awaits the PermissionRequest hook and honours it

  ctxloom routes a headless claude child's approvals through claude's
  PermissionRequest hook and passes NO --permission-prompt-tool. That design is
  only sound while claude, in `-p` with no permission host, still (a) runs the
  hook when a tool call needs permission, (b) WAITS for its answer, and (c) does
  what the answer says. If a release stops doing any of the three, every gated
  call a child makes is decided by something other than the human — silently.

  The documented contract is https://code.claude.com/docs/en/hooks#permissionrequest.
  It is prose nothing checks, and it has said the opposite before. These cells
  are the checked copy, re-run on every claude pin bump:

    just capability-probe p12-permission-hook-no-host probes/capability_permission_hook.feature claude-code host none

  HOW A CELL WORKS. The vendor binary is run directly — not through ctxloom, so
  a red names claude and nothing else — in a throwaway CLAUDE_CONFIG_DIR and
  HOME, with a fresh git repo as cwd and a --settings file outside it that
  registers one PermissionRequest hook on Bash. The hook captures its input,
  sleeps, writes a marker, and prints its decision. The prompt asks for a
  `touch` outside the working directory, which is what makes claude ask.
  Every assertion reads stream-json frames, the hook's captured input, the
  marker and the proof file — never the model's prose.

  Each cell self-skips LOUDLY when claude is absent, and when no credential is
  EXPORTED: a throwaway config dir cannot use the subscription login.
  One paid haiku turn per cell.
  Scenario Outline: The <decision> answer from a <engine> PermissionRequest hook decides the gated call
    Given the permission-hook probe targets "<engine>" under runtime "<runtime>" and workspace "<workspace>" with the hook answering "<decision>"
    When it asks the engine to touch a file outside its working directory in one turn
    Then the gated call honours the hook's "<decision>" decision

    # ALLOW exists because a human's "allow" reaches a headless child ONLY as
    # this hook's answer: if claude stops waiting for it, or ignores it, every
    # approved call is still refused. Asserts: the file exists, the hook fired
    # on Bash, and the gated call's tool_result is stamped AFTER the hook's
    # marker — claude waited.
    @claude-code @host @ws-none @var-allow
    Examples:
      | engine      | runtime | workspace | decision |
      | claude-code | host    | none      | allow    |

    # DENY exists because a human's "deny" is the same answer with the other
    # behavior: if claude stops honouring it, a refused call runs anyway.
    # Asserts: the file is absent, the hook fired on Bash, the tool_result is
    # "Permission denied by hook" (is_error), and permission_denials names the call.
    @claude-code @host @ws-none @var-deny
    Examples:
      | engine      | runtime | workspace | decision |
      | claude-code | host    | none      | deny     |

    # SILENT exists because ctxloom's approval hook writes NO decision when it
    # cannot reach the runner, and the route's fail-closed guarantee is that
    # claude -p then refuses the call. Asserts: the hook fired on Bash, the
    # gated call's tool_result is an error, and the file is absent.
    @claude-code @host @ws-none @var-silent
    Examples:
      | engine      | runtime | workspace | decision |
      | claude-code | host    | none      | silent   |

    # ALLOW-PROMPTS-NONE pins what --permission-prompts none does to the hook:
    # claude documents none as answering prompts with a local deny, and the
    # measurement is that the hook is still consulted FIRST and its allow
    # honoured. Asserts the allow arm: awaited, the call ran, the file exists.
    @claude-code @host @ws-none @var-allow-prompts-none
    Examples:
      | engine      | runtime | workspace | decision           |
      | claude-code | host    | none      | allow-prompts-none |

    # ALLOW-TOOL-USE-ID-ABSENT pins the shape of the event itself: the route
    # correlates a PermissionRequest with its call through the PreToolUse that
    # precedes it, because PermissionRequest names no tool_use_id. Asserts the
    # allow arm, and that the hook's input has no tool_use_id key at all.
    @claude-code @host @ws-none @var-allow-tool-use-id-absent
    Examples:
      | engine      | runtime | workspace | decision                 |
      | claude-code | host    | none      | allow-tool-use-id-absent |
