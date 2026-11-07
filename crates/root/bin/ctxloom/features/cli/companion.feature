# Ported from cli/companion.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@doc
@pending
Feature: companion — which programs ctxloom runs alongside your session

  Covers: `ctxloom companion add`, `companion remove`, `companion list`, and
  the bare `ctxloom companion` form.

  A companion is a program that CONTRIBUTES context — hooks, MCP servers,
  fragments — by answering `<binary> loadout`. Reading what a companion
  contributes means RUNNING it, which is why this noun exists at all.

  ONLY WHAT YOU REGISTERED RUNS. `ctxloom companion add <name>` finds the
  companion on your PATH, checks that it answers the loadout probe, and records
  its NAME in your home config. From then on, every session resolves that name
  on PATH and runs it. Nothing else on PATH is ever run for being there: a
  dependency in `./node_modules/.bin` that ships a `ctxloom-companion-*`
  binary earns nothing by its name.

  A project's config may list more names under the same `companions` key:
  they are ADDED to the home registrations, and a project cannot remove one.

  A name maps to a binary: the shipped ltk, taskloom and reprise are their own
  binaries; any other name <n> is the binary `ctxloom-companion-<n>`.

  The registration is the name only, never a path, so it holds wherever the
  binary is installed. A binary of a registered name placed EARLIER on PATH is
  the one that runs.

  Deliberately NO MCP tools for any of this: handing the agent the ability to
  register the binaries that run alongside it defeats the property the
  registration exists to provide.

  This is the comprehensive per-noun spec: what the noun DOES, leaf by leaf.

  Rule: Only a registered companion is ever executed

    The assertion that matters is not the exit code and not the warning — it
    is WHICH BINARIES ACTUALLY RAN. The fake companion in these fixtures
    appends to a witness file every time it is invoked, so "was never
    executed" is read off the filesystem rather than inferred from a missing
    line of output.

    Scenario: A companion an npm dependency drops on PATH is never run
      Given an initialized ctxloom project
      And a dependency drops a companion "ctxloom-companion-evil" into an absolute node_modules/.bin on PATH
      When I run "ctxloom doctor"
      Then the companion "ctxloom-companion-evil" was never executed
      When I run "ctxloom companion list"
      Then the command succeeds
      And the output does not contain "evil"
      And the companion "ctxloom-companion-evil" was never executed

    Scenario: A registered companion runs, and removing it stops it
      Given an initialized ctxloom project
      And a companion "ctxloom-companion-acme" is on PATH, not registered
      When I run "ctxloom doctor"
      Then the companion "ctxloom-companion-acme" was never executed
      When I run "ctxloom companion add acme"
      Then the command succeeds
      And the home config registers the companion "acme" by name only
      Given the companion executions so far are forgotten
      When I run "ctxloom doctor"
      Then the companion "ctxloom-companion-acme" was executed
      When I run "ctxloom companion remove acme --yes"
      Then the command succeeds
      Given the companion executions so far are forgotten
      When I run "ctxloom doctor"
      Then the companion "ctxloom-companion-acme" was never executed

  Rule: a project may add companions, never remove home's

    Scenario: A project's companions are added to the home registrations
      Given an initialized ctxloom project
      And a companion "ctxloom-companion-acme" is on PATH, not registered
      When I run "ctxloom companion add acme"
      Given the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        companions: [beta]
        """
      When I run "ctxloom companion list --format json"
      Then the command succeeds
      And the output reports "0.name" as "acme"
      And the output reports "1.name" as "beta"

    Scenario: An empty project list does not unregister a home companion
      Given an initialized ctxloom project
      And a companion "ctxloom-companion-acme" is on PATH, not registered
      When I run "ctxloom companion add acme"
      Given the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        companions: []
        """
      When I run "ctxloom companion list --format json"
      Then the command succeeds
      And the output reports "0.name" as "acme"

  Rule: add checks the companion before it records anything

    Scenario: A name with nothing on PATH is refused, naming the binary it looked for
      Given an initialized ctxloom project
      When I run "ctxloom companion add nowhere"
      Then the command fails
      And the output contains "ctxloom-companion-nowhere"
      When I run "ctxloom companion list --format text"
      Then the output contains "No companions registered"

  Rule: remove previews by default, and applies only with --yes

    Scenario: Remove without --yes names the apply command and changes nothing
      Given an initialized ctxloom project
      And a companion "ctxloom-companion-acme" is on PATH, not registered
      When I run "ctxloom companion add acme"
      And I run "ctxloom companion remove acme --format text"
      Then the command succeeds
      And the output contains "ctxloom companion remove acme --yes"
      And the home config registers the companion "acme" by name only

  Rule: list reports each registered name and whether it resolves on PATH

    Listing runs nothing: it resolves names on PATH and reports what it found.

    Scenario: List answers which binary each registered name resolves to
      Given an initialized ctxloom project
      And a companion "ctxloom-companion-acme" is on PATH, not registered
      When I run "ctxloom companion add acme"
      And I run "ctxloom companion list --format json"
      Then the command succeeds
      And the output reports "0.name" as "acme"
      And the output reports "0.bin" as "ctxloom-companion-acme"
      And the output reports "0.resolves" as "true"

    Scenario: Bare companion lists the registered companions
      Given an initialized ctxloom project
      And a companion "ctxloom-companion-acme" is on PATH, not registered
      When I run "ctxloom companion add acme"
      And I run "ctxloom companion"
      Then the command succeeds
      And the output contains "acme"
      And the output contains "ctxloom-companion-acme"
      And the output does not contain "Available Commands:"

  Rule: disable keeps a companion registered but its guidance reaches nothing; enable restores it; remove unregisters it

    `companion disable <name>` and `companion enable <name>` write the user's
    home, as `add` and `remove` do. A disabled companion is still listed, and
    contributes nothing to any session or setup interview until it is enabled
    again. Removing a companion unregisters it.

    Scenario: A disabled companion stays registered, and its guidance reaches nothing
      # new: companion disable.
      Given an initialized ctxloom project
      And a companion "ctxloom-companion-acme" is on PATH, not registered
      And Alice has added the companion "acme"
      When Alice disables it:
        """
        ctxloom companion disable acme
        """
      Then the command succeeds
      And the companion "acme" is still registered in her home, disabled
      And a session's context carries none of "acme"'s guidance

    Scenario: Enabling a disabled companion restores its guidance
      # new: companion enable.
      Given an initialized ctxloom project
      And a companion "ctxloom-companion-acme" is on PATH, not registered
      And Alice has added the companion "acme" and disabled it
      When Alice enables it again:
        """
        ctxloom companion enable acme
        """
      Then the command succeeds
      And a session's context carries "acme"'s guidance

    Scenario: Removing a companion unregisters it
      # new: remove's contract beside enable and disable.
      Given an initialized ctxloom project
      And a companion "ctxloom-companion-acme" is on PATH, not registered
      And Alice has added the companion "acme"
      When Alice removes it:
        """
        ctxloom companion remove acme --yes
        """
      Then the command succeeds
      And the companion "acme" is not registered
