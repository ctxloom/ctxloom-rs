# Messages for the crate's errors. Each id is a code, which begins with the
# crate's name; the variables are its args. An id ending -fix is the remedy of the one before.

tasks-ports-not-found = { $program } was not found, so the Deferred tasks cannot be read
tasks-ports-not-found-fix = install taskloom, put it on PATH, or set TASKLOOM_BIN to its path
tasks-ports-unrunnable = { $program } could not be run: { $why }
tasks-ports-refused = { $program } refused ({ $exit }): { $cause }
tasks-ports-failed = { $program } failed ({ $exit }): { $stderr }
tasks-ports-unparsable = { $program } printed something that is not a task list: { $why }
tasks-ports-timed-out = { $program } did not finish within { $timeout } and was killed
tasks-ports-timed-out-fix = run taskloom by hand to see what it waits on (a locked or unreachable task store), then run again
