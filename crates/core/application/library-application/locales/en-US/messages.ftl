# Messages for the crate's errors and notices. Each id is a code, which begins
# with the crate's name; the variables are its args. An id ending in -fix is
# the remedy of the error whose code it follows.

library-application-config-unparsable = failed to load config: config layer cannot be parsed: { $path }: yaml: { $cause }
library-application-config-unknown-key = failed to load config: the config breaks its schema:{ $violations }
library-application-config-unknown-key-fix = remove or rename the key in config.yaml (ctxloom config edit)
library-application-config-breaks-schema = failed to load config: the config breaks its schema:{ $violations }
library-application-config-breaks-schema-fix = give the key a value the schema accepts, or remove it (ctxloom config edit)
library-application-unknown-section =
    unknown section: { $section }

    Available: { $available }
library-application-bundle-unreadable = bundle unreadable: { $ask }: { $cause }
library-application-bundle-unreadable-fix = fix or remove the bundle file
library-application-no-bundles-dir = no bundles directory found
library-application-view-path = invalid path format: { $cause } (expected fragments, commands, mcp, skills, or profiles)
library-application-view-missing = { $kind } not found: { $name }
library-application-not-viewable = { $dir } items are not viewable: { $path }
library-application-item-missing =
    { $kind } "{ $name }": item not found

    Available { $kind }s: { $available }
library-application-bad-reference = invalid reference format: expected { $expected } (got "{ $got }")
library-application-wrong-kind = "{ $ask }" selects a { $got }/, not a { $want }/ (expected { $expected })
library-application-unsupported-format = { $command }: --format { $format } was accepted but this command does not support it yet (nothing was rendered through it)
library-application-unsupported-format-fix = rerun without --format
library-application-skill-not-found = skill not found: "{ $name }" in bundle "{ $bundle }"
library-application-skill-name-not-found = skill not found: { $name }
library-application-no-bundle-named = no bundle named "{ $name }" (bundles: { $known })
library-application-search-nothing = please provide a search query or tags — e.g. ctxloom search cache, or ctxloom search --tag golang
library-application-search-type = unknown type "{ $value }" (valid: fragment, command, skill, profile, bundle, mcp_server) — see 'ctxloom search --help'
library-application-search-remote = search --remote reads the remote registry, which this build does not read yet
library-application-search-remote-fix = search the project's own content with --local
library-application-agent-not-found = agent "{ $name }" not found
library-application-launch-no-agent = launch: the named agent is not found: "{ $name }"
library-application-launch-no-agent-fix = create it with `ctxloom agent create <name> --profiles <profile>` (`ctxloom profile list` names the profiles)
library-application-launch-no-project = launch: the named agent is not found: no project here, so no default agent
library-application-launch-no-project-fix = run `ctxloom init` here to set up a project and its default agent
library-application-launch-no-default = launch: the named agent is not found: no default agent is set
library-application-launch-no-default-fix = choose one with `ctxloom agent default <name>` (`ctxloom agent list` names them)
library-application-launch-no-agents = launch: the named agent is not found: no agents are configured
library-application-launch-no-agents-fix = create it with `ctxloom agent create <name> --profiles <profile>` (`ctxloom profile list` names the profiles)
library-application-mcp-not-found = MCP server "{ $name }" not found
library-application-unknown-llm = unknown LLM "{ $label }"; usable now: { $usable }
library-application-compose = assemble context: profile { $profile }: { $cause }
library-application-no-fragments = no fragments loaded: { $detail }
library-application-agent-exclusive = if any flags in the group [agent { $other }] are set none of the others can be; [agent { $other }] were all set
library-application-saved-command = failed to load command: command not found: { $name }
library-application-workspace = invalid workspace "{ $value }": workspace is one of none or worktree
library-application-run-needs-dry-run = run launches the engine, which this build does not do yet: run it with --dry-run to see what a launch would carry
library-application-startup-aborted = aborting startup: { $count } fatal finding(s); fix them, or rerun with --degraded (env CTXLOOM_DEGRADED=1) to launch anyway:{ $findings }

# The remedy is the fix's; the message names it so that it is an arg of both.
library-application-notice-finding =
    { $text }{ $remedy ->
       *[other] { "" }
    }
library-application-notice-finding-fix = { $remedy }
library-application-notice-bundle-skipped = skipping bundle { $manifest }: bundles: reading the tree at { $dir }: { $cause }
library-application-notice-bundle-skipped-fix = fix or remove the bundle file
library-application-config-newer = { $path }: ctxloom config: schema_version { $found }: written by a newer format than this binary reads; upgrade the binary (this binary reads up to { $current })
library-application-config-newer-fix = upgrade ctxloom: { $path } was written by a newer one
library-application-config-older = { $path }: ctxloom config: schema_version { $found }: older than the oldest format this binary can migrate (the oldest is { $oldest })
library-application-config-older-fix = back up { $path }, then re-run `ctxloom init` to scaffold a current one and re-apply your settings
library-application-config-version-unreadable = { $path }: ctxloom config: format version cannot be read: schema_version is not an integer
library-application-config-version-unreadable-fix = back up { $path }, then re-run `ctxloom init` to scaffold a current one and re-apply your settings
library-application-notice-config-keyless = config file { $path } defines no keys
library-application-notice-root-ignored = CTXLOOM_ROOT={ $root } is not a directory; ignoring it
