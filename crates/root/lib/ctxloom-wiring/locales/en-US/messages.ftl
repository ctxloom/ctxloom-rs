# Messages for the crate's errors. Each id is an error's code, which begins
# with the crate's name; the variables are its args.

ctxloom-wiring-no-home = there is no home directory: { $variable } is not set
ctxloom-wiring-bad-session = { $session } is not a session id: { $ext_err_txt }
ctxloom-wiring-no-runtime = cannot start a runtime to open the store: { $ext_err_txt }
ctxloom-wiring-store = cannot open the project store: { $error }
ctxloom-wiring-seed = the test seed could not plant the session: { $ext_err_txt }
