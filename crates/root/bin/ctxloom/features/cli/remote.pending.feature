# Ported from cli/remote.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   remote.feature with its steps (gherkin_lint.test.rs).

@doc
@pending
Feature: remote — registering the sources content comes from, and browsing them

  Covers: `ctxloom remote create`, `remote edit`, `remote list`, `remote show`, `remote
  default`, `remote remove`, and the bare `ctxloom remote` form.

  A remote is an ADDRESS. Registering one records it in
  `.ctxloom/remotes.yaml` — which repositories this project may draw shared
  bundles from, and which of them is the default — and clones it into the
  cache, so a wrong URL or missing access shows up then rather than on first
  use. A clone that fails is a warning, not a refusal: the remote stays
  registered. Nothing is installed and no credential is stored.

  Registering a remote is the trust act: its content reaches the assistant
  because the project added it. Registering a remote is also the consent to
  PUBLISH to it: `ctxloom bundle push` writes to a registered remote and asks for no
  second blessing, because naming the destination was the deliberate act.

  What this project has INSTALLED from these remotes is a different question
  and a different noun — see cli/deps.feature.

  This is the comprehensive per-noun spec: what the noun DOES, leaf by leaf,
  hermetically against a seeded file:// repository.

  Rule: The registry records the address, and nothing is installed

    Registering, defaulting, and removing a remote change `.ctxloom/remotes.yaml`;
    registering also clones the repository into the cache. None of them
    installs a bundle.

    Scenario: Registering a remote records it and it is listed back
      Given an initialized ctxloom project
      When Alice registers a remote:
        """
        ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git
        """
      Then the command succeeds
      And the file ".ctxloom/remotes.yaml" contains "origin"
      And the file ".ctxloom/remotes.yaml" contains "forge: git"
      When I run "ctxloom remote list"
      Then the command succeeds
      And the output contains "origin"

    # One repository, one remote. A second name for an address already
    # registered would split the bundles drawn from it across two remotes
    # that nothing ties together.
    Scenario: Registering an address that is already a remote is refused
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When Alice registers the same repository under another name:
        """
        ctxloom remote create mirror file:///tmp/acceptance-remote.git --forge git
        """
      Then the command fails
      And the output contains "origin"
      And the file ".ctxloom/remotes.yaml" does not contain "mirror"

    # A forge label names an adapter; a typo must surface when it is bound,
    # not later as a silent fall back to resolving by URL host.
    Scenario: Registering a remote bound to an unknown forge is refused
      Given an initialized ctxloom project
      When Alice registers a remote bound to a forge nobody configured:
        """
        ctxloom remote create origin file:///tmp/acceptance-remote.git --forge no-such-forge
        """
      Then the command fails
      And the output contains "no-such-forge"
      And the file ".ctxloom/remotes.yaml" does not contain "no-such-forge"

    Scenario: Remotes are listed in name order
      Given an initialized ctxloom project
      And I run "ctxloom remote create zeta file:///tmp/acceptance-zeta.git --forge git"
      And I run "ctxloom remote create alpha file:///tmp/acceptance-alpha.git --forge git"
      When I run "ctxloom remote list --format text"
      Then the command succeeds
      And the output matches "(?s)alpha.*zeta"

    # The bare noun answers the question somebody typing it has, rather than
    # teaching them what they could have typed instead.
    Scenario: Bare remote lists the registry
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When I run "ctxloom remote"
      Then the command succeeds
      And the output contains "origin"
      And the output does not contain "Available Commands:"

    Scenario: A remote can be made the default
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When Alice sets the default remote:
        """
        ctxloom remote default origin
        """
      Then the command succeeds
      And the file ".ctxloom/remotes.yaml" contains "default: origin"

    # Bare `remove` is a preview: it must leave the remote registered. A guard
    # that quietly destroyed anyway would still pass a scenario that only
    # checked exit code — the follow-up `remote list` is what actually
    # catches that.
    Scenario Outline: Bare remote remove reports and destroys nothing
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When I run "ctxloom remote remove origin <flags>"
      Then the command succeeds
      And the output reports "applied" as "<reports nothing removed>"
      And the output reports "apply" as "<names the apply command>"
      When I run "ctxloom remote list"
      Then the output contains "origin"

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         | reports nothing removed | names the apply command |
      |               | false                   | ctxloom remote remove origin --yes |
      | --format json | false                   | ctxloom remote remove origin --yes |
      | --format text | Nothing was removed     | ctxloom remote remove origin --yes |


    Scenario: Removing a remote takes it out of the registry
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When Alice removes the remote:
        """
        ctxloom remote remove origin --yes
        """
      Then the command succeeds
      When I run "ctxloom remote list"
      Then the output does not contain "origin"

  Rule: A registered remote can be corrected in place

    An address changes — a repository moves host, a forge is misresolved, a
    name stops describing what it points at. Editing is the same local
    bookkeeping as registering: content already installed is untouched,
    because each lockfile entry records the URL it came from rather than
    naming a remote, and trust is unaffected because a remote never carried
    any.

    Scenario: Correcting a remote's URL records the new address and drops the old
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When Alice corrects the remote's address:
        """
        ctxloom remote edit origin --url file:///tmp/acceptance-moved.git
        """
      Then the command succeeds
      And the file ".ctxloom/remotes.yaml" contains "acceptance-moved.git"
      And the file ".ctxloom/remotes.yaml" does not contain "acceptance-remote.git"

    # Restating a remote's own address, or its own name, is not a collision
    # with itself.
    Scenario: An edit restating a remote's own name and address is accepted
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When Alice restates the remote as it already stands:
        """
        ctxloom remote edit origin --name origin --url file:///tmp/acceptance-remote.git
        """
      Then the command succeeds
      And the file ".ctxloom/remotes.yaml" contains "origin"

    Scenario: Moving a remote onto another remote's address is refused
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      And I run "ctxloom remote create mirror file:///tmp/acceptance-mirror.git --forge git"
      When Alice points the mirror at the address origin already holds:
        """
        ctxloom remote edit mirror --url file:///tmp/acceptance-remote.git
        """
      Then the command fails
      And the output contains "origin"
      And the file ".ctxloom/remotes.yaml" contains "acceptance-mirror.git"

    Scenario: Rebinding a remote to an unknown forge is refused
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When Alice rebinds the remote to a forge nobody configured:
        """
        ctxloom remote edit origin --forge no-such-forge
        """
      Then the command fails
      And the file ".ctxloom/remotes.yaml" contains "forge: git"
      And the file ".ctxloom/remotes.yaml" does not contain "no-such-forge"

    # --forge rebinds the adapter a remote resolves through, independent of
    # its URL. Created bound to "git", then rebound to "github" — the new
    # label read back out of the registry is the only honest proof the
    # rebind landed rather than the create-time binding just sitting there.
    Scenario: Rebinding a remote's forge records the new binding
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When Alice rebinds the remote to a different forge:
        """
        ctxloom remote edit origin --forge github
        """
      Then the command succeeds
      And the file ".ctxloom/remotes.yaml" contains "forge: github"

    # The rename must carry the default with it. The pointer is stored BY NAME,
    # so a rename that ignored it would leave `default:` naming a remote that
    # no longer exists — and every bare command reaching for a default would
    # find nothing, with the registry looking perfectly well-formed.
    Scenario: Renaming the default remote carries the default with it
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      And I run "ctxloom remote default origin"
      When Alice renames the remote:
        """
        ctxloom remote edit origin --name upstream
        """
      Then the command succeeds
      And the file ".ctxloom/remotes.yaml" contains "default: upstream"
      And the file ".ctxloom/remotes.yaml" does not contain "default: origin"
      When I run "ctxloom remote list"
      Then the output contains "upstream"
      And the output does not contain "origin"

    # An edit naming no field is a refusal, not a success. Reporting exit 0
    # over an untouched registry is this project's characteristic bug.
    Scenario: An edit that asks for nothing is refused
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When I run "ctxloom remote edit origin"
      Then the command fails
      And the file ".ctxloom/remotes.yaml" contains "origin"

  Rule: A remote's catalog can be browsed without installing anything

    `remote show` and the equivalent MCP resource both read a remote's
    published bundles without touching any profile or lockfile — browsing is
    not installing.

    Scenario: Browsing a remote lists the bundles it publishes
      Given an initialized ctxloom project
      And a git remote "origin" serving a ctxloom bundle
      When Alice browses what a remote publishes:
        """
        ctxloom remote show origin --format text
        """
      Then the command succeeds
      And the output contains "//bundles/demo"


  Rule: A registry written by hand is read as written, and rewritten without loss

    `.ctxloom/remotes.yaml` is a file people edit and commit. Configured forge
    instances live only there (`forges:`), and a key ctxloom does not manage
    is the author's, not ctxloom's to drop: every `remote` command rewrites
    the whole file, so whatever survives one rewrite is what the file keeps.

    Scenario: A hand-configured forge can be bound, and survives the rewrite
      Given an initialized ctxloom project
      And the project already has the file ".ctxloom/remotes.yaml":
        """
        schema_version: 1
        forges:
          corp:
            type: git
        remotes:
          origin:
            url: file:///tmp/acceptance-remote.git
        note: written by hand
        """
      When Alice registers a remote bound to the forge her team configured:
        """
        ctxloom remote create mirror file:///tmp/acceptance-mirror.git --forge corp
        """
      Then the command succeeds
      And the file ".ctxloom/remotes.yaml" matches "(?m)^forges:\n\s+corp:"
      And the file ".ctxloom/remotes.yaml" contains "note: written by hand"
      When I run "ctxloom remote list"
      Then the output contains "origin"
      And the output contains "mirror"

    # A registry that declares no schema generation is refused, naming the
    # generation it found and the oldest this build reads, rather than guessed at.
    Scenario: A registry that declares no schema generation is refused
      Given an initialized ctxloom project
      And the project already has the file ".ctxloom/remotes.yaml":
        """
        remotes:
          origin:
            url: file:///tmp/acceptance-remote.git
        """
      When I run "ctxloom remote list"
      Then the command fails
      And the output contains "schema_version 0"

  Rule: A command finds the registry from wherever it runs

    Scenario: A remote registered at the root is listed from a subdirectory
      Given an initialized ctxloom project
      And I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      When I run "ctxloom remote list" from the project subdirectory "docs"
      Then the command succeeds
      And the output contains "origin"

    # Outside any project a remote is registered in the home layer, and on a
    # machine ctxloom has never run on, that registration creates the home
    # .ctxloom directory — which its owner must then be able to use.
    Scenario: Registering a first remote outside any project creates a usable home registry
      Given an empty project directory
      And the home has no ".ctxloom" directory yet
      When I run "ctxloom remote create origin file:///tmp/acceptance-remote.git --forge git"
      Then the command succeeds
      And the home directory ".ctxloom" is readable, writable and searchable by its owner
      And the home file ".ctxloom/remotes.yaml" contains "origin"
