# Messages for the crate's errors. Each id is an error's code, which begins with
# the crate's name; the variables are its args.

sessions-session-exists = session { $harp } already exists
sessions-no-session = there is no such session
sessions-forgotten = session { $harp } was removed
sessions-harp-invalid = { $harp } cannot be a session name: { $reason }
sessions-harp-too-long = { $harp } is { $length } characters, the limit for a session name is { $limit }
sessions-names-taken = the session name { $harp } is taken
sessions-no-claim = nobody holds that session name
sessions-not-holder = this session does not hold the name { $harp }
sessions-child-cannot-stop = a child (depth { $depth }) cannot order a stop
sessions-unknown-message = no message { $message_id } in the inbox
sessions-already-delivered = message { $message_id } was already delivered
