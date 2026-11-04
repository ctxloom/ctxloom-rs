# Messages for the crate's errors. Each id is an error's code, which begins
# with the crate's name; the variables are its args. An id ending in -fix is
# the remedy of the error whose code it follows.

ctxloom-cli-usage = { $ext_err_txt }
ctxloom-cli-usage-fix = run ctxloom -h, or the command with -h, for its usage
ctxloom-cli-unknown-verb = unknown command { $verb } for { $command }: Did you mean one of { $choices }?
ctxloom-cli-unknown-verb-fix = run { $command } -h for its commands
ctxloom-cli-unknown-format = the format { $value } is not one of text or json
ctxloom-cli-unknown-format-fix = use --format json or --format text
ctxloom-cli-output = cannot write the output: { $ext_err_txt }
ctxloom-cli-test-refused = the test command refused, as it always does
ctxloom-cli-test-refused-fix = nothing to fix: ask for the other test command
ctxloom-cli-test-findings = { $count } fatal findings
