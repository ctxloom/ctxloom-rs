//! The library commands' shape: nouns and verbs, flags and arguments.

use clap::{Arg, ArgAction, Command};

/// A noun whose bare form is its default verb: no help is demanded of it.
fn noun(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .disable_help_subcommand(true)
        .subcommand_required(false)
        .arg_required_else_help(false)
}

fn list(about: &'static str) -> Command {
    Command::new("list").visible_alias("ls").about(about)
}

fn bundle_filter() -> Arg {
    Arg::new("bundle")
        .long("bundle")
        .value_name("BUNDLE")
        .help("Only this bundle's")
}

fn distilled(help: &'static str) -> Arg {
    Arg::new("distilled")
        .short('d')
        .long("distilled")
        .action(ArgAction::SetTrue)
        .help(help)
}

fn one(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .value_name(name.to_uppercase())
        .required(true)
        .help(help)
}

fn yes() -> Arg {
    Arg::new("yes")
        .short('y')
        .long("yes")
        .action(ArgAction::SetTrue)
        .help("Apply the removal this invocation would report (default: report only)")
}

fn remove(about: &'static str) -> Command {
    Command::new("remove")
        .visible_aliases(["rm", "del"])
        .about(about)
        .arg(yes())
}

fn description(help: &'static str) -> Arg {
    Arg::new("description")
        .short('d')
        .long("description")
        .value_name("TEXT")
        .help(help)
}

fn force(help: &'static str) -> Arg {
    Arg::new("force")
        .short('f')
        .long("force")
        .action(ArgAction::SetTrue)
        .help(help)
}

/// A repeatable flag that also takes a comma-separated list.
fn list_flag(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .value_delimiter(',')
        .action(ArgAction::Append)
        .help(help)
}

/// A repeatable flag whose every occurrence is one value (an empty one
/// clears), a value that starts with `-` allowed.
fn each(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::Append)
        .allow_hyphen_values(true)
        .help(help)
}

/// A flag whose being named is the point, an empty value included.
fn named(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .value_name(name.to_uppercase())
        .allow_hyphen_values(true)
        .help(help)
}

/// Every noun the library adds to the command line.
pub fn nouns() -> Vec<Command> {
    vec![
        config(),
        bundle(),
        fragment(),
        command(),
        skill(),
        profile(),
        agent(),
        llm(),
        mcp(),
        companion(),
        search(),
        context(),
        run(),
        loadout(),
    ]
}

fn config() -> Command {
    noun("config", "Show ctxloom configuration")
        .subcommand(Command::new("edit").about("Open config.yaml in $EDITOR"))
        .subcommand(
            Command::new("create")
                .about("Scaffold a default config.yaml (and remotes.yaml)")
                .arg(
                    Arg::new("engine")
                        .long("engine")
                        .value_name("ENGINE")
                        .help("AI engine to record in the scaffolded config"),
                ),
        )
        .subcommand(
        Command::new("show")
            .about("Show the effective configuration, or one section")
            .arg(Arg::new("section").value_name("SECTION"))
            .arg(
                Arg::new("raw")
                    .long("raw")
                    .action(ArgAction::SetTrue)
                    .help("Show only what the configuration sets, without the shipped default engine registry"),
            ),
    )
}

fn bundle() -> Command {
    noun("bundle", "Inspect ctxloom bundles")
        .subcommand(list("List installed bundles"))
        .subcommand(
            Command::new("show")
                .about("Show bundle contents")
                .arg(one("name", "The bundle")),
        )
        .subcommand(
            Command::new("view")
                .about("View bundle content, optionally one item")
                .arg(one("name", "The bundle, or bundle#kind/name"))
                .arg(distilled("Show the distilled version if there is one")),
        )
        .subcommand(
            Command::new("create")
                .about("Create a new bundle")
                .arg(one("name", "The new bundle"))
                .arg(description("Bundle description")),
        )
        .subcommand(
            Command::new("edit")
                .about("Edit a bundle")
                .arg(one("name", "The bundle"))
                .arg(description("New description"))
                .arg(named("version", "New version"))
                .arg(list_flag("add-tag", "Tag(s) to add"))
                .arg(list_flag("remove-tag", "Tag(s) to remove"))
                .arg(list_flag("add-fragment", "Fragment(s) to add"))
                .arg(list_flag("remove-fragment", "Fragment(s) to remove"))
                .arg(list_flag("add-prompt", "Prompt(s) to add"))
                .arg(list_flag("remove-prompt", "Prompt(s) to remove"))
                .arg(list_flag("add-mcp", "MCP server(s) to add"))
                .arg(list_flag("remove-mcp", "MCP server(s) to remove")),
        )
        .subcommand(remove("Remove a bundle").arg(one("name", "The bundle")))
        .subcommand(
            Command::new("export")
                .about("Export a bundle to a directory")
                .arg(one("name", "The bundle"))
                .arg(
                    Arg::new("dest")
                        .value_name("DEST-DIR")
                        .help("Where the bundle's directory lands"),
                )
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .value_name("PATH")
                        .help("Destination path of the exported tree"),
                ),
        )
        .subcommand(
            Command::new("import")
                .about("Import a bundle from a local directory")
                .arg(one(
                    "path",
                    "A bundle tree: its directory, or its bundle.yaml",
                ))
                .arg(force("Overwrite existing bundle")),
        )
}

/// `create`, `remove` and `edit` of an item noun.
fn item_writes(noun: Command, word: &'static str, dir: &'static str) -> Command {
    noun.subcommand(
        Command::new("create")
            .about(match word {
                "fragment" => "Create a new fragment",
                _ => "Create a new command",
            })
            .arg(one("bundle", "The bundle"))
            .arg(one("name", "The new item's name")),
    )
    .subcommand(
        remove(match word {
            "fragment" => "Remove a fragment",
            _ => "Remove a command",
        })
        .arg(one(
            "ref",
            match dir {
                "fragments" => "bundle#fragments/name",
                _ => "bundle#commands/name",
            },
        )),
    )
    .subcommand(
        Command::new("edit")
            .about(match word {
                "fragment" => "Edit a fragment's content in your editor",
                _ => "Edit a command's content in your editor",
            })
            .arg(one(
                "ref",
                match dir {
                    "fragments" => "bundle#fragments/name",
                    _ => "bundle#commands/name",
                },
            ))
            .arg(
                Arg::new("no-distill")
                    .long("no-distill")
                    .action(ArgAction::SetTrue)
                    .help("Skip re-distillation for this edit"),
            ),
    )
}

fn fragment() -> Command {
    item_writes(fragment_reads(), "fragment", "fragments")
}

fn fragment_reads() -> Command {
    noun("fragment", "Inspect context fragments")
        .subcommand(list("List all fragments").arg(bundle_filter()))
        .subcommand(
            Command::new("show")
                .about("Show fragment content")
                .arg(one("ref", "bundle#fragments/name"))
                .arg(distilled("Show the distilled version if there is one")),
        )
        .subcommand(
            Command::new("premises")
                .about("List conditionally-loaded fragments and the premise each applies under"),
        )
}

fn command() -> Command {
    item_writes(command_reads(), "command", "commands")
}

fn command_reads() -> Command {
    noun("command", "Inspect commands")
        .subcommand(list("List all commands").arg(bundle_filter()))
        .subcommand(
            Command::new("show")
                .about("Show command content")
                .arg(one("ref", "bundle#commands/name"))
                .arg(distilled("Show the distilled version if there is one")),
        )
}

fn skill() -> Command {
    noun("skill", "Inspect Agent Skills (SKILL.md packages)")
        .subcommand(list("List all Agent Skill packages").arg(bundle_filter()))
        .subcommand(
            Command::new("show")
                .about("Show a skill's frontmatter and file manifest")
                .arg(one("ref", "bundle#skills/name")),
        )
        .subcommand(
            Command::new("create")
                .about("Scaffold a new Agent Skill package in a bundle")
                .arg(one("bundle", "The bundle"))
                .arg(one("name", "The new skill's name"))
                .arg(description(
                    "SKILL.md frontmatter description (default: a TODO placeholder)",
                )),
        )
        .subcommand(remove("Remove a skill package").arg(one("ref", "bundle#skills/name")))
        .subcommand(
            Command::new("export")
                .about("Pack a skill into a .zip")
                .arg(one("ref", "bundle#skills/name"))
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .value_name("PATH")
                        .help("Output .zip path (default: <name>.zip)"),
                ),
        )
        .subcommand(
            Command::new("import")
                .about("Import a skill archive into a bundle")
                .arg(one("archive", "A skill .zip"))
                .arg(
                    Arg::new("bundle")
                        .long("bundle")
                        .value_name("BUNDLE")
                        .required(true)
                        .help("Target bundle to import into"),
                )
                .arg(
                    Arg::new("force")
                        .long("force")
                        .action(ArgAction::SetTrue)
                        .help("Replace a skill that is there"),
                ),
        )
}

fn profile() -> Command {
    noun("profile", "Inspect profiles (named fragment collections)")
        .subcommand(list("List all profiles"))
        .subcommand(
            Command::new("show")
                .about("Show details of a profile")
                .arg(one("name", "The profile")),
        )
        .subcommand(
            Command::new("create")
                .about("Create a new profile with included bundles and/or parents")
                .arg(one("name", "The new profile"))
                .arg(list_flag("parent", "Parent profile(s) to inherit from"))
                .arg(
                    Arg::new("include")
                        .short('i')
                        .long("include")
                        .value_delimiter(',')
                        .action(ArgAction::Append)
                        .help("Bundle(s) the profile includes"),
                )
                .arg(description("Description of the profile"))
                .arg(named("llm", "Preferred LLM config label/backend to launch"))
                .arg(named(
                    "bundle",
                    "Local bundle to create the profile in (default: the project bundle)",
                )),
        )
        .subcommand(
            Command::new("modify")
                .about("Modify an existing profile by adding or removing items")
                .arg(one("name", "The profile"))
                .arg(list_flag("add-parent", "Parent profile(s) to add"))
                .arg(list_flag("remove-parent", "Parent profile(s) to remove"))
                .arg(list_flag("add-bundle", "Bundle(s) to add"))
                .arg(list_flag("remove-bundle", "Bundle(s) to remove"))
                .arg(description("New description for the profile"))
                .arg(named(
                    "llm",
                    "Set the preferred LLM config label/backend (empty clears it)",
                ))
                .arg(list_flag("exclude-fragment", "Fragment name(s) to exclude"))
                .arg(list_flag(
                    "include-fragment",
                    "Fragment name(s) to stop excluding",
                ))
                .arg(list_flag("exclude-mcp", "MCP server name(s) to exclude"))
                .arg(list_flag(
                    "include-mcp",
                    "MCP server name(s) to stop excluding",
                )),
        )
        .subcommand(
            Command::new("edit")
                .about("Edit a profile in your editor")
                .arg(one("name", "The profile")),
        )
        .subcommand(
            Command::new("export")
                .about("Export a profile to a directory")
                .arg(one("name", "The profile"))
                .arg(one("dest", "The directory")),
        )
        .subcommand(
            Command::new("import")
                .about("Import a profile from a local file")
                .arg(one("path", "The profile's YAML file"))
                .arg(force("Overwrite an existing profile"))
                .arg(named(
                    "bundle",
                    "Local bundle to import into (default: the project bundle)",
                )),
        )
        .subcommand(remove("Remove a profile").arg(one("name", "The profile")))
}

fn agent() -> Command {
    noun("agent", "Inspect local agents (engine-profile bindings)")
        .subcommand(list("List all local agents"))
        .subcommand(
            Command::new("show")
                .about("Show an agent and its resolved engine")
                .arg(one("name", "The agent")),
        )
        .subcommand(agent_write(
            "create",
            "Create a local agent (engine-profile binding)",
        ))
        .subcommand(agent_write(
            "edit",
            "Edit an existing local agent (engine-profile binding)",
        ))
        .subcommand(
            Command::new("default")
                .about("Show or set the always-bound default agent")
                .arg(Arg::new("name").value_name("NAME")),
        )
        .subcommand(remove("Remove a local agent from config.yaml").arg(one("name", "The agent")))
}

/// `agent create` and `agent edit`: the same axes either way.
fn agent_write(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(one("name", "The agent"))
        .arg(named("llm", "llm.configs label to bind (empty = project default)"))
        .arg(
            Arg::new("profiles")
                .long("profiles")
                .value_name("PROFILES")
                .value_delimiter(',')
                .allow_hyphen_values(true)
                .num_args(1)
                .help("Comma-separated profile name(s)/ref(s) to compose"),
        )
        .arg(named(
            "runtime",
            "Runtime axis: host|container-rootless|container-rootful (empty = project default)",
        ))
        .arg(each("surface", "Delivery preference: kind=approach (repeatable)"))
        .arg(each("root", "Root selection: kind=root (repeatable)"))
        .arg(named(
            "permissions",
            "Permission mode, in the vocabulary of the engine this agent binds (empty clears it)",
        ))
        .arg(named("engine-home", "Engine-home axis: session|host (empty = session)"))
        .arg(
            Arg::new("env-host")
                .long("env-host")
                .num_args(0..=1)
                .require_equals(true)
                .default_missing_value("true")
                .value_parser(clap::builder::BoolishValueParser::new())
                .help("Whether the engine inherits the host environment whole (false = a curated base plus the --env names)"),
        )
        .arg(each(
            "env",
            "Bare variable name passed through when --env-host=false (repeatable; '' clears the list)",
        ))
}

fn llm() -> Command {
    noun("llm", "List LLM backends")
        .subcommand(list("List available LLMs"))
        .subcommand(llm_write("create", "Create a new LLM engine config"))
        .subcommand(llm_write("edit", "Edit an existing LLM engine config"))
        .subcommand(
            Command::new("default")
                .about("Show or set the default LLM")
                .arg(Arg::new("name").value_name("NAME")),
        )
        .subcommand(
            remove("Remove an LLM engine config from config.yaml")
                .arg(one("label", "The label"))
                .arg(home_flag()),
        )
}

fn home_flag() -> Arg {
    Arg::new("home")
        .long("home")
        .action(ArgAction::SetTrue)
        .help("Act on the user-global ~/.ctxloom/config.yaml instead of the project's")
}

fn llm_write(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(one("label", "The label"))
        .arg(named("type", "Backend discriminator (empty = claude-code)"))
        .arg(named("model", "Model string"))
        .arg(named("permissions", "Permission mode, in the label's engine's vocabulary (empty clears it)"))
        .arg(each("set", "Set a field by dotted key: <dotted.key>=<value>, the value read as YAML, empty removes it (repeatable)"))
        .arg(home_flag())
}

fn mcp() -> Command {
    noun("mcp", "List the MCP servers this project registers").subcommand(
        noun(
            "server",
            "List and show the MCP servers this project registers",
        )
        .subcommand(list("List configured MCP servers"))
        .subcommand(
            Command::new("show")
                .about("Show details of an MCP server configuration")
                .arg(one("name", "The server")),
        )
        .subcommand(
            Command::new("edit")
                .about("Edit an MCP server configuration in $EDITOR")
                .arg(one("ref", "<bundle>#mcp/<name>")),
        )
        .subcommand(
            Command::new("set")
                .about("Set fields of a bundle's MCP server from flags")
                .arg(one("ref", "<bundle>#mcp/<name>"))
                .arg(named(
                    "command",
                    "Command that launches a stdio server (empty clears it)",
                ))
                .arg(each(
                    "arg",
                    "Command argument (repeatable; replaces the list)",
                ))
                .arg(each(
                    "env",
                    "Environment variable as NAME=value (repeatable; replaces the map)",
                ))
                .arg(named(
                    "url",
                    "Endpoint of a remote server, http or https (empty clears it)",
                ))
                .arg(each(
                    "header",
                    "HTTP header as Name=value (repeatable; replaces the map)",
                ))
                .arg(each("tag", "Tag (repeatable; replaces the list)"))
                .arg(named(
                    "served-by",
                    "session-endpoint: the running session serves this entry (empty clears it)",
                ))
                .arg(named("notes", "Human-readable notes (empty clears them)"))
                .arg(named(
                    "installation",
                    "Setup instructions shown to the user (empty clears them)",
                )),
        ),
    )
}

fn companion() -> Command {
    noun("companion", "List the companion programs ctxloom runs")
        .subcommand(list(
            "List registered companions and whether each resolves on PATH",
        ))
        .subcommand(
            Command::new("add")
                .about("Register a companion by name, after checking it answers")
                .arg(one("name", "The companion's name").allow_hyphen_values(true)),
        )
        .subcommand(
            Command::new("remove")
                .visible_alias("rm")
                .about("Unregister a companion")
                .arg(one("name", "The companion's name"))
                .arg(yes()),
        )
}

fn loadout() -> Command {
    Command::new("loadout").about("Print ctxloom's own loadout: what it contributes to a session")
}

fn search() -> Command {
    Command::new("search")
        .about("Search the project's content")
        .arg(Arg::new("query").value_name("QUERY"))
        .arg(
            Arg::new("tag")
                .short('t')
                .long("tag")
                .value_delimiter(',')
                .action(ArgAction::Append)
                .help("Filter by tags (comma-separated; matches any)"),
        )
        .arg(Arg::new("type").long("type").value_name("TYPE").help(
            "Filter by type: fragment, command, skill, profile, bundle, mcp_server (default: all)",
        ))
        .arg(
            Arg::new("local")
                .long("local")
                .action(ArgAction::SetTrue)
                .help("Search only local content"),
        )
        .arg(
            Arg::new("remote")
                .long("remote")
                .action(ArgAction::SetTrue)
                .help("Search only remote repositories"),
        )
}

fn many(name: &'static str, short: char, help: &'static str) -> Arg {
    Arg::new(name)
        .short(short)
        .long(name)
        .value_delimiter(',')
        .action(ArgAction::Append)
        .help(help)
}

fn context() -> Command {
    noun("context", "Assemble context").subcommand(
        Command::new("assemble")
            .about("Assemble the context of a profile set, fragments and tags")
            .arg(many(
                "profile",
                'p',
                "Profile(s) to compose (can be repeated)",
            ))
            .arg(many(
                "fragment",
                'f',
                "Fragment(s) to include (can be repeated)",
            ))
            .arg(many(
                "tag",
                't',
                "Include fragments with this tag (can be repeated)",
            )),
    )
}

fn run() -> Command {
    Command::new("run")
        .about("Assemble context and run AI (this build previews with --dry-run)")
        .arg(Arg::new("prompt_words").value_name("PROMPT").num_args(0..))
        .arg(
            Arg::new("llm")
                .short('l')
                .long("llm")
                .value_name("LABEL")
                .help("Config label to use (e.g. claude-code, claude-fast); overrides the configured default"),
        )
        .arg(
            Arg::new("prompt")
                .long("prompt")
                .value_name("PROMPT")
                .help("Prompt to send to the AI (alternative to positional args)"),
        )
        .arg(
            Arg::new("command")
                .short('r')
                .long("command")
                .value_name("NAME")
                .help("Run a saved command by name"),
        )
        .arg(many("fragment", 'f', "Context fragment(s) to include (can be repeated)"))
        .arg(many("tag", 't', "Include fragments with this tag (can be repeated)"))
        .arg(
            Arg::new("profile")
                .short('p')
                .long("profile")
                .value_name("PROFILE")
                .overrides_with("profile")
                .help("Profile to use (predefined fragment collection)"),
        )
        .arg(
            Arg::new("agent")
                .long("agent")
                .value_name("NAME")
                .help("Run a named local agent binding (excludes -p/-f/-t)"),
        )
        .arg(
            Arg::new("workspace")
                .long("workspace")
                .value_name("AXIS")
                .help("Session workspace axis (none|worktree; empty = project default)"),
        )
        .arg(
            Arg::new("dry-run")
                .short('n')
                .long("dry-run")
                .action(ArgAction::SetTrue)
                .help("Show what a launch would carry, without launching"),
        )
}
