# Ported from probes/capability_native_history.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   capability_native_history.feature with its steps (gherkin_lint.test.rs).

@live @probe-p14-native-history
@pending
Feature: P14 — claude keeps its conversation history under the config home's projects/, and writes through a symlinked one

  ctxloom keeps an agent's native claude history in sessions/<harp>/native/claude/
  and reaches it through a RELATIVE symlink at the config home's projects/, so the
  disposable config home can be deleted on Close without deleting the history,
  and the same link resolves in a container that mounts native/. That holds only
  while claude (a) writes its conversation .jsonl under $CLAUDE_CONFIG_DIR/projects
  when it runs in ctxloom's container, and (b) writes THROUGH a symlinked projects/
  and never replaces it with a real directory. probe_p14_native_history.go holds
  the judges; re-run on every claude pin bump:

    just capability-probe p14-native-history probes/capability_native_history.feature claude-code <runtime> none

  One paid haiku turn per cell. Each self-skips LOUDLY when claude, its
  credential, or (for the container cell) a rootless runtime is absent.

  Background:
    Given Alice has a git-backed project

  # (a) Through ctxloom, on the isolation probe's own run: the session's engine
  # home is bind-mounted into the container, so what claude wrote there is
  # observed on the host while the run is in flight.
  Scenario Outline: claude in a <runtime> container writes its conversation history under its config home's projects/
    Given the isolation probe targets "<engine>" under the "<runtime>" axis
    When the probe runs it live, writing a unique token in one turn
    Then claude's conversation history landed under its config home's projects directory

    @claude-code @container-rootless @ws-none @var-container-writes
    Examples:
      | engine      | runtime            | workspace |
      | claude-code | container-rootless | none      |

  # (b) The vendor binary directly, in a throwaway HOME and CLAUDE_CONFIG_DIR
  # whose projects/ is the layout's own relative link.
  Scenario Outline: claude writes through a symlinked projects/ and leaves the link in place
    Given the native-history probe targets "<engine>" under runtime "<runtime>" and workspace "<workspace>" with projects symlinked
    When it runs one native-history turn
    Then claude wrote its history through the symlink and left the link in place

    @claude-code @host @ws-none @var-symlinked-projects
    Examples:
      | engine      | runtime | workspace |
      | claude-code | host    | none      |
