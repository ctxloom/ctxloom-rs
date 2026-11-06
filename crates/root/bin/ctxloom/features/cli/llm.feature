# world: base
# Ported from cli/llm.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending.
@doc
Feature: llm — the named engine configurations

  Covers: `ctxloom llm list`, `llm create`, `llm edit`, `llm remove`,
  `llm default`, and the bare `ctxloom llm` form.

  An LLM entry is a LABEL for an engine configuration: a backend type
  (claude-code, mock, ...), a model string, and a launch-time permission
  posture. Agents reference these labels by name (`ctxloom agent create dev
  --llm big`), so this is the vocabulary cli/agent.feature draws on: a team
  manages it here, once, instead of every binding restating a model string.

  AN ENTRY CARRIES NO CREDENTIALS, AND THAT IS A SECURITY POSTURE, NOT AN
  OMISSION. Every engine authenticates itself, and the process ctxloom
  launches inherits the ambient environment, so a variable exported in the
  shell that runs ctxloom reaches the engine with ctxloom neither seeing nor
  storing it. The project config file is committed; a key that invited a
  secret into it was a documented way to push one. So the key is gone, and a
  config still spelling it is refused at load with the replacement named —
  never decoded into a setting that looks applied and is not.

  Which engine a given agent binds is the other noun — see cli/agent.feature.

  This is the comprehensive per-noun spec: what the noun DOES, leaf by leaf,
  hermetically against a seeded project.

  @pending
  Rule: The vocabulary is enumerable before anything is configured

    A project that has declared no engine config still has engines — the
    registered backends. `llm list` answers with the union of those and any
    labels config.yaml declares, which is exactly the set `agent create
    --engine` and `llm default` accept, so a name offered here is never
    rejected there.

    # ONE BODY, ONCE PER ENCODING. Off a terminal ctxloom derives JSON, so the
    # no-flag row exercises the DEFAULT every script receives; the explicit
    # rows prove a typed --format wins in both directions. The structured
    # rows address each backend BY PATH rather than by a substring that would
    # also match across field boundaries.
    Scenario Outline: The listing names the built-in backends and marks the default
      Given an initialized ctxloom project
      When Alice asks which engines she can bind:
        """
        ctxloom llm list <flags>
        """
      Then the command succeeds
      And the output reports "[label=claude-code].default" as "<claude-code is the default>"
      And the output reports "[label=claude-fast].label" as "<names another built-in too>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | claude-code is the default | names another built-in too |
        |               | true                        | claude-fast                 |
        | --format json | true                        | claude-fast                 |
        | --format text | (default)                   | claude-fast                 |

    # The bare noun answers the question somebody typing it has, rather than
    # teaching them what they could have typed instead.
    Scenario: Bare llm lists the engines
      Given an initialized ctxloom project
      When I run "ctxloom llm"
      Then the command succeeds
      And the output contains "claude-code"
      And the output does not contain "Available Commands:"

  @pending
  Rule: Every row says whether the team wrote it or ctxloom supplied it

    That union is two very different kinds of thing. A LABEL is something a
    team authored in config.yaml and maintains; a BUILT-IN is a bare engine
    name ctxloom fills in for a project that configured none. Printed
    identically they read as one list of decisions somebody made, and acting on
    that misreading is exactly how `llm remove claude-code` came to report
    success on a project that had never written an llm.configs line. So the
    listing marks BOTH kinds — not one, leaving the other bare, which a reader
    would take for missing information rather than an answer.

    Scenario Outline: A label the team wrote is told apart from an engine ctxloom supplied
      Given an initialized ctxloom project
      And I run "ctxloom llm create big --type mock --model o1"
      And I run "ctxloom llm default mock"
      When Alice asks which of these engines her team actually configured:
        """
        ctxloom llm list <flags>
        """
      Then the command succeeds
      # One listing, both kinds. Asserting either marker alone would pass just
      # as happily against a render that stamped every row the same way.
      And the output reports "[label=big].authored" as "<big is configured>"
      And the output reports "[label=claude-code].authored" as "<claude-code is built-in>"
      # The origin marker does not displace the default marker: a row can be
      # the fallback engine everything resolves to and still be one nobody
      # configured.
      And the output reports "[label=mock].default" as "<mock is the default>"
      And the output reports "[label=mock].authored" as "<and still built-in>"
      # The marker is not decoration — it predicts what the label can DO. A
      # name ctxloom supplied has no entry to take away, and remove says so
      # rather than reporting a deletion that never happened.
      When I run "ctxloom llm remove claude-code"
      Then the command fails
      And the output contains "not defined in config.yaml"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | big is configured | claude-code is built-in | mock is the default | and still built-in |
        |               | true               | false                    | true                  | false                |
        | --format json | true               | false                    | true                  | false                |
        | --format text | [configured]       | [built-in]               | (default)             | [built-in]           |

  @pending
  Rule: The default is a label, and only a label the project knows

    Asserting only the exit code once let a show path that returned an EMPTY
    default pass. What a project with no llm block resolves to is a fact worth
    stating, and it is the effect the command exists to report.

    Scenario: A project with no llm block still reports the default it resolves to
      Given an initialized ctxloom project
      When I run "ctxloom llm default"
      Then the command succeeds
      And the output contains "claude-code"

    # Read back through the OTHER surface on purpose: `llm default` reporting
    # what `llm default` just set proves the two calls agree with each other,
    # not that anything was written. The listing's default marker is derived
    # from stored config, so it can only move if the write landed.
    Scenario Outline: Setting the default moves the marker in the listing
      Given an initialized ctxloom project
      When Alice makes claude-fast the engine everything falls back to:
        """
        ctxloom llm default claude-fast <flags>
        """
      Then the command succeeds
      And the output reports "status" as "<confirms the change>"
      When I run "ctxloom llm default <flags>"
      Then the output reports "default" as "<reads back claude-fast>"
      When I run "ctxloom llm list <flags>"
      Then the output reports "[label=claude-fast].default" as "<claude-fast is now marked default>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | confirms the change             | reads back claude-fast | claude-fast is now marked default |
        |               | set                              | claude-fast             | true                                |
        | --format json | set                              | claude-fast             | true                                |
        | --format text | Default LLM set to: claude-fast | claude-fast             | claude-fast (default)               |

    # A default nobody can resolve is a project that fails at run time with no
    # clue why. The refusal names the set it would have accepted, which is the
    # same set `llm list` prints.
    Scenario: An unknown label is refused, and the old default stands
      Given an initialized ctxloom project
      When I run "ctxloom llm default nosuchengine"
      Then the command fails
      And the output contains "unknown LLM"
      And the output contains "claude-code"
      When I run "ctxloom llm default"
      Then the command succeeds
      And the output contains "claude-code"

  @pending
  Rule: create and edit are NOT an upsert, and edit merges per field

    The same split `agent` has, for the same reason and against the same
    shared body: `create` refuses a label that already resolves, `edit` refuses
    one that does not. "Already resolves" is the MERGED view — a registered
    backend name counts, so `llm create claude-code` is refused (it would
    shadow the built-in) while `llm edit claude-code` is accepted (that is how
    a built-in becomes an explicit entry).

    Scenario Outline: A labeled engine config is created and read back out of the project file
      Given an initialized ctxloom project
      When Alice names an engine configuration her team can bind by label:
        """
        ctxloom llm create big --type mock --model o1 <flags>
        """
      Then the command succeeds
      And the output reports "type" as "<names the backend>"
      And the output reports "model" as "<names the model>"
      # The EFFECT: the confirmation is rendered from the request and would
      # print identically over a save that never happened.
      And the file ".ctxloom/config.yaml" contains "big"
      And the file ".ctxloom/config.yaml" contains "o1"
      When I run "ctxloom llm list <flags>"
      Then the output reports "[label=big].label" as "<the label round-trips>"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | names the backend | names the model | the label round-trips |
        |               | mock               | o1               | big                    |
        | --format json | mock               | o1               | big                    |
        | --format text | mock               | o1               | big                    |

    Scenario: Create refuses a label that already resolves, including a built-in backend
      Given an initialized ctxloom project
      And I run "ctxloom llm create big --type mock --model o1"
      When I run "ctxloom llm create big --type mock"
      Then the command fails
      And the output contains "already exists"
      And the output contains "ctxloom llm edit big"
      # The built-in half of "already resolves": shadowing claude-code with a
      # config entry that answers to the same name is the ambiguity the guard
      # exists to refuse.
      When I run "ctxloom llm create claude-code --type mock"
      Then the command fails
      And the output contains "already exists"
      # Neither refusal wrote: the original entry is intact.
      When I run "ctxloom llm list"
      Then the output contains "big"
      And the file ".ctxloom/config.yaml" contains "o1"

    Scenario: Edit refuses a label nothing defines, and creates nothing
      Given an initialized ctxloom project
      When I run "ctxloom llm edit nosuchlabel --model o1-pro"
      Then the command fails
      And the output contains "no llm named"
      And the output contains "ctxloom llm create nosuchlabel"
      When I run "ctxloom llm list"
      Then the command succeeds
      And the output does not contain "nosuchlabel"

    # Every field the invocation did not name survives. A merge that applied
    # the model and reset the type would leave a config pointing the new model
    # at the wrong backend — and would satisfy a model-only assertion.
    Scenario Outline: An edit naming only --model leaves the backend type intact
      Given an initialized ctxloom project
      And I run "ctxloom llm create big --type mock --model o1"
      When Alice moves one label onto a newer model:
        """
        ctxloom llm edit big --model o1-pro <flags>
        """
      Then the command succeeds
      And the output reports "model" as "<the new model>"
      And the output reports "type" as "<the backend survives unmerged>"
      And the file ".ctxloom/config.yaml" contains "o1-pro"
      And the file ".ctxloom/config.yaml" contains "mock"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | the new model | the backend survives unmerged |
        |               | o1-pro         | mock                           |
        | --format json | o1-pro         | mock                           |
        | --format text | o1-pro         | mock                           |

    # --permissions is the launch-time posture, a project-file field distinct
    # from the credential rule below (which is about the machine-scoped env
    # block). Read back out of the project config, not the echo.
    Scenario Outline: Setting the permission posture records it in the project config
      Given an initialized ctxloom project
      And I run "ctxloom llm create big --type mock --model o1"
      When Alice sets the posture this label launches with:
        """
        ctxloom llm edit big --permissions bypass <flags>
        """
      Then the command succeeds
      And the output reports "permissions.mode" as "<the posture>"
      And the file ".ctxloom/config.yaml" contains "mode: bypass"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | the posture |
        |               | bypass       |
        | --format json | bypass       |
        | --format text | bypass       |

  @pending
  Rule: Removing reports before it destroys

    A bare `remove` prints what would go and removes nothing, naming the
    invocation that would — the posture every destroyer in this CLI shares.
    `--yes` takes the entry out of the project file.

    Scenario Outline: Bare remove reports and destroys nothing
      Given an initialized ctxloom project
      And I run "ctxloom llm create big --type mock --model o1"
      When I run "ctxloom llm remove big <flags>"
      Then the command succeeds
      And the output reports "applied" as "<nothing was applied>"
      And the output reports "apply" as "<the command to actually do it>"
      When I run "ctxloom llm list <flags>"
      Then the output reports "[label=big].label" as "<the entry survives>"
      And the file ".ctxloom/config.yaml" contains "big"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | nothing was applied  | the command to actually do it | the entry survives |
        |               | false                 | ctxloom llm remove big --yes  | big                  |
        | --format json | false                 | ctxloom llm remove big --yes  | big                  |
        | --format text | Nothing was removed   | ctxloom llm remove big --yes  | big                  |

    Scenario Outline: --yes takes the entry out of the project file
      Given an initialized ctxloom project
      And I run "ctxloom llm create big --type mock --model o1"
      And the file ".ctxloom/config.yaml" contains "big"
      When Alice retires a label her team no longer binds:
        """
        ctxloom llm remove big --yes <flags>
        """
      Then the command succeeds
      And the output reports "status" as "<confirms the removal>"
      And the file ".ctxloom/config.yaml" does not contain "big"
      When I run "ctxloom llm list <flags>"
      Then the output does not contain "big"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         | confirms the removal |
        |               | removed                |
        | --format json | removed                |
        | --format text | Removed llm            |

    # A built-in backend is not an entry in config.yaml, so there is nothing
    # here to remove — and saying "removed" would be the silent no-op this
    # project is named for.
    Scenario: Removing a label config.yaml never declared is refused
      Given an initialized ctxloom project
      When I run "ctxloom llm remove claude-code"
      Then the command fails
      And the output contains "not defined in config.yaml"
      When I run "ctxloom llm list"
      Then the output contains "claude-code"

  Rule: A config carrying an env: block under a label is refused, not ignored

    A silently-dropped key is the worse outcome: a user who wrote `env:` under
    a label believes their variable reached the engine, and nothing tells them
    otherwise. So the command fails naming the key by its path, label included.

    Scenario: An env: block under a label fails the command and names the key
      Given an initialized ctxloom project
      And the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        llm:
          configs:
            big:
              type: mock
              env:
                OPENAI_API_KEY: sk-acceptance-MUST-NEVER-BE-APPLIED
        """
      When I run "ctxloom llm list"
      Then the command fails
      And the output contains "llm.configs.big.env"

    # There is no flag that would write the key either: a credential has no
    # way into ctxloom's config from the command line, by file or by argv.
    @pending
    Scenario: There is no way to put a credential into an entry
      Given an initialized ctxloom project
      And I run "ctxloom llm create big --type mock --model o1"
      When I run "ctxloom llm edit big --env-file secrets.env"
      Then the command fails
      And the output contains "unknown flag"
