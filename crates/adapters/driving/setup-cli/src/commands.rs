//! The setup commands' shape: nouns, flags and arguments.

use clap::{Arg, ArgAction, Command};

fn flag(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::SetTrue)
        .help(help)
}

/// The nouns this adds to the command line.
pub fn nouns() -> Vec<Command> {
    vec![auth(), clean(), doctor(), init()]
}

fn auth() -> Command {
    Command::new("auth")
        .about("Show how engines authenticate and whether each credential is exported")
        .long_about(
            "Every agent ctxloom spawns uses one long-lived token you mint and export: for \
             claude, run 'claude setup-token' and export CLAUDE_CODE_OAUTH_TOKEN, or export it \
             from your secret manager.\n\n\
             ctxloom never collects, stores or mints a credential. It reads the token from the \
             environment it is launched in, so this reports whether each is exported and never \
             its value, and there is no command that takes one.",
        )
        .after_help("Examples:\n  ctxloom auth\n  ctxloom auth status")
        .subcommand(Command::new("status").about(
            "Show whether each engine's credential is present in the environment (never its value)",
        ))
}

fn clean() -> Command {
    Command::new("clean")
        .about("Remove this project's regenerable cache, keeping everything a clone cannot restore")
        .long_about(
            "Removes .ctxloom/cache: the pulled bundle copies, the git clone cache and the \
             assembled context files. Each is rebuilt by a command the report names.\n\n\
             Never taken: authored content and profiles, lock.yaml, a session's identity and \
             its output dir. Without --yes this only reports.",
        )
        .after_help("Examples:\n  ctxloom clean            # report what would be removed\n  ctxloom clean --yes")
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes")
                .action(ArgAction::SetTrue)
                .help("apply exactly the plan this reports"),
        )
}

fn doctor() -> Command {
    Command::new("doctor")
        .about("Run deterministic setup checks (deps, agents, hooks, MCP, gitignore)")
        .after_help(
            "Examples:\n  ctxloom doctor            # the warnings, and the first fix\n  ctxloom doctor --all      # every check\n  ctxloom doctor --deps     # only what this machine needs\n  ctxloom doctor --fix      # remove stale ctxloom hook entries, then report",
        )
        .arg(flag("all", "list every check in the text report, not only the warnings"))
        .arg(flag(
            "deps",
            "check ONLY machine-capability dependencies (git and its identity), for use before a project is set up",
        ))
        .arg(flag(
            "fix",
            "remove settings hook entries that run a 'ctxloom hook' subcommand this ctxloom does not have, then report",
        ))
}

fn init() -> Command {
    Command::new("init")
        .about("Set up ctxloom in this project (or re-run it)")
        .after_help("Examples:\n  ctxloom init\n  ctxloom init --remote owner/repo\n  ctxloom init prompt")
        .arg(
            Arg::new("remote")
                .long("remote")
                .action(ArgAction::Append)
                .value_name("REPO")
                .help("Personal ctxloom repo to add as a trusted remote (owner/repo or URL); repeatable"),
        )
        .arg(
            Arg::new("forge")
                .long("forge")
                .value_name("FORGE")
                .help("Bind every --remote to this forge (github, git, or a configured forges: label)"),
        )
        .subcommand(
            Command::new("prompt")
                .about("Print ctxloom's setup prompt (companions, profiles, agents) for the LLM to follow"),
        )
}

#[cfg(test)]
#[path = "commands.test.rs"]
mod tests;
