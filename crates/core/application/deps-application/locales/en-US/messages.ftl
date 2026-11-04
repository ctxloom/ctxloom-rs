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
deps-application-pull-request-needs-forge = --pr opens a pull request through a forge's API, which this build does not have
deps-application-pull-request-needs-forge-fix = push without --pr, and open the pull request in the forge
deps-application-discover-needs-forge = remote discover searches GitHub through its API, which this build does not have
deps-application-discover-needs-forge-fix = find the repository on GitHub, then `ctxloom remote create <name> <url>`
deps-application-clone-not-refreshed = the repository could not be fetched, so whether this reference is current is unknown: { $url }: { $detail }
deps-application-not-remote-reference = reference has no repository URL: { $reference }
deps-application-unreadable = cannot read { $what }: { $detail }
# The remedy is the fix's; the message names it so that it is an arg of both.
deps-application-notice =
    { $text }{ $remedy ->
       *[other] { "" }
    }
deps-application-notice-fix = { $remedy }
deps-application-notice-unexpanded = could not expand remote parent profile { $what }: { $cause } (its dependencies are preserved from the existing lockfile)
deps-application-notice-unlisted = could not list what { $identity } at { $sha } brings in: { $cause }
