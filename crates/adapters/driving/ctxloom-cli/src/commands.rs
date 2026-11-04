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
    /// Browse and manage your recorded sessions
    #[command(after_help = "Examples:\n  ctxloom session list\n  ctxloom session search lockfile")]
    Session(session_cli::Session),
    /// Generate the shell completion script
    #[command(
        after_help = "Examples:\n  ctxloom completion bash > /etc/bash_completion.d/ctxloom"
    )]
    Completion(CompletionArgs),
    /// Callbacks ctxloom installs into engines (not for people)
    #[command(hide = true)]
    Hook(hook_cli::Hook),
    /// Commands for the tests only
    #[cfg(feature = "test-support")]
    #[command(subcommand, hide = true)]
    Test(TestVerb),
}

/// `completion`'s argument.
#[derive(Args, Debug)]
pub struct CompletionArgs {
    /// The shell to generate for
    #[arg(value_enum, value_name = "SHELL")]
    pub shell: clap_complete::Shell,
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
    /// Plant a session in the cwd's project store, and the files a fixture
    /// writes beside it; print its stable id
    SeedSession(SeedArgs),
}

/// What `ctxloom test seed-session` plants (see the acceptance tests).
#[cfg(feature = "test-support")]
#[derive(Args, Debug, Clone, Default)]
pub struct SeedArgs {
    /// The session's name
    pub harp: String,
    /// Another project than the cwd's
    #[arg(long)]
    pub project: Option<String>,
    /// Seconds ago it was minted
    #[arg(long, default_value_t = 86_400)]
    pub started_ago: u64,
    /// Seconds ago its last run ended; left out, it never ended
    #[arg(long)]
    pub ended_ago: Option<u64>,
    /// `free` makes its lock file with nothing holding it; `none` makes none
    #[arg(long, default_value = "none")]
    pub lock: String,
    /// Record the canonical transcript Go's fixture records
    #[arg(long)]
    pub transcript: bool,
    /// Mark its transcript kept
    #[arg(long)]
    pub keep: bool,
    /// An internal one-shot's session
    #[arg(long)]
    pub oneshot: bool,
    /// The engine's session it is bound to
    #[arg(long)]
    pub native_id: Option<String>,
    /// The engine's transcript it is bound to
    #[arg(long)]
    pub native_transcript: Option<String>,
    /// An authored file in the session directory
    #[arg(long)]
    pub other_file: Vec<String>,
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
pub const BARE_RUNS_DEFAULT: [&str; 3] = ["session", "transcript", "init"];

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
