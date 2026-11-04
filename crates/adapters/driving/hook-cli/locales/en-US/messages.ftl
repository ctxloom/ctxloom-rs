# Messages for the crate's errors and warnings. Each id is a code, which begins
# with the crate's name; the variables are its args.

hook-cli-unknown-engine = --engine { $engine } names no engine whose hooks ctxloom reads (known: { $known })
hook-cli-unknown-engine-fix = name one of: { $known }
hook-cli-payload = session-bind: harp { $harp }: the session_start payload did not decode, so the harp-to-session bind did not happen: { $detail }
hook-cli-no-usable-harp = session-bind: no usable harp (CTXLOOM_SESSION_HARP="{ $harp }"): this session's transcript carries no harp self-id marker and cannot be attributed to a harp by content
hook-cli-marker-failed = session-bind: harp { $harp }: could not write the harp self-id marker: { $detail }: the transcript carries no owner tag
