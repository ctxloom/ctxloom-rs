# Ported from cli/version.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   version.feature with its steps (gherkin_lint.test.rs).

@doc
@pending
Feature: version — the one question a build must always be able to answer

  Covers: `ctxloom version`, `ctxloom --format json version`, and the
  `ctxloom --version` flag.

  A trivial surface with one non-trivial obligation: THREE SPELLINGS, ONE
  ANSWER. A person types `--version`, a script parses `--format json`, and a
  bug report quotes `ctxloom version` — and the moment those disagree, every
  version-dependent conversation (which build has the fix, what does the
  changelog apply to, is this the binary I just installed) is being had about
  the wrong build. Nothing else in the CLI depends on this, and everything
  people say about the CLI does.

  Rule: Every spelling reports the same version-shaped string

    # The version this binary was stamped with is not knowable to the suite —
    # the CLI is stamped by ldflags at build time and the test process is not —
    # so the assertion is the strongest one that IS available: the printed
    # string is version-SHAPED, and every row agrees with `ctxloom --version`,
    # which never goes through cobrafmt.Resolve and so is the fixed point.
    #
    # This replaces `the output matches "."` — one arbitrary character, which
    # the literal "MUTATION-not-the-version" satisfies exactly as well as the
    # truth does, and which a build stamping an empty string would fail only by
    # accident.
    #
    # The no-flag row is the important one: off a terminal (which this harness
    # always is) `ctxloom version` now resolves to the SAME JSON the
    # `--format json` row gets, per cobrafmt.Resolve's derived default. Only an
    # explicit `--format text` still gets the prose rendering.
    Scenario Outline: The version reads the same however it is asked for
      Given an initialized ctxloom project
      Then the version reported for "<flags>" agrees with "ctxloom --version"

      @json-default
      Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
        | flags         |
        |               |
        | --format json |
        | --format text |
