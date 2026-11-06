# The fixture gate, as a justfile fragment consumers copy (to
# gates/fixture-gates.just, or beside their justfile) and import:
#
#     import 'gates/fixture-gates.just'
#
# The importing justfile defines `TOP` (the repository's top level) and has
# scripts/exec-mode-check.sh and scripts/exec-mode-allow.txt from this
# repository's scripts/ (copied; the copies are independent). Make `lint`
# depend on `lint-fixtures`. The self-test (scripts/exec-mode-check.selftest.sh)
# stays here.

# No test gives a file an executable mode to run it: a fake program is
# fixture_program::install_program, which never execs a file a test wrote
# (ETXTBSY). The allowlist (scripts/exec-mode-allow.txt) is the modes that
# are the point of a test.
[group('check')]
[doc("No test gives a file an executable mode to run it")]
[no-exit-message]
lint-fixtures:
    @{{TOP}}/scripts/exec-mode-check.sh
