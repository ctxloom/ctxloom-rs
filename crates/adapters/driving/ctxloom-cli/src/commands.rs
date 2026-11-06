//! The command line's shape.

use clap::{ArgAction, Args, Command, Parser, Subcommand};

/// The program's name.
pub const PROG: &str = "ctxloom";

/// The command line.
#[derive(Parser, Debug)]
#[command(name = PROG, about = "Context for agents: bundles, profiles and sessions")]
pub struct Cli {
    #[command(flatten)]
    pub globals: Globals,
    #[command(subcommand)]
    pub command: Option<Verb>,
}

/// The flags every command takes.
#[derive(Args, Debug)]
pub struct Globals {
    /// The output format: json or text (json when stdout is not a terminal)
    #[arg(long, global = true, value_name = "FORMAT", overrides_with = "format")]
    pub format: Option<String>,
    /// Accepted by every command; changes no output
    #[arg(short = 'v', long, global = true, action = ArgAction::Count)]
    pub verbose: u8,
    /// Degrade instead of failing: downgrade fatal startup findings (broken
    /// config, unresolvable profiles or bundles) to warnings
    #[arg(long, global = true)]
    pub degraded: bool,
    /// Run no registered companion: contribute none of their commands,
    /// hooks, MCP servers or context
    #[arg(long, global = true)]
    pub no_companions: bool,
    /// Override a config value for this invocation, repeatable (e.g.
    /// --config-set llm.defaults.primary=big)
    #[arg(long = "config-set", global = true, value_name = "PATH=VALUE", action = ArgAction::Append)]
    pub config_set: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Verb {
    /// Print the ctxloom version
    #[command(after_help = "Examples:\n  ctxloom version --format json")]
    Version,
    /// Generate the shell completion script
    #[command(
        after_help = "Examples:\n  ctxloom completion bash > /etc/bash_completion.d/ctxloom"
    )]
    Completion(CompletionArgs),
}

/// `completion`'s argument.
#[derive(Args, Debug)]
pub struct CompletionArgs {
    /// The shell to generate for
    #[arg(value_enum, value_name = "SHELL")]
    pub shell: clap_complete::Shell,
}

/// The command with its version flag (`--version`, long only: `-v` is
/// verbose), and with no `help` command anywhere: help is `-h` and
/// `--help`, and a group given no command prints its own.
pub fn command(version: &str, nouns: Vec<Command>) -> Command {
    use clap::CommandFactory;
    let mut command = without_help_command(
        Cli::command()
            .version(version.to_string())
            .disable_version_flag(true)
            .arg_required_else_help(true)
            .arg(
                clap::Arg::new("version")
                    .long("version")
                    .help("Print the ctxloom version")
                    .action(ArgAction::Version),
            ),
    );
    // An extension's nouns arrive shaped: a group that lists when bare must not
    // be made to print its help.
    for noun in nouns {
        command = command.subcommand(noun);
    }
    command
}

/// The groups whose bare form runs their default verb rather than printing
/// their usage (D28: "a bare namespace runs its default verb"; Go's
/// `groupNodeDefault`). A group added by a later slice names itself here.
pub const BARE_RUNS_DEFAULT: [&str; 6] = [
    "session",
    "transcript",
    "init",
    "worktrees",
    "tooling",
    "container",
];

/// A noun shaped as the command line shapes its own: no `help` command, and a
/// bare group printing its usage unless it is in [`BARE_RUNS_DEFAULT`].
pub fn shaped(command: Command) -> Command {
    without_help_command(command)
}

fn without_help_command(command: Command) -> Command {
    let names: Vec<String> = command
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    let mut command = command;
    if !names.is_empty() {
        let bare_runs = BARE_RUNS_DEFAULT.contains(&command.get_name());
        command = command
            .disable_help_subcommand(true)
            .arg_required_else_help(!bare_runs);
    }
    for name in names {
        command = command.mut_subcommand(name, without_help_command);
    }
    command
}
