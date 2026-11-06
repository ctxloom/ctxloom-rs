# Ported from cli/session.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).
#   The rest of Go's file is in the files that say `Ported from cli/session.feature`.
#   Left out: the `session remove` Rule (retired: `session purge`
#   is the one destroyer, cli/session_purge.feature) and the `session adopt`
#   Rule (dropped: it repaired transcripts a Go defect orphaned).
# Ported from journeys/j001200_recall.feature (its empty search, the last scenario of the reading Rule)

@doc
@pending
Feature: session — the record of what your assistant did, and the tools to prune it

  Every `ctxloom run` mints a harp-named session and writes down what
  happened. The `session` noun is how you read that record back and how you
  throw parts of it away. Reading is free; every destroyer reports first and
  changes nothing until you say `--yes`.

  This is the comprehensive per-noun spec: what the noun DOES, leaf by leaf,
  hermetically against a seeded session index. The narrative versions — Alice
  recalling a conversation from March, Alice closing a shipped workstream out
  — are journeys/j001200_recall.feature and j001300_closeout.feature, which
  assert what a PERSON sees.

  Surfaces covered here:

    ctxloom session                     (bare: lists)
    ctxloom session list [--state live|ended|unknown]
    ctxloom session show <harp>
    ctxloom session edit <harp> --name <new>
    ctxloom session keep <harp> [--undo]
    ctxloom session search <word>... [--state live|ended|unknown]
    ctxloom session transcript              (bare: lists)
    ctxloom session transcript list [<harp>]
    ctxloom session transcript show <harp>
    ctxloom session transcript watch <harp>
    ctxloom session worktrees               (bare: lists)
    ctxloom session worktrees list [<harp>]
    ctxloom session purge <harp> | --older-than <age> [--yes]
    ctxloom session approvals

  Rule: Reading the record touches nothing

    Scenario: The bare noun lists what has been recorded
      Given an initialized ctxloom project
      And a recorded session "amber-swift-owl"
      When I run "ctxloom session"
      Then the command succeeds
      And the output contains "amber-swift-owl"

    Scenario: --all reaches past the current project
      Given an initialized ctxloom project
      And a recorded session "amber-swift-owl"
      When I run "ctxloom session list --all"
      Then the command succeeds
      And the output contains "amber-swift-owl"

    # Tabled by format: `session list` is wired to emit(), so off a terminal
    # (which this harness always is) the no-flag row now gets the JSON rows
    # array (empty here), not the "(no sessions)" text line. Reuses the
    # branch's shared empty-collection step rather than a new spelling.
    Scenario Outline: A fresh project has recorded nothing
      Given an initialized ctxloom project
      When I run "ctxloom <flags> session list"
      Then the command succeeds
      And the output reports "$" as empty, saying "no sessions"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         |
        |               |
        | --format json |
        | --format text |

    # A session's title is the first thing its user asked, read from its
    # recorded transcript: the listing, the search and `show` all carry it.
    Scenario: Showing a session prints its record, titled by its first prompt
      Given an initialized ctxloom project
      And a recorded session "amber-swift-owl"
      When I run "ctxloom session show amber-swift-owl --format text"
      Then the command succeeds
      And the output contains "Seeded prompt for amber-swift-owl."
      And the output contains "engine:"
      When I run "ctxloom session list --format json"
      Then the JSON output array "$" contains an object whose "harp" is "amber-swift-owl" and whose "summary" is "Seeded prompt for amber-swift-owl."

    # The index says ended; the lock says running. The lock wins in the
    # listing too: ended_at is when the LAST run ended and a resume keeps it,
    # so it is no answer to "is this running now". The state is the lock's
    # verdict — held is live, free is ended, no lock file is unknown — and a
    # live session's end is blank rather than a stamp from its previous run.
    Scenario: The listing takes each session's state from its lock, not its end stamp
      Given an initialized ctxloom project
      And a session "amber-swift-owl" with a transcript that is running again
      And a finished session "brisk-copper-moth" with a transcript
      And a finished session "calm-dusty-wren" with a transcript but no session lock
      When I run "ctxloom session list --all --format json"
      Then the command succeeds
      And the JSON output array "$" contains an object whose "harp" is "amber-swift-owl" and whose "state" is "live"
      And the JSON output array "$" contains an object whose "harp" is "amber-swift-owl" and that has no "end"
      And the JSON output array "$" contains an object whose "harp" is "brisk-copper-moth" and whose "state" is "ended"
      And the JSON output array "$" contains an object whose "harp" is "calm-dusty-wren" and whose "state" is "unknown"
      When I run "ctxloom session list --all --format text"
      Then the command succeeds
      And the output contains "STATE"

    Scenario Outline: --state keeps only the sessions in that state
      Given an initialized ctxloom project
      And a session "amber-swift-owl" with a transcript that is running again
      And a finished session "brisk-copper-moth" with a transcript
      And a finished session "calm-dusty-wren" with a transcript but no session lock
      When I run "ctxloom session <leaf> --all --state <state> --format json"
      Then the command succeeds
      And the JSON output array "$" contains an object whose "harp" is "<kept>" and whose "state" is "<state>"
      And the JSON output array "$" contains no object whose "harp" is "<dropped1>"
      And the JSON output array "$" contains no object whose "harp" is "<dropped2>"

      Examples: both listings filter the same way
        | leaf           | state   | kept              | dropped1          | dropped2          |
        | list           | live    | amber-swift-owl   | brisk-copper-moth | calm-dusty-wren   |
        | list           | ended   | brisk-copper-moth | amber-swift-owl   | calm-dusty-wren   |
        | list           | unknown | calm-dusty-wren   | amber-swift-owl   | brisk-copper-moth |
        | search seeded  | live    | amber-swift-owl   | brisk-copper-moth | calm-dusty-wren   |
        | search seeded  | ended   | brisk-copper-moth | amber-swift-owl   | calm-dusty-wren   |

    Scenario: --state refuses a word that is not a state
      Given an initialized ctxloom project
      When I run "ctxloom session list --state running"
      Then the command fails
      And the output contains "live, ended, unknown"

    # "Nothing matched" and "the search did not run" must not look the same:
    # an empty result is said in so many words, never answered with no output.
    Scenario: A query that matches nothing says so rather than printing nothing
      # substitution: a recorded session the query does not match stands in for the journey's March fixture.
      Given an initialized ctxloom project
      And a recorded session "amber-swift-owl"
      When I run "ctxloom session search J001200-MATCHES-ABSOLUTELY-NOTHING"
      Then the command succeeds
      And ctxloom says plainly that nothing matched

  Rule: Renaming is a field assignment, so it rides `edit`

    A session's name is a field on its record, not a thing with a verb of
    its own. `edit` is the one mutation verb for the noun, and `--name` is
    the assignment that renames the harp. The backend transcript is
    unaffected: the session keeps its bound session id and its transcript,
    and only the name it answers to moves. Its output folder
    moves with it, to the new name beside the old, and the record names
    where it went; a rename whose folder cannot move is refused whole.

    @json-default
    Scenario: --name renames the harp and moves its output folder
      Given an initialized ctxloom project
      And a finished session "amber-swift-owl" with a transcript and a summary an older ctxloom left
      When I run "ctxloom session edit amber-swift-owl --name bright-keen-hawk"
      Then the command succeeds
      And the output contains "bright-keen-hawk"
      And the output file "essence.md" of session "bright-keen-hawk" exists
      And the output file "essence.md" of session "amber-swift-owl" does not exist
      When I run "ctxloom session list --all"
      Then the JSON output array "$" contains an object whose "harp" is "bright-keen-hawk"
      And the JSON output array "$" contains no object whose "harp" is "amber-swift-owl"

    Scenario: The old rename verb is gone, not hidden
      Given an initialized ctxloom project
      And a recorded session "amber-swift-owl"
      When I run "ctxloom session rename amber-swift-owl bright-keen-hawk"
      Then the command fails
      And the output contains "unknown command"

    # A session has no authored document. Its index entry is machine-written
    # and its title is DERIVED from its transcript, so an editor round-trip
    # here would offer edits nothing keeps. The bare form says so outright and names the
    # assignment it does take, rather than exiting 0 having changed nothing.
    Scenario: A bare edit refuses, because there is no document to open
      Given an initialized ctxloom project
      And a recorded session "amber-swift-owl"
      When I run "ctxloom session edit amber-swift-owl"
      Then the command fails
      And the output contains "no editable document"
      And the output contains "--name"

    Scenario: Editing a session nobody recorded fails
      Given an initialized ctxloom project
      When I run "ctxloom session edit no-such-harp --name bright-keen-hawk"
      Then the command fails
      And the output contains "no-such-harp"

  Rule: The transcript is a population of its own

    A session's recorded conversation has its own listing, its own stream and
    its own destroyer, so it gets its own sub-noun. `session transcript`
    named alone lists — the same bare-noun answer `remote` gives.

    Scenario: The bare sub-noun lists which sessions have a transcript
      Given an initialized ctxloom project
      And a recorded session "amber-swift-owl"
      When I run "ctxloom session transcript"
      Then the command succeeds
      And the output contains "amber-swift-owl"

    # A session whose transcript was never captured is listed SAYING SO.
    # Omitting it would make "nothing was captured" read identically to
    # "there are no sessions".
    Scenario: A session with no captured transcript is still named
      Given an initialized ctxloom project
      And a recorded session "amber-swift-owl" for backend "claude-code"
      When I run "ctxloom session transcript list"
      Then the command succeeds
      And the output contains "amber-swift-owl"
      And the output contains "false"

    Scenario: The harp is a positional, and an unknown one fails
      Given an initialized ctxloom project
      When I run "ctxloom session transcript list no-such-harp"
      Then the command fails
      And the output contains "no-such-harp"

    # Off a terminal (which this harness always is) show's default is NDJSON:
    # the whole transcript, and nothing else, on stdout, then exit.
    Scenario: Show prints the whole transcript and exits, ready for a pipe
      Given an initialized ctxloom project
      And a recorded session "amber-swift-owl"
      When I run "ctxloom session transcript show amber-swift-owl"
      Then the command succeeds
      And every line of stdout is a JSON object
      And the output contains "Seeded prompt for amber-swift-owl."

    Scenario: Watching moved under the transcript it watches
      Given an initialized ctxloom project
      And a recorded session "amber-swift-owl"
      When I run "ctxloom session watch amber-swift-owl"
      Then the command fails
      And the output contains "unknown command"

  Rule: Pending approvals are readable from any terminal, and only readable

    # `session approvals` asks every live coordinator on this host, found by
    # the endpoint files coordinators write under the state root. With none
    # running nothing is parked anywhere, so that is a message naming where
    # discovery looked, not a failure; the structured form says how many
    # coordinators answered, which is what separates "none running" from
    # "running with nothing pending".
    Scenario: With no coordinator running, the text says so and where it looked
      Given an initialized ctxloom project
      When I run "ctxloom --format text session approvals"
      Then the command succeeds
      And the output contains "no coordinator is running"
      And the output contains "no live endpoint under"

    Scenario: With no coordinator running, the JSON reports none answered and nothing pending
      Given an initialized ctxloom project
      When I run "ctxloom --format json session approvals"
      Then the command succeeds
      And the output reports "coordinators" as "0"
      And the output reports "approvals" as empty, saying "no coordinator is running"

    # A SECOND PROCESS reaching a LIVE coordinator: the standing owner is a
    # real `ctxloom run` hosting one, and this command finds it by discovery
    # and asks it over RPC. "coordinators" is 1 only because that round trip
    # happened. The owner is interactive and routes no approvals, so nothing
    # is parked — a request parked by a headless child is not staged here:
    # nothing in this harness can wait on another process's queue without
    # polling it. That proof is the integration lane's
    # TestPermissionContract_AnotherProcessListsTheParkedAsk.
    Scenario: Another terminal reaches the running session's coordinator and finds nothing parked
      Given an initialized ctxloom project
      And a session owner is standing
      When I run "ctxloom --format json session approvals"
      Then the command succeeds
      And the output reports "coordinators" as "1"
      And the output reports "approvals" as empty, saying "no pending approvals"
      When I run "ctxloom --format text session approvals"
      Then the command succeeds
      And the output contains "no pending approvals"

    # The command only lists: there is no argument that names a request to
    # answer, so one passed is refused rather than acted on.
    Scenario: Naming a request to answer is refused
      Given an initialized ctxloom project
      When I run "ctxloom session approvals amber-swift-owl"
      Then the command fails
      And the output contains "unknown command"
