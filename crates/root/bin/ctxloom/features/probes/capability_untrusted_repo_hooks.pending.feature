# Ported from probes/capability_untrusted_repo_hooks.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   capability_untrusted_repo_hooks.feature with its steps (gherkin_lint.test.rs).

@live @probe-p13-untrusted-repo-hooks
@pending
Feature: P13 — claude -p runs an untrusted repo's committed hooks, and --setting-sources user suppresses them

  A repository can commit a .claude/settings.json whose hooks run arbitrary
  commands. claude's own trust does not stop them under `-p`: a repo with no
  projects entry in CLAUDE_CONFIG_DIR/.claude.json — one nobody ever trusted —
  still has its PreToolUse and SessionStart hooks executed. ctxloom's repo
  trust therefore cannot lean on claude's trust flag; it launches an untrusted
  repo's child with `--setting-sources user --strict-mcp-config`, which is what
  keeps the repo's settings, and the hooks in them, from loading. The repo also
  commits a skill and an agent whose frontmatter declares hooks and an MCP
  server: the same flags keep them from loading at all, and a trusted-repo
  control shows that frontmatter really executes when they do load. All of it
  is vendor behaviour, re-run on every claude pin bump:

    just capability-probe p13-untrusted-repo-hooks probes/capability_untrusted_repo_hooks.feature claude-code host none

  HOW A CELL WORKS. The vendor binary is run directly — not through ctxloom, so
  a red names claude and nothing else — in a throwaway CLAUDE_CONFIG_DIR (no
  .claude.json, so the repo is untrusted) and HOME, with cwd a git repo whose
  COMMITTED .claude/settings.json registers a PreToolUse hook on Bash and a
  SessionStart hook, each touching a marker outside the repo, and whose
  committed .claude/skills and .claude/agents carry a skill with a PreToolUse
  hook and an agent with a Stop hook and an inline stdio MCP server, each
  touching a marker of its own. A flag-scope --settings allows `echo hi`; the
  prompt invokes the skill, runs echo hi, then runs the agent in the
  foreground. Every assertion reads stream-json frames (the init frame's
  skills and agents, the echo's tool_result) and the marker files — never the
  model's prose.

  Each cell self-skips LOUDLY when claude is absent, and when no token was
  captured at launch: a throwaway config dir cannot use the subscription login.
  Four paid haiku turns for the set.
  Scenario Outline: A repo's committed hooks, skill and agent under the <variant> posture
    Given the untrusted-repo-hooks probe targets "<engine>" under runtime "<runtime>" and workspace "<workspace>" in the "<variant>" posture
    When it asks the engine to run echo hi in one turn
    Then the repo's committed hooks behave as the "<variant>" posture requires

    # UNTRUSTED-FIRES exists because it is the premise: if claude's trust ever
    # stops repo hooks under -p, ctxloom's suppression is no longer the only
    # line and the design should be re-examined. Asserts: both settings markers
    # written, the init frame lists the repo's skill and agent, and the echo's
    # tool_result printed "hi".
    @claude-code @host @ws-none @var-untrusted-fires
    Examples:
      | engine      | runtime | workspace | variant         |
      | claude-code | host    | none      | untrusted-fires |

    # SETTING-SOURCES-SUPPRESSES exists because it is the defence: if these
    # flags stop keeping project settings out, an untrusted repo's hooks run in
    # ctxloom's children. Asserts: NO marker written (settings or frontmatter),
    # the init frame lists neither the repo's skill nor its agent, while the
    # echo still ran and printed "hi" — suppressed, not untriggered.
    @claude-code @host @ws-none @var-setting-sources-suppresses
    Examples:
      | engine      | runtime | workspace | variant                    |
      | claude-code | host    | none      | setting-sources-suppresses |

    # TRUSTED-FRONTMATTER-FIRES exists because the suppressing arm's frontmatter
    # silence means nothing unless the fixture's frontmatter can run at all.
    # claude honours agent frontmatter only from a folder it trusts, so this
    # cell trusts the repo in its config dir. Asserts: the init frame lists the
    # skill and agent, and the agent's Stop hook and MCP server both wrote
    # their markers.
    @claude-code @host @ws-none @var-trusted-frontmatter-fires
    Examples:
      | engine      | runtime | workspace | variant                   |
      | claude-code | host    | none      | trusted-frontmatter-fires |

    # CTXLOOM-LAUNCH-UNTRUSTED exists because the other arms run claude by
    # hand: this one launches the repo the way ctxloom does — claude's verdict
    # read from a human home that never trusted it, the session home claude's
    # instance-config writer generates for that verdict, and the argv and
    # stdin prompt the engine's Exec composes. Asserts what the suppressing
    # arm asserts: NO marker, neither the skill nor the agent listed, the echo
    # ran and printed "hi".
    @claude-code @host @ws-none @var-ctxloom-launch-untrusted
    Examples:
      | engine      | runtime | workspace | variant                  |
      | claude-code | host    | none      | ctxloom-launch-untrusted |
