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
}

#[derive(Subcommand, Debug)]
pub enum Verb {
    /// Print the ctxloom version
    #[command(after_help = "Examples:\n  ctxloom version --format json")]
    Version,
    /// Commands for the tests only
    #[cfg(feature = "test-support")]
    #[command(subcommand, hide = true)]
    Test(TestVerb),
}

#[cfg(feature = "test-support")]
#[derive(Subcommand, Debug)]
pub enum TestVerb {
    /// Refuse (exit 2)
    Refuse,
    /// Report fatal findings (exit 3)
    Findings,
    /// Print a sample list
    Sample {
        /// Print the empty list
        #[arg(long)]
        empty: bool,
    },
    /// Print every diagnostic the binary can raise, as envelopes
    Diagnostics,
    /// Mint a session id
    SessionId,
    /// Print a run's id
    RunId {
        /// The session's id (a UUID)
        session: String,
        /// The incarnation's number
        incarnation: u64,
    },
    /// Open a project's store under the home
    OpenStore {
        /// The project's id
        project: String,
    },
}

/// The command with its version flag (`--version`, long only: `-v` is
/// verbose), and with no `help` command anywhere: help is `-h` and
/// `--help`, and a group given no command prints its own.
pub fn command(version: &str) -> Command {
    use clap::CommandFactory;
    without_help_command(
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
    )
}

fn without_help_command(command: Command) -> Command {
    let names: Vec<String> = command
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    let mut command = command;
    if !names.is_empty() {
        command = command
            .disable_help_subcommand(true)
            .arg_required_else_help(true);
    }
    for name in names {
        command = command.mut_subcommand(name, without_help_command);
    }
    command
}
