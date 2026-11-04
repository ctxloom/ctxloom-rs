# Messages for the crate's errors and refusals. Each id is a code, which begins
# with the crate's name; the variables are its args. A refusal says what was not done and the command that
# would, in its message, so the json envelope carries it too. An id ending -fix
# is the remedy of the one before.

sessions-application-no-session = no session named { $harp }
sessions-application-no-session-fix = list the sessions with: ctxloom session list --all
sessions-application-no-captured-transcript = session { $harp }: no captured transcript
sessions-application-gave-up = gave up after { $attempts } attempts: { $error }
sessions-application-refused-not-dead = refusing to remove { $harp }: it cannot be proven dead ({ $reason }). Nothing was removed. To destroy it anyway, deliberately: ctxloom { $verb } { $harp } --yes --even-if-live
sessions-application-refused-kept = refusing to remove { $harp }: it is marked keep, so its transcript is kept at any age. Nothing was removed. To release it: ctxloom session keep { $harp } --undo
sessions-application-refused-retained = refusing to destroy { $harp }'s transcript, its record: the session ended within the transcript retention period ({ $detail }). Nothing was removed. It becomes reclaimable once the session ended more than session_transcript_retention ago.
sessions-application-removed-nothing = removed nothing: this was a report, not a removal. To apply exactly this plan: { $command }
sessions-application-go-sidecar-too-new = Go's session record was written by a newer ctxloom (generation { $found }; this one reads up to { $current })
sessions-application-go-sidecar-too-old = Go's session record is older than any this ctxloom reads (generation { $found }; the oldest is { $oldest })
sessions-application-go-sidecar-instant = Go's session record has { $value } for { $field }, which is not a date and time
sessions-application-go-sidecar-unreadable = Go's session record cannot be read: { $error }
sessions-application-left-no-project = left: its project directory { $project_dir } is the work root of no project here, so there is no store to import it into
sessions-application-left-name-taken = left: a session of the project already holds the name { $harp }
