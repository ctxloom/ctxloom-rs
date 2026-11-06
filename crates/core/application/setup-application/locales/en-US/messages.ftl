# Messages for the crate's errors. Each id is a code, which begins with the
# crate's name; the variables are its args. An id ending in -fix is the remedy
# of the error whose code it follows.

setup-application-no-target = no --target given: this would write { $engines }'s files into the project directory { $dir }
setup-application-no-target-fix = pass --target <dir> to write elsewhere, or --yes to write the project directory
setup-application-target-not-directory = materialize: target { $path } exists and is not a directory
setup-application-request = invalid request: { $detail }
setup-application-unknown-engine = unknown engine "{ $engine }"; ctxloom knows: { $known }
setup-application-engine-not-applied = --engine { $engine } cannot be applied: .ctxloom already exists, and the engine is only recorded while scaffolding it
setup-application-engine-not-applied-fix = change the default engine with `ctxloom llm default <name>` (or remove .ctxloom to re-scaffold)
setup-application-empty-context = empty context: profile set { $profiles } assembled to nothing, so materializing { $engine } is refused (check that the profile's fragments and bundles resolve)
setup-application-refused = materialize: refused over fatal findings raised while assembling; nothing was written: { $findings }
setup-application-refused-fix = fix what the findings name, or pass --degraded to waive them
setup-application-unknown-hook-engine = --engine { $engine } names no engine whose hooks ctxloom reads (known: { $known })
setup-application-unknown-hook-engine-fix = name one of: { $known }
setup-application-unknown-event = unknown hook event "{ $event }"; the lifecycle events are { $known }
setup-application-config-write = config-write: { $detail }
setup-application-diff-file-missing = materialize --diff: { $path } does not exist, so there is nothing to compare against
setup-application-diff-file-missing-fix = name the delivered file to compare with, e.g. another checkout's CLAUDE.md
setup-application-diff-file-unreadable = materialize --diff: could not read { $path } to compare against: { $detail }
