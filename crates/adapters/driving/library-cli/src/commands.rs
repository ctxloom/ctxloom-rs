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
    ]
}

fn config() -> Command {
    noun("config", "Show ctxloom configuration").subcommand(
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
}

fn fragment() -> Command {
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
}

fn profile() -> Command {
    noun("profile", "Inspect profiles (named fragment collections)")
        .subcommand(list("List all profiles"))
        .subcommand(
            Command::new("show")
                .about("Show details of a profile")
                .arg(one("name", "The profile")),
        )
}

fn agent() -> Command {
    noun("agent", "Inspect local agents (engine-profile bindings)")
        .subcommand(list("List all local agents"))
        .subcommand(
            Command::new("show")
                .about("Show an agent and its resolved engine")
                .arg(one("name", "The agent")),
        )
}

fn llm() -> Command {
    noun("llm", "List LLM backends").subcommand(list("List available LLMs"))
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
        ),
    )
}

fn companion() -> Command {
    noun("companion", "List the companion programs ctxloom runs").subcommand(list(
        "List registered companions and whether each resolves on PATH",
    ))
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
