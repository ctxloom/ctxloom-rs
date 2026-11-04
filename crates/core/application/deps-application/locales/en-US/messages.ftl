# Messages for the crate's errors and notices. Each id is a code, which begins
# with the crate's name; the variables are its args. An id ending in -fix is
# the remedy of the error whose code it follows.

deps-application-file = cannot access { $what }: { $detail }
deps-application-required = { $what } is required
deps-application-not-registered = remote not registered: { $url } is reached only by reference; register it first: ctxloom remote create <name> { $url }
deps-application-conflict = { $text }
deps-application-conflict-fix = make the profiles agree on one commit, or pin the item the same way everywhere
deps-application-pull-failed = deps pull: { $count } failed ({ $refs }); check network/auth and retry (ctxloom deps pull), or drop the reference from its profile
deps-application-browse = browse { $remote }: { $detail }
deps-application-no-remote-named = no remote named "{ $name }"
deps-application-no-remote-named-fix = `ctxloom remote list` names the registered remotes
deps-application-default-needs-name = remote name required (or use --clear); see the current default in 'ctxloom remote list'
deps-application-nothing-to-hold = "{ $name }" is neither a local bundle nor an entry in the active lockfile, so there was nothing to { $verb } — run `ctxloom deps pull` to lock it, or `ctxloom deps list` to check the name
deps-application-bundle-not-found = bundle "{ $name }" not found: bundle not found: { $name }
deps-application-bundle-not-found-fix = `ctxloom bundle list` names the bundles of this project
deps-application-bad-destination = destination "{ $to }" is neither a configured remote nor an existing directory{ $known }
deps-application-destination-exists = bundle already exists at destination: { $dest } (use --force to overwrite)
deps-application-destination-itself = destination is the bundle's own directory: { $dest }
deps-application-publish = cannot publish to { $remote }: { $detail }
deps-application-publish-fix = check the remote's address and your git credentials, then retry
deps-application-pull-request-needs-forge = remote { $remote } is on the { $forge } forge, which has no API to open a pull request through
deps-application-pull-request-needs-forge-fix = bind it to GitHub with `ctxloom remote edit { $remote } --forge github`, or push without --pr and open the pull request in the forge
deps-application-discover-source = unsupported discovery source "{ $source }": only "github" is searchable
deps-application-discover-source-fix = leave --source out, or give github
deps-application-discover-failed = repository search failed (see warning(s) above); no results could be retrieved
deps-application-discover-failed-fix = check the network, and GH_TOKEN or GITHUB_TOKEN if GitHub is rate limiting you, then retry
deps-application-empty-file = refusing to publish empty file { $path }: a 0-byte file would overwrite the remote with nothing
deps-application-empty-file-fix = put content in it or remove it from the bundle, then push again
deps-application-clone-not-refreshed = the repository could not be fetched, so whether this reference is current is unknown: { $url }: { $detail }
deps-application-not-remote-reference = reference has no repository URL: { $reference }
deps-application-refresh-aborted = aborting project refresh: { $detail }
deps-application-refresh-aborted-fix = the pull itself landed and wrote nothing into the project; fix what is named, then run `ctxloom deps pull --yes` again
deps-application-unreadable = cannot read { $what }: { $detail }
# The remedy is the fix's; the message names it so that it is an arg of both.
deps-application-notice =
    { $text }{ $remedy ->
       *[other] { "" }
    }
deps-application-notice-fix = { $remedy }
deps-application-notice-unexpanded = could not expand remote parent profile { $what }: { $cause } (its dependencies are preserved from the existing lockfile)
deps-application-notice-unlisted = could not list what { $identity } at { $sha } brings in: { $cause }
