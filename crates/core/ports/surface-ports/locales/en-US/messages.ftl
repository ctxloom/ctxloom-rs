# Messages for the crate's errors. Each id is a code, which begins with the
# crate's name; the variables are its args. An id ending in -fix is the remedy
# of the error whose code it follows.

surface-ports-refused = refusing to write { $file }: { $reason }
surface-ports-refused-fix = fix or remove what the reason names, then apply again; the file was left as it was
surface-ports-io = { $file }: { $detail }
surface-ports-record = the ownership record { $file } could not be read: { $detail }
surface-ports-record-fix = ctxloom cannot tell which entries are its own without it: restore the record from a backup, or remove it and release by hand
surface-ports-unsupported = { $engine } cannot carry that out: { $what }
surface-ports-payload = { $engine } hook payload: { $detail }
