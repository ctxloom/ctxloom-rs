//! The setup commands' shape: nouns and verbs, flags and arguments.

use clap::{Arg, ArgAction, Command};

fn group(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .disable_help_subcommand(true)
        .subcommand_required(false)
        .arg_required_else_help(false)
}

/// The nouns this adds to the command line.
pub fn nouns() -> Vec<Command> {
    vec![manage(), materialize(), util()]
}

fn manage() -> Command {
    group("manage", "Install and manage ctxloom's project harness")
        .subcommand(
            Command::new("install")
                .about("Scaffold .ctxloom and git-ignore its private state")
                .after_help("Examples:\n  ctxloom manage install --engine claude-code")
                .arg(
                    Arg::new("engine")
                        .long("engine")
                        .value_name("ENGINE")
                        .help("AI engine to record when scaffolding"),
                )
                .arg(
                    Arg::new("print")
                        .long("print")
                        .action(ArgAction::SetTrue)
                        .help("Print the steps that would run, without executing"),
                )
                .arg(
                    Arg::new("no-pull")
                        .long("no-pull")
                        .action(ArgAction::SetTrue)
                        .help("Skip the dependency pull install ends with; declared dependencies stay uninstalled until 'ctxloom deps pull' runs"),
                ),
        )
        .subcommand(
            Command::new("uninstall")
                .about("Remove what ctxloom delivered into the project, for every engine"),
        )
        .subcommand(Command::new("check").about("Show what ctxloom has wired into this project"))
        .subcommand(
            group("hooks", "Inspect ctxloom backend hooks")
                .subcommand(
                    Command::new("check").about("Show which backends have ctxloom hooks wired in"),
                )
                .subcommand(
                    Command::new("list")
                        .about("List the hooks that will fire, per event, in their resolved order")
                        .arg(
                            Arg::new("event")
                                .long("event")
                                .value_name("EVENT")
                                .help("Report only this lifecycle event"),
                        )
                        .arg(
                            Arg::new("profile")
                                .long("profile")
                                .value_name("PROFILE")
                                .action(ArgAction::Append)
                                .help("Resolve against these profiles instead of the configured defaults (repeatable)"),
                        ),
                ),
        )
        .subcommand(
            group("statusline", "Enable or disable ctxloom's HUD statusline")
                .subcommand(
                    Command::new("install").about("Let ctxloom manage the HUD statusline (default)"),
                )
                .subcommand(
                    Command::new("uninstall")
                        .about("Stop managing the HUD statusline; keep your own"),
                ),
        )
        .subcommand(
            group("gitignore", "Maintain ctxloom's .gitignore entries").subcommand(
                Command::new("install").about("Add ctxloom's private-state and transient-artifact ignores"),
            ),
        )
}

fn materialize() -> Command {
    Command::new("materialize")
        .about("Write the profiles' surfaces at rest, as each engine's native files")
        .after_help("Examples:\n  ctxloom materialize dev --target out --backend claude-code\n  ctxloom materialize --release --surface commands --target out --backend mock")
        .arg(
            Arg::new("profiles")
                .value_name("PROFILE")
                .num_args(0..)
                .help("The profiles to deliver (the default agent's when none)"),
        )
        .arg(
            Arg::new("backend")
                .long("backend")
                .value_name("ENGINE")
                .action(ArgAction::Append)
                .help("Deliver for this engine (repeatable; the configured engines when none)"),
        )
        .arg(
            Arg::new("surface")
                .long("surface")
                .value_name("KIND[=MECHANISM][:DEST]")
                .action(ArgAction::Append)
                .help("Deliver only this kind: context, mcp, settings, hooks, commands, skills (repeatable)"),
        )
        .arg(
            Arg::new("target")
                .long("target")
                .value_name("DIR")
                .help("Write into this directory instead of the project's"),
        )
        .arg(
            Arg::new("release")
                .long("release")
                .action(ArgAction::SetTrue)
                .help("Take back exactly what the selected kinds delivered"),
        )
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes")
                .action(ArgAction::SetTrue)
                .help("Write the project directory when no --target is given"),
        )
        .arg(
            Arg::new("dry-run")
                .short('n')
                .long("dry-run")
                .action(ArgAction::SetTrue)
                .help("Plan and report; write nothing"),
        )
        .arg(
            Arg::new("diff")
                .long("diff")
                .value_name("FILE")
                .help("Compare the context as it would be written against this file, writing nothing"),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .action(ArgAction::SetTrue)
                .help("Write an engine's user-global scope too"),
        )
}

fn util() -> Command {
    group("util", "Helpers other tools call (internal)")
        .hide(true)
        .subcommand(
            Command::new("config-write")
                .about("Guarded merge-write into a third-party config file (internal)")
                .arg(
                    Arg::new("file")
                        .long("file")
                        .value_name("PATH")
                        .required(true)
                        .help("Absolute, already-resolved path to the target config file"),
                )
                .arg(
                    Arg::new("filetype")
                        .long("filetype")
                        .value_name("TYPE")
                        .help("The file's content format: json (inferred from --file's extension if omitted)"),
                ),
        )
}
