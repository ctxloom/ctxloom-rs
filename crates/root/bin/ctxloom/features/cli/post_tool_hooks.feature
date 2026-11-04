# Go ctxloom's tests/acceptance/features/cli/post_tool_hooks.feature at f36e8dfb21 (release/0.7), kept
# as written except for the scenarios left out below.
# Left out:
#   - nothing; the "session index records harp" step is a no-op here (plan 8.4: `--engine` names the codec).


Feature: The PostToolUse callback — a nudge after the tool call that earns one, and silence after every other

  A hidden command rides the host engine's PostToolUse hook, invoked after a
  tool call rather than by a person: `hook skill-mates` names the linked
  skills a just-completed skill leaves uninvoked this session.

  It shares session_hooks.feature's contract: a hook that fails interrupts
  the tool call it rode on, so it always exits 0 and always leaves one JSON
  object on stdout. Exit status therefore proves nothing, and the assertions
  below read the envelope the engine would parse — stdout alone — for WHAT it
  carries. Silence is asserted as a well-formed, empty envelope, which is what
  distinguishes "correctly said nothing" from "emitted nothing at all".

  Background:
    Given an initialized ctxloom project

  Rule: skill-mates names a completed skill's uninvoked link-group mates, and nothing else

    # THE EFFECT END TO END: the delivered skill set is resolved the way the
    # session's own assembly resolved it (the default agent's profile over
    # this project's bundles), the link group comes from the skills'
    # `ctxloom:link_id` tags, and "not yet invoked" is read off the session's
    # own transcript — which here holds no Skill call, so the mate is named.
    Scenario: Completing one skill of a linked pair names the other
      Given the project already has the file ".ctxloom/content/bundles/v2/nightly/skills/admit/SKILL.md":
        """
        ---
        name: admit
        description: Admit work to the nightly queue.
        ---

        Admit the queue.
        """
      And the project already has the file ".ctxloom/content/bundles/v2/nightly/skills/unattended/SKILL.md":
        """
        ---
        name: unattended
        description: Work the admitted queue.
        ---

        Work the queue.
        """
      And the project already has the bundle "nightly":
        """
        version: "1.0"
        skills:
          admit:
            tags: [ctxloom:link_id=nightly]
          unattended:
            tags: [ctxloom:link_id=nightly]
        """
      And a profile "ops" with bundle "nightly"
      And I run "ctxloom agent create default --llm claude-code --profiles ops"
      And I run "ctxloom agent default default"
      And the session harp is "brisk-copper-moth"
      And the session index records harp "brisk-copper-moth" on engine "claude-code"
      And a "claude-code" transcript at "turn.jsonl" whose turn ends with:
        """
        The queue is admitted.
        """
      When I run "ctxloom hook skill-mates --engine claude-code" with input:
        """
        {"session_id":"s","hook_event_name":"PostToolUse","transcript_path":"$PROJECT_DIR/turn.jsonl","cwd":"/repo","tool_name":"Skill","tool_input":{"skill":"admit","args":""},"tool_response":"ok"}
        """
      Then the command succeeds
      And the hook's additionalContext contains "Skill admit completed; linked skills not yet invoked this session: unattended"

    # Every other tool is answered before any session, transcript or config
    # is touched — and answered with silence, not with an error.
    Scenario: A tool call that is not a skill is passed over in silence
      When I run "ctxloom hook skill-mates --engine claude-code" with input:
        """
        {"session_id":"s","hook_event_name":"PostToolUse","tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":"ok"}
        """
      Then the command succeeds
      And the hook's additionalContext is empty
