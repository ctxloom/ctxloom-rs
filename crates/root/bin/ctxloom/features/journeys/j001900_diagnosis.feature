# Ported from journeys/j001900_diagnosis.feature (rev f36e8dfb2)
# NOT ported: the engine-ingestion inspector; the walk ends with her assistant answering.
@doc
@pending
Feature: The day the assistant goes blind

  Monday morning. "The assistant knew our deploy process on Friday. It doesn't
  today." And the worst part of the report is the second half: it is still
  reaching Bob's assistant and not Alice's. Nothing errored. Nothing is red.
  The guidance simply stopped arriving, and Alice has to find out where.

  Content travels a long way to her assistant: written, packaged into a
  bundle, pulled, composed into a profile, and handed to her session. Alice
  walks it back from the symptom. At every step a ctxloom command tells her,
  out loud, whether the guidance made it that far; she fixes what it names
  and asks again. Each scenario is one Monday with the guidance lost at one
  step, and each ends the same way: her next session knows the deploy process
  again.

  Materializing is not on that path: a session ctxloom starts never reads a
  materialized file. `ctxloom materialize` is an export, for a teammate who
  does not use ctxloom: it writes the team's context out as the engine's own
  files, a snapshot that goes stale as soon as the team's context moves on.

  Background:
    Given Alice's team ships its deploy process as ctxloom content

  # Where it starts: Bob's assistant has it and hers does not. Each of them
  # has ctxloom show what their next session would be handed, and Alice
  # compares the two.
  Scenario: His assistant has the guidance and hers does not, and their dry runs side by side show it
    # new: told as the user's flow.
    Given Bob's checkout of the same project delivers the deploy guidance and Alice's does not
    When Bob shows what his next session would be handed, and sends it to Alice:
      """
      ctxloom run --dry-run > bob.txt
      """
    And Alice shows what her next session would be handed, and compares it with Bob's:
      """
      ctxloom run --dry-run > alice.txt
      diff bob.txt alice.txt
      """
    Then the comparison shows the deploy guidance present for Bob and absent for her

  # Her profile does not compose the runbook. She asks which profile her agent
  # runs and what it composes, adds the runbook, and asks her assistant.
  Scenario: The runbook is in no profile; she adds it, and her next session knows the deploy process
    # new: she fixes what profile show names.
    Given the runbook is pulled, but composed into no profile of Alice's
    When Alice asks which profile her agent runs, and what that profile composes:
      """
      ctxloom agent show default
      ctxloom profile show default
      """
    Then she sees that her profile does not compose the runbook
    When Alice adds the runbook to her profile and asks her assistant how to deploy:
      """
      ctxloom profile edit default --add-bundle team/runbook
      ctxloom run --one-shot "how do we deploy?"
      """
    Then her assistant answers with the team's deploy process

  # Someone held the runbook at an older version and forgot. The listing says
  # it is held, so a deliberate freeze does not look like a broken pull; she
  # releases the hold and moves the runbook on, which brings the new guidance.
  Scenario: The runbook is held at an older version; she moves it on, and her next session has the new guidance
    # new: she releases the hold the listing names and upgrades.
    Given Carol published a newer runbook while Alice's copy is held at the older one
    When Alice lists what her project depends on:
      """
      ctxloom deps list
      """
    Then she sees the runbook named as held at the older version
    When Alice releases the hold, moves the runbook to Carol's newer version, and asks her assistant how to deploy:
      """
      ctxloom deps hold team/runbook --undo
      ctxloom deps upgrade team/runbook --yes
      ctxloom run --one-shot "how do we deploy?"
      """
    Then her assistant answers with the newer deploy process, not the older

  # The deploy process was written down, but never packaged. The search says
  # it found nothing; she packages it and puts it in her profile.
  Scenario: Nothing packaged the deploy process; she packages it, and her next session knows it
    # new: she packages what the search could not find.
    Given the deploy process exists only as loose notes in Alice's repo, in no bundle
    When Alice searches for it:
      """
      ctxloom search deploy
      """
    Then she sees that nothing packaged carries the deploy process
    When Alice writes the deploy notes up as a fragment, adds it to her profile, and asks her assistant how to deploy:
      """
      ctxloom fragment create runbook#fragments/deploy
      ctxloom profile edit default --add-bundle runbook
      ctxloom run --one-shot "how do we deploy?"
      """
    Then her assistant answers with the deploy process she packaged

  # Dave does not use ctxloom. Last month Alice exported the team's context
  # for him, and the deploy process has moved on since. His copy went stale,
  # as an export does; ctxloom names it, and she exports it again.
  Scenario: The export she made for a teammate without ctxloom went stale; ctxloom names it, and she exports it again
    # new: a materialized export is a snapshot that goes stale.
    Given Alice exported the team's context into the project for Dave last month
    And the team's deploy process has changed since
    When Alice checks what ctxloom has written into the project:
      """
      ctxloom manage check
      """
    Then she sees the exported context named as stale
    When Alice exports the team's context for Dave again:
      """
      ctxloom materialize --yes
      """
    Then Dave's assistant, opened on the project without ctxloom, has the current deploy process
    And Alice's own sessions are handed the team's context as before, not the exported files
