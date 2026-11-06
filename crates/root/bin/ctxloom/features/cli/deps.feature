# Ported from cli/deps.feature (rev f36e8dfb2)
# Ported from journeys/j001400_bundle_distribution.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@doc
@pending
Feature: deps — the installed dependency closure, and everything that moves it

  Covers: `ctxloom deps list`, `deps pull`, `deps check`, `deps upgrade`,
  `deps hold`, `deps unhold`, and the bare `ctxloom deps` form.

  A profile names a remote bundle by its short form ("<remote>/<bundle>"; a
  bare name is local). The CLOSURE is every bundle that follows from those
  names, and the lockfile pins each one to a resolved commit, so two checkouts
  of the same project install the same bytes.

  FOUR VERBS, FOUR DIFFERENT JOBS, and the boundaries between them are what
  this file specifies. `list` reads the lockfile and nothing else. `pull` makes
  the installation match upstream. `check` reports which pins could move.
  `upgrade` moves them — and it is the only thing that does: `pull` (forced or
  not) creates first pins and keeps every existing one, even when a profile
  changed its constraint. `upgrade` shows each move first and applies it only
  with --yes.

  Where content comes FROM is the other noun — see cli/remote.feature.

  # THE PIN AND THE PAYLOAD ARE TWO DIFFERENT QUESTIONS. These scenarios assert
  # that changed upstream content actually reaches an assembled/materialized
  # context, not just the lockfile; that a stale local clone checkout never
  # leaches into what is served; and that a skipped pull says what is true
  # about the pin rather than implying the content is current.

  Rule: The listing is offline and answers when nothing else can

    `deps list` reads the lockfile. That makes it the one question still
    answerable with no network, an expired credential, or a remote that has
    been deleted — and the reason it deliberately says nothing about whether
    anything newer exists upstream.

    # Tabled by format: `deps list` is wired to emit(), so off a terminal
    # (which this harness always is) the no-flag row now gets the JSON
    # depsListing (an empty "deps" array), not the "No dependencies
    # installed." text line. Reuses the branch's shared empty-collection step.
    Scenario Outline: A project with nothing installed reports an empty closure
      Given an initialized ctxloom project
      When Alice asks what she has installed:
        """
        ctxloom <flags> deps list
        """
      Then the command succeeds
      And the output reports "deps" as empty, saying "No dependencies installed"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         |
        |               |
        | --format json |
        | --format text |

    # NOT tabled, pinned to --format text: this remedy line
    # ("run 'ctxloom deps pull' to install it") is written straight to
    # renderDepsList's text-only branch — depsListing carries only {deps,
    # count}, no hint field — so a json row has nothing to assert here.
    Scenario: A project with nothing installed says what to run next
      Given an initialized ctxloom project
      When Alice asks what she has installed:
        """
        ctxloom --format text deps list
        """
      Then the command succeeds
      And the output contains "ctxloom deps pull"

    Scenario: The listing names each dependency, its commit and its origin
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      When Alice asks what she has installed:
        """
        ctxloom deps list
        """
      Then the command succeeds
      And the output contains "demo"
      And the output contains "origin"

    # The bare noun answers rather than teaches, through the same seam
    # `ctxloom remote` uses.
    # Tabled by format: same reasoning as the Outline above — bare `deps`
    # aliases to `deps list`.
    Scenario Outline: Bare deps lists the closure
      Given an initialized ctxloom project
      When I run "ctxloom <flags> deps"
      Then the command succeeds
      And the output reports "deps" as empty, saying "No dependencies installed"
      And the output does not contain "Available Commands:"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         |
        |               |
        | --format json |
        | --format text |

    # A hold changes what `upgrade` is allowed to do, so a listing that could
    # not show it would be a listing you cannot plan from.
    Scenario: A held dependency is marked held in the listing
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And I run "ctxloom deps hold origin/demo"
      When Alice reviews what is frozen:
        """
        ctxloom deps list
        """
      Then the command succeeds
      And the output contains "held"

  Rule: Pull installs exactly what is pinned, and never overstates what happened

    `deps pull` records the pin straight into the active lock. It is also
    incremental: an item whose lock entry
    already resolves from the clone cache is not re-fetched, and is reported
    as kept at its locked commit rather than "already installed" — a phrase
    that reads as "you have the latest", which upstream having since moved
    makes false.

    Scenario: Referencing a remote bundle and pulling it locks the dependency closure
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      When Alice installs the content her profile draws on:
        """
        ctxloom deps pull
        """
      Then the command succeeds
      And the file ".ctxloom/lock.yaml" contains "//bundles/demo"

    # Registering a remote is the one act that admits a repository's content.
    # An address alone admits nothing: the pull is refused, it says how to
    # register the repository, and nothing is registered or pinned behind the
    # user's back.
    Scenario: Pulling straight from an unregistered address is refused
      Given an initialized ctxloom project
      And the profile "dev" draws on a bundle straight from an unregistered git repository
      When Alice installs the content her profile draws on:
        """
        ctxloom deps pull
        """
      Then the command fails
      And the output contains "remote not registered"
      And the output contains "ctxloom remote create <name> file://"
      And the file ".ctxloom/lock.yaml" does not exist
      When I run "ctxloom remote list --format json"
      Then the JSON output array "remotes" contains no object whose "name" is "remote"

    # A profile shipped in a repository may name only that repository's
    # content: registering one remote must never admit another repository its
    # profiles happen to name.
    Scenario: A remote profile reaching into another repository is refused
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And a git remote "vendor" serving a bundle "kit" whose profile "reach" draws on bundle "demo" of the remote "origin"
      And I run "ctxloom profile create dev --include vendor/kit"
      And I run "ctxloom deps pull"
      When Alice looks at the vendor's profile:
        """
        ctxloom profile show vendor/kit#profiles/reach
        """
      Then the command fails
      And the output contains "a remote profile may refer only to bundles in its own repository"
      And the output contains "//bundles/demo"

    # Composing several repositories is what a profile of the user's own is
    # for: each repository it names was registered, so each is admitted.
    Scenario: A local profile draws on several registered remotes
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And a git remote "vendor" serving a ctxloom bundle
      And I run "ctxloom profile create dev --include origin/demo --include vendor/demo"
      When Alice installs the content her profile draws on:
        """
        ctxloom deps pull
        """
      Then the command succeeds
      And the file ".ctxloom/lock.yaml" contains "//bundles/demo" exactly 2 times

    Scenario: A second pull is incremental — an already-locked dependency is not re-fetched
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      When Alice pulls again with nothing new pinned:
        """
        ctxloom deps pull --format text
        """
      Then the command succeeds
      And the output contains "Skipped (kept at their locked commit)"

    # This pins BOTH halves: the old content actually stays (a plain pull
    # never moves an existing pin — only `deps upgrade` does), AND the
    # output says what is true rather than implying currency. Reporting
    # "Skipped (already installed)" here once cost an hour diagnosing a false
    # "stale content" bug, because that phrasing is indistinguishable from
    # "you have the latest".
    Scenario: A skipped pull leaves old content in place, and says so rather than implying it is current
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" changes fragment "demo-frag" to "MARKER-SKIPPED-PULL-never-seen"
      When Alice pulls again while upstream has moved on:
        """
        ctxloom deps pull --format text
        """
      Then the command succeeds
      And the output contains "Skipped (kept at their locked commit)"
      And the output contains "ctxloom deps check"
      And the output does not contain "ctxloom deps upgrade"
      And the output does not contain "already installed"
      When I run "ctxloom materialize dev --target out --backend claude-code"
      Then the command succeeds
      And the file "out/CLAUDE.md" contains "Demo fragment content."
      And the file "out/CLAUDE.md" does not contain "MARKER-SKIPPED-PULL-never-seen"

  Rule: Pull writes the project's engine files only with --yes

    After a pull installs remote content, a project refresh re-delivers the
    project's hooks, MCP servers, settings, commands and skills into the
    project directory's engine files. That directory is shared by every
    session and person using the checkout, so the refresh asks the same
    consent `ctxloom materialize` asks before writing there: `--yes`. Without
    it the PULL STILL SUCCEEDS. It installs and locks the closure, writes no
    engine file, and names what the refresh would have changed and the two
    ways to apply it. The note compares against the project as it stands: a
    project already current gets no note, and a kind the refresh would be
    refused is named with its fix.

    Scenario: A pull without --yes installs the closure and leaves the project's engine files alone
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      When Alice pulls without consenting to the project refresh:
        """
        ctxloom deps pull --format text
        """
      Then the command succeeds
      And the file ".ctxloom/lock.yaml" contains "//bundles/demo"
      And the file ".claude/settings.json" does not exist
      And the output contains "Project refresh skipped"
      And the output contains "ctxloom deps pull --yes"
      And the output contains "ctxloom materialize --yes"

    Scenario: A pull right after a --yes pull has no refresh to skip
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And Alice has already let a pull refresh the project:
        """
        ctxloom deps pull --yes --format text
        """
      When Alice pulls again without consenting to the project refresh:
        """
        ctxloom deps pull --format text
        """
      Then the command succeeds
      And the output does not contain "Project refresh skipped"

    Scenario: A refresh the project would refuse is named with its fix, and the pull still succeeds
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And Alice has already let a pull refresh the project:
        """
        ctxloom deps pull --yes --format text
        """
      And the project already has the file ".claude/settings.json":
        """
        { this is not json
        """
      When Alice pulls again without consenting to the project refresh:
        """
        ctxloom deps pull --format text
        """
      Then the command succeeds
      And the output contains "Project refresh skipped"
      And the output contains "hooks refused"
      And the output contains "fix: "

    Scenario: A pull with --yes also refreshes the project's engine files
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      When Alice pulls and lets the refresh write the project:
        """
        ctxloom deps pull --yes --format text
        """
      Then the command succeeds
      And the file ".claude/settings.json" contains "session-start"
      And the output does not contain "Project refresh skipped"

  Rule: Pull reconciles to upstream — it removes what upstream no longer has

    Installed content is a PROJECTION of remote state, not user data: every
    byte of it can be re-fetched from the address it came from. So a pull that
    finds a bundle gone upstream removes it locally and says which, by name,
    without asking. That is synchronization, not destruction, and it is why
    removing asks for no `--yes`.

    Scenario: A bundle deleted upstream is removed locally, by name
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" stops publishing bundle "demo"
      When Alice pulls after upstream deleted the bundle:
        """
        ctxloom deps pull --format text
        """
      Then the command succeeds
      And the output contains "no longer published"
      And the output contains "demo"
      And the file ".ctxloom/lock.yaml" does not contain "//bundles/demo"

  Rule: "I could not reach upstream" is never "upstream deleted everything"

    THE PATHOLOGICAL CASE, and the one this whole rule exists for. An
    unreachable remote, an expired credential and an API that answers with an
    empty list all produce the same local observation as a genuine deletion:
    nothing came back. Treating that as authority turns one auth failure into
    an emptied installation, reported as a successful sync.

    So absence is authority only from a remote this run PROVED it could read.
    A remote it could not reach is reported as unchecked, and every dependency
    from it is kept exactly where it was.

    Scenario: An unreachable remote removes nothing and says it could not check
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" becomes unreachable
      When Alice pulls while the remote is unreachable:
        """
        ctxloom deps pull --format text
        """
      Then the output contains "could not be reached"
      And the output does not contain "no longer published"
      And the file ".ctxloom/lock.yaml" contains "//bundles/demo"
      When I run "ctxloom materialize dev --target out --backend claude-code"
      Then the file "out/CLAUDE.md" contains "Demo fragment content."

    # The lockfile surviving is not enough on its own: a pull that pruned the
    # CONTENT while leaving the pin behind would pass a lockfile-only
    # assertion and still have emptied what the agent receives. So the
    # offline listing is read back too — it is the surface a person would use
    # to find out what just happened to their installation.
    Scenario: A remote that cannot be reached leaves the listing intact
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" becomes unreachable
      And I run "ctxloom deps pull"
      When Alice checks what survived:
        """
        ctxloom deps list
        """
      Then the command succeeds
      And the output contains "demo"

    # The lock rebuild after a pull keeps the previous entries of whatever part
    # of the closure it could not reach. That is safe only if it is SAID: a
    # summary with no such line reads as a complete lock.
    Scenario: A pull whose lock could not reach part of the closure names what it kept
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom remote create gone file:///nonexistent/nonexistent-ctxloom-remote --forge git"
      And I run "ctxloom profile create dev --include origin/demo"
      And the project already has the file ".ctxloom/content/bundles/v2/project/profiles/orphan.yaml":
        """
        parents:
          - file:///nonexistent/nonexistent-ctxloom-remote@bundles/kit#profiles/parent
        """
      When Alice pulls with part of the closure unreachable:
        """
        ctxloom deps pull --format text
        """
      Then the output contains "Lock incomplete"
      And the output contains "nonexistent-ctxloom-remote"
      And the output contains "previous lock entries were kept"

    # The same rule for the commands that report currency. A check that could
    # not fetch the remote has only the clone it fetched LAST time to read, and
    # an answer read from that clone is about the past: it is unchecked, never
    # "up to date".
    Scenario: A check that could not reach the remote does not call it up to date
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" becomes unreachable
      When Alice checks for updates while the remote is unreachable:
        """
        ctxloom deps check --format text
        """
      Then the output does not contain "up to date"
      And the output contains "could not be checked"

    Scenario: An upgrade that could not reach the remote does not call everything up to date
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" becomes unreachable
      When Alice upgrades while the remote is unreachable:
        """
        ctxloom deps upgrade --format text
        """
      Then the output does not contain "Everything is up to date"
      And the output contains "unreachable"

  Rule: Check reads, upgrade shows, and upgrade --yes writes

    `deps check` reaches the network and reports; it changes nothing, which is
    what makes it safe to run anywhere. `deps upgrade` re-resolves each
    profile's closure to the newest commit its constraint allows and shows what
    every move brings in; with --yes it writes the advance straight to the
    active lock — no staging, no approval.

    Scenario: Check reports an available advance and changes nothing
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" advances its bundle
      When Alice asks what could be advanced:
        """
        ctxloom deps check --format text
        """
      Then the command succeeds
      And the output contains "ctxloom deps upgrade"
      When I run "ctxloom deps list"
      Then the output contains "demo"

    Scenario: Check on an empty closure has nothing to check
      Given an initialized ctxloom project
      When I run "ctxloom deps check --format text"
      Then the command succeeds
      And the output contains "nothing to check"

    Scenario: An upgrade advances the pin directly
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" advances its bundle
      When Alice advances her pins to the newest commit:
        """
        ctxloom deps upgrade --yes --format text
        """
      Then the command succeeds
      And the output contains "Applied 1 pin(s)."

    # Upgrade rewrites the lock wholesale from the closure, so an entry the
    # project stopped composing disappears in that write. Unnamed, a removal
    # reads as a pin that never existed.
    Scenario: An upgrade names each dependency it drops from the lock
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And a git remote "other" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo --include other/demo"
      And I run "ctxloom deps pull"
      And I run "ctxloom profile modify dev --remove-bundle other/demo"
      When Alice advances her pins to the newest commit:
        """
        ctxloom deps upgrade --yes --format text
        """
      Then the command succeeds
      And the output contains "from the lockfile: nothing this project composes depends on it any more."

    # `deps pull` is offline by design: it cannot observe whether upstream has
    # moved, so it must not assert that it has. The entry was advanced one
    # command ago; telling the user to advance it again is advice the tool has
    # no evidence for.
    Scenario: A pull right after an upgrade does not tell the user to upgrade again
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" advances its bundle
      And I run "ctxloom deps upgrade --yes"
      When Alice pulls immediately after advancing her pins:
        """
        ctxloom deps pull
        """
      Then the command succeeds
      And the output does not contain "ctxloom deps upgrade"

  Rule: Only upgrade moves a pin, and it shows the move before --yes applies it

    A pin is the decision to run what a bundle ships, so moving one is shown
    before it happens: every item added, removed or changed, what each hook and
    MCP server runs before and after, and the diff of every changed script. Env
    and header VALUES may be credentials, so they are shown only by name and a
    fingerprint of the value: a change is visible, the secret is not.
    Without --yes nothing is written; with it the closure is resolved again and
    what was actually applied is shown. A FIRST pin — the one `pull` creates —
    is shown the same way, with everything the bundle brings in. `pull` never
    moves an existing pin: a changed constraint is reported and waits for
    `upgrade`.

    Scenario: An upgrade without --yes shows the move and writes nothing
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" ships an MCP server "srv" running "fixture-mcp-one" and a skill script printing "SCRIPT-ONE"
      When Alice asks what an upgrade would do:
        """
        ctxloom deps upgrade --format text
        """
      Then the command succeeds
      And the output contains "+ mcp srv"
      And the output contains "command: fixture-mcp-one"
      And the output contains "+ skill runner"
      And the output contains "+echo SCRIPT-ONE"
      And the output contains "1 pin(s) would move. Re-run with --yes to apply."
      When I run "ctxloom deps upgrade --format text"
      Then the output contains "1 pin(s) would move."

    Scenario: An upgrade with --yes applies the move and shows what it applied
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" ships an MCP server "srv" running "fixture-mcp-one" and a skill script printing "SCRIPT-ONE"
      When Alice applies the upgrade:
        """
        ctxloom deps upgrade --yes --format text
        """
      Then the command succeeds
      And the output contains "+ mcp srv"
      And the output contains "Applied 1 pin(s)."
      When I run "ctxloom deps upgrade --format text"
      Then the output contains "Everything is up to date."

    Scenario: An upgrade shows what an MCP server and a script run before and after
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And the remote "origin" ships an MCP server "srv" running "fixture-mcp-one" and a skill script printing "SCRIPT-ONE"
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" ships an MCP server "srv" running "fixture-mcp-two" and a skill script printing "SCRIPT-TWO"
      When Alice asks what an upgrade would change:
        """
        ctxloom deps upgrade --format text
        """
      Then the command succeeds
      And the output contains "~ mcp srv"
      And the output contains "command: fixture-mcp-one -> fixture-mcp-two"
      And the output contains "-echo SCRIPT-ONE"
      And the output contains "+echo SCRIPT-TWO"
      And the output contains "~ API_KEY: <"
      And the output does not contain "secret-SCRIPT-ONE"
      And the output does not contain "secret-SCRIPT-TWO"

    Scenario: A first pin shows everything the bundle brings in, executables included
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And the remote "origin" ships an MCP server "srv" running "fixture-mcp-one" and a skill script printing "SCRIPT-ONE"
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      When Alice pulls a bundle for the first time:
        """
        ctxloom deps pull --format text
        """
      Then the command succeeds
      And the output contains "New pins, with everything each one brings in:"
      And the output contains "first pin"
      And the output contains "+ mcp srv"
      And the output contains "command: fixture-mcp-one"
      And the output contains "+ fragment demo-frag"
      And the output contains "+echo SCRIPT-ONE"
      And the output contains "API_KEY: <"
      And the output does not contain "secret-SCRIPT-ONE"

    Scenario: A pull never moves an existing pin, even when its constraint changed
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" advances its bundle
      And I run "ctxloom profile modify dev --remove-bundle origin/demo --add-bundle origin/demo@main"
      When Alice pulls after changing the constraint:
        """
        ctxloom deps pull --force --format text
        """
      Then the command succeeds
      And the output contains "the manifest now asks for main; the pin stays at"
      And the output contains "ctxloom deps upgrade --yes"
      And the output does not contain "New pins"
      When I run "ctxloom deps upgrade --format text"
      Then the output contains "1 pin(s) would move."

  Rule: A hold freezes one dependency, even against a forced pull

    `deps hold` is the opt-out: a held entry stays frozen against `upgrade`,
    and against a `pull --force`, which reinstalls each reference rather than
    skipping it.

    Scenario: A held dependency is not upgraded
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And I run "ctxloom deps hold origin/demo"
      And the remote "origin" advances its bundle
      When Alice tries to advance a held pin:
        """
        ctxloom deps upgrade --format text
        """
      Then the command succeeds
      And the output contains "up to date"

    # THE MECHANIC A NEW CLONE RELIES ON (see j000800_onboarding.feature's
    # "Bob receives the versions the team pinned, not the latest ones"): an
    # ordinary pull SKIPS a reference it already considers installed and
    # never consults the pin at all. `--force` puts the pull on the full
    # fetch-and-record path (Puller.Pull, then Puller.updateLockfile) against
    # the advanced remote, and the held content has to survive it.
    Scenario: A held dependency's content survives even a forced pull
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And I run "ctxloom deps hold origin/demo"
      And the remote "origin" advances its bundle
      When Alice forces a pull of every reference:
        """
        ctxloom deps pull --force
        """
      Then the command succeeds
      And the file ".ctxloom/lock.yaml" contains "held: true"
      When I run "ctxloom materialize dev --target out --backend claude-code"
      Then the file "out/CLAUDE.md" contains "Demo fragment content."
      And the file "out/CLAUDE.md" does not contain "version two"

    Scenario: Unholding lets the pin move again
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And I run "ctxloom deps hold origin/demo"
      And the remote "origin" advances its bundle
      When Alice releases the freeze:
        """
        ctxloom deps unhold origin/demo
        """
      Then the command succeeds
      When I run "ctxloom deps upgrade --yes --format text"
      Then the command succeeds
      And the output contains "Applied 1 pin(s)."

  Rule: What lands in the lockfile is not what reaches the agent

    The pin is not the point: what actually lands in front of the agent is.
    Upgrading to changed upstream content must replace
    what the agent sees — not just what the lockfile records. And content is
    served by resolving the remote-tracking ref a fetch advanced, never by
    whatever the cached clone's checked-out working tree happens to hold.

    Scenario: An upstream content change reaches the assembled context once the pin advances
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And I run "ctxloom materialize dev --target before --backend claude-code"
      Then the file "before/CLAUDE.md" contains "Demo fragment content."
      When the remote "origin" changes fragment "demo-frag" to "MARKER-BRAVO-second-edition"
      And Alice advances the pin to the revised content:
        """
        ctxloom deps upgrade --yes
        """
      And I run "ctxloom materialize dev --target after --backend claude-code"
      Then the command succeeds
      And the file "after/CLAUDE.md" contains "MARKER-BRAVO-second-edition"
      And the file "after/CLAUDE.md" does not contain "Demo fragment content."

    # A clone's checked-out HEAD is not the source of truth for content — the
    # remote-tracking refs a fetch advances are. Forcing the local checkout
    # back to the very first commit must not resurrect old content.
    Scenario: A stale local checkout never leaks into what's served
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      And I run "ctxloom remote default origin"
      And I run "ctxloom profile create dev --include origin/demo"
      And I run "ctxloom deps pull"
      And the remote "origin" changes fragment "demo-frag" to "MARKER-STALE-CHECKOUT-current"
      And I run "ctxloom deps upgrade --yes"
      And the remote "origin"'s cached clone is forced back to its first commit
      When Alice materializes after the local checkout went stale:
        """
        ctxloom materialize dev --target out --backend claude-code
        """
      Then the command succeeds
      And the file "out/CLAUDE.md" contains "MARKER-STALE-CHECKOUT-current"
      And the file "out/CLAUDE.md" does not contain "Demo fragment content."

  Rule: A pull keeps a published bundle's tree exactly as it was published

    A pulled bundle is the publisher's tree, file for file. Each kind keeps its
    metadata where that kind stores it, structures stay structures, a skill
    package arrives whole with its modes, and hooks keep their declared order.

    Background:
      Given Trent authors a directory-form bundle "atelier" carrying every surface kind

    # Every artifact, profile included, is compared with the published bytes.
    # The steps read the consumer's pulled tree, not the project directory.
    Scenario Outline: A pulled tree carries every published artifact byte for byte
      # new: the publication half of j001400's per-kind scenario and of its profile scenario.
      Given Trent publishes the "atelier" tree to his company repo
      When Alice references the company's "atelier" bundle and pulls it
      Then Alice's pulled "atelier" tree carries "<artifact>" byte for byte as published

      Examples:
        | artifact                        |
        | fragments/house-style.md        |
        | prompts/ship-it.md              |
        | mcp/ledger.yaml                 |
        | hooks/post_file_edit/stamp.yaml |
        | skills/reviewer/SKILL.md        |
        | profiles/studio.yaml            |

    # Content kinds carry their metadata as front-matter inside the file;
    # executable and structural kinds carry it in a separate sidecar file,
    # which a pull that copied only content files would lose.
    Scenario Outline: Each kind's metadata survives publication in the place that kind stores it
      Given Trent publishes the "atelier" tree to his company repo
      When Alice references the company's "atelier" bundle and pulls it
      Then Alice's pulled "atelier" tree stores the "<kind>" metadata "<probe>" as <placement>

      Examples:
        | kind     | probe                    | placement                                  |
        | fragment | ATELIER-FRAGMENT-DESC    | front-matter in "fragments/when-shipping.md" |
        | command  | ATELIER-COMMAND-DESC     | front-matter in "prompts/ship-it.md"       |
        | mcp      | ATELIER-MCP-DESC         | the sidecar "mcp/.ledger.meta.yaml"        |
        | skill    | ATELIER-SKILL-DESC       | the sidecar "skills/.reviewer.meta.yaml"   |

    # An MCP server is a structure: its command, args and env are compared
    # parsed, so a flattened or reordered round trip fails where a substring
    # match would pass.
    Scenario: A published MCP server's configuration structure survives, not merely its text
      Given Trent publishes the "atelier" tree to his company repo
      When Alice references the company's "atelier" bundle and pulls it
      Then Alice's pulled "atelier" MCP server "ledger" parses with its command, args and env intact

    # A skill is the one multi-file item, and the one where a file's mode
    # matters: the whole package is compared, and its script must stay
    # executable.
    Scenario: A published skill package arrives whole, with its script still executable
      Given Trent publishes the "atelier" tree to his company repo
      When Alice references the company's "atelier" bundle and pulls it
      Then Alice's pulled skill "reviewer" contains exactly the files Trent published, byte for byte
      And Alice's pulled skill "reviewer" file "scripts/run.sh" is executable

    # A tree stores one file per hook, so a hook's place in its event's
    # sequence is the order field of its sidecar. The fixture's declared order
    # (stamp before audit) disagrees with the alphabetical one, and the pulled
    # hooks are read the way assembly reads them: bucketed by each hook's own
    # declared event and sorted by its order.
    Scenario: Hooks arrive in the right event buckets AND in their declared order within each
      Given Trent publishes the "atelier" tree to his company repo
      When Alice references the company's "atelier" bundle and pulls it
      Then Alice's pulled "atelier" hooks under "post_file_edit" are exactly "stamp, audit" in that order
      And Alice's pulled "atelier" hooks under "session_start" are exactly "greet" in that order
      And no pulled "atelier" hook appears under an event it was not declared in
