//! The remote and dependency commands' shape: nouns and verbs, flags and
//! arguments.

use clap::{Arg, ArgAction, Command};

/// A noun whose bare form is its default verb: no help is demanded of it.
fn noun(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .disable_help_subcommand(true)
        .subcommand_required(false)
        .arg_required_else_help(false)
}

fn one(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .value_name(name.to_uppercase())
        .required(true)
        .help(help)
}

fn yes(help: &'static str) -> Arg {
    Arg::new("yes")
        .short('y')
        .long("yes")
        .action(ArgAction::SetTrue)
        .help(help)
}

/// The nouns this adds to the command line.
pub fn nouns() -> Vec<Command> {
    vec![remote(), deps()]
}

/// The verbs this adds to the library's `bundle` noun.
pub fn bundle_verbs() -> Vec<Command> {
    vec![push(), move_()]
}

/// Whether `verb` of `bundle` is one of these.
pub fn is_bundle_verb(verb: &str) -> bool {
    matches!(verb, "push" | "move")
}

fn remote() -> Command {
    noun(
        "remote",
        "Register and browse the sources content comes from",
    )
    .subcommand(
        Command::new("list")
            .visible_alias("ls")
            .about("List configured remotes"),
    )
    .subcommand(
        Command::new("show")
            .about("Show a remote and the bundles it publishes")
            .arg(one("remote", "The remote"))
            .arg(
                Arg::new("recursive")
                    .short('r')
                    .long("recursive")
                    .num_args(0..=1)
                    .default_missing_value("true")
                    .help("Descend into subdirectories (the default)"),
            ),
    )
    .subcommand(
        Command::new("create")
            .about("Register a remote source")
            .arg(one("name", "The remote's name"))
            .arg(one("url", "Its address"))
            .arg(
                Arg::new("forge").long("forge").value_name("FORGE").help(
                    "Forge to bind this remote to: github, git, or a configured forges: label",
                ),
            ),
    )
    .subcommand(remote_edit())
    .subcommand(
        Command::new("remove")
            .visible_aliases(["rm", "del"])
            .about("Remove a remote source")
            .arg(one("remote", "The remote"))
            .arg(yes(
                "Apply the removal this invocation would report (default: report only)",
            )),
    )
    .subcommand(
        Command::new("default")
            .about("Set the default remote")
            .arg(Arg::new("remote").value_name("REMOTE"))
            .arg(
                Arg::new("clear")
                    .long("clear")
                    .action(ArgAction::SetTrue)
                    .help("Clear the default remote"),
            ),
    )
    .subcommand(remote_discover())
}

fn remote_edit() -> Command {
    Command::new("edit")
        .about("Change a remote's name, URL or forge binding")
        .arg(one("remote", "The remote"))
        .arg(
            Arg::new("name")
                .long("name")
                .value_name("NAME")
                .help("Rename the remote"),
        )
        .arg(
            Arg::new("url")
                .long("url")
                .value_name("URL")
                .help("Change the repository URL"),
        )
        .arg(
            Arg::new("forge")
                .long("forge")
                .value_name("FORGE")
                .help("Rebind to a forge (empty resolves from the URL host)"),
        )
}

fn remote_discover() -> Command {
    Command::new("discover")
        .about("Search GitHub for ctxloom repositories")
        .arg(Arg::new("query").value_name("QUERY").num_args(0..))
        .arg(
            Arg::new("source")
                .short('s')
                .long("source")
                .default_value("all")
                .help("Search source: github or all"),
        )
        .arg(
            Arg::new("limit")
                .short('n')
                .long("limit")
                .default_value("30")
                .help("Maximum results"),
        )
        .arg(
            Arg::new("stars")
                .long("stars")
                .default_value("0")
                .help("Minimum star count"),
        )
}

fn deps() -> Command {
    noun(
        "deps",
        "Manage this project's installed dependency closure",
    )
    .subcommand(
        Command::new("list")
            .visible_alias("ls")
            .about("List the installed dependency closure"),
    )
    .subcommand(
        Command::new("pull")
            .about("Make this project's installed closure match upstream")
            .arg(
                Arg::new("force")
                    .short('f')
                    .long("force")
                    .action(ArgAction::SetTrue)
                    .help("Reinstall every reference at its pin instead of skipping what is installed"),
            )
            .arg(
                Arg::new("lock")
                    .long("lock")
                    .num_args(0..=1)
                    .default_value("true")
                    .default_missing_value("true")
                    .value_parser(clap::value_parser!(bool))
                    .help("Update the lockfile after the pull (--lock=false leaves it alone)"),
            )
            .arg(yes("Let the post-pull refresh write the project directory's engine files")),
    )
    .subcommand(
        Command::new("check")
            .about("Report which dependencies have a newer commit available")
            .arg(Arg::new("reference").value_name("REFERENCE")),
    )
    .subcommand(
        Command::new("upgrade")
            .about("Show, then apply, the newest pins your constraints allow")
            .arg(yes("Apply the upgrade this invocation would report (default: report only)")),
    )
    .subcommand(
        Command::new("hold")
            .about("Freeze a dependency at its locked commit so `upgrade` cannot advance it")
            .arg(one("name", "The dependency")),
    )
    .subcommand(
        Command::new("unhold")
            .about("Release a hold so `upgrade` can advance the dependency again")
            .arg(one("name", "The dependency")),
    )
}

fn push() -> Command {
    Command::new("push")
        .about("Publish a bundle to a registered remote")
        .arg(one("bundle", "The bundle"))
        .arg(
            Arg::new("remote")
                .value_name("REMOTE")
                .help("The remote (the default remote where left out)"),
        )
        .arg(
            Arg::new("message")
                .short('m')
                .long("message")
                .value_name("MESSAGE")
                .help("The commit message"),
        )
        .arg(
            Arg::new("pr")
                .long("pr")
                .action(ArgAction::SetTrue)
                .help("Open a pull request instead (needs a forge)"),
        )
}

fn move_() -> Command {
    Command::new("move")
        .about("Move a bundle to a remote or another project")
        .arg(one("name", "The bundle"))
        .arg(
            Arg::new("to")
                .long("to")
                .required(true)
                .value_name("DEST")
                .help("A registered remote's name, or an existing directory"),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .action(ArgAction::SetTrue)
                .help("Overwrite a bundle of the same name at a directory"),
        )
        .arg(
            Arg::new("message")
                .short('m')
                .long("message")
                .value_name("MESSAGE")
                .help("The commit message for a remote destination"),
        )
}
