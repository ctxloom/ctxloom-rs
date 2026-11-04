# Messages for the crate's errors. Each id is a code, which begins with the
# crate's name; the variables are its args. An id ending -fix is the remedy of the one before.

container-cli-help-not-payload = help is not a payload: ctxloom container has no machine-readable form, so --format { $format } cannot be honoured
container-cli-help-not-payload-fix = name a verb: check, build, prune, scaffold or tooling
container-cli-output = cannot write the output: { $ext_err_txt }
container-cli-no-such-command = { $noun } is not a command of this crate
