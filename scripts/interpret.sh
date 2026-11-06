#!/bin/sh
# A test fixture's program, run by the interpreter and never exec'd as written.
#
# A fixture links its program (`git`, `docker`, `taskloom`...) to this file and
# writes the script beside it as `<program>.body`. Running the program runs
# this file (never written by a test, so never open for writing), and this
# file runs the body with `sh`, which only reads it.
#
# Exec'ing a file a test has just written is racy in a multithreaded test
# process: a thread that forks while the file is open for writing gives its
# child a copy of the descriptor until that child execs, and until then the
# exec fails with "text file busy" (ETXTBSY) though the file is closed.
exec /bin/sh "$0.body" "$@"
