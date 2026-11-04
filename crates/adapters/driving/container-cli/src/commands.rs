//! The command's shape.

use clap::{Arg, ArgAction, Command};

/// The nouns this adds to the command line.
pub fn nouns() -> Vec<Command> {
    vec![container()]
}

fn backend() -> Arg {
    Arg::new("backend")
        .value_name("BACKEND")
        .help("The engine whose agent image this is about (the configured default when omitted)")
}

fn runtime(help: &'static str) -> Arg {
    Arg::new("runtime")
        .long("runtime")
        .value_name("RUNTIME")
        .help(help)
}

fn container() -> Command {
    Command::new("container")
        .about("Manage agent container images")
        .long_about(
            "Manage the per-backend agent images a containerized engine runs in \
             (agents with 'runtime: container', or the project 'runtime:' default).",
        )
        .subcommand(check())
        .subcommand(build())
        .subcommand(prune())
        .subcommand(scaffold())
        .subcommand(tooling())
}

fn check() -> Command {
    Command::new("check")
        .about("Diagnose container capability (runtime, image, shared filesystem)")
        .long_about(
            "Report whether containerized agents ('runtime: container') can launch here: \
             whether THIS process runs inside a container, which container runtime is \
             reachable, whether the backend's agent image is present locally, and whether \
             the runtime's daemon shares this filesystem.\n\n\
             Diagnostic only: no probe outcome fails the command, and nothing is built or \
             changed - read the report, not the exit code. A usage error is still an error \
             (an unknown backend argument, or a --format this build cannot render).",
        )
        .after_help("Examples:\n  ctxloom container check")
        .arg(backend())
}

fn build() -> Command {
    Command::new("build")
        .about("Build the agent container image for a backend")
        .long_about(
            "Build the agent image a containerized run of the given backend uses (the \
             configured default backend when omitted). The backend is validated before any \
             daemon is touched.",
        )
        .after_help("Examples:\n  ctxloom container build\n  ctxloom container build --runtime podman")
        .arg(backend())
        .arg(
            Arg::new("overlay-image")
                .long("overlay-image")
                .value_name("IMAGE")
                .help("overlay ctxloom onto this base image instead of the default build sources"),
        )
        .arg(
            Arg::new("base")
                .long("base")
                .value_name("BASE")
                .help("the base the engine's agent stage layers onto: ctxloom | devcontainer | <image ref>"),
        )
        .arg(
            Arg::new("devcontainer-service")
                .long("devcontainer-service")
                .value_name("SERVICE")
                .help("docker-compose service to use as the base when the project devcontainer.json declares dockerComposeFile"),
        )
        .arg(
            Arg::new("engines")
                .long("engines")
                .value_name("ENGINES")
                .value_delimiter(',')
                .help("engines to build an agent image for, one image each"),
        )
        .arg(runtime("container runtime to build with (docker|podman); auto-detected when empty"))
        .arg(
            Arg::new("keep-cache")
                .long("keep-cache")
                .action(ArgAction::SetTrue)
                .help("reuse cached layers instead of --pull --no-cache"),
        )
}

fn prune() -> Command {
    Command::new("prune")
        .about("Remove superseded ctxloom agent images (a dry run unless --yes)")
        .long_about(
            "Find the agent images ctxloom built that nothing uses any more, and - with --yes - \
             remove them. Without --yes this only prints the plan.\n\n\
             Exit status: 0 when the plan printed or everything planned was removed; 1 when any \
             removal failed; 3 when no container runtime is available.",
        )
        .after_help(
            "Examples:\n  ctxloom container prune\n  ctxloom container prune --yes --min-age 72h",
        )
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes")
                .action(ArgAction::SetTrue)
                .help("remove the superseded images (default: print the plan and remove nothing)"),
        )
        .arg(
            Arg::new("min-age")
                .long("min-age")
                .value_name("DURATION")
                .help("keep any image younger than this"),
        )
        .arg(runtime(
            "sweep only this container runtime (docker|podman); every available one when empty",
        ))
}

fn scaffold() -> Command {
    Command::new("scaffold")
        .about("Write a project devcontainer seeded from ctxloom's default base")
        .long_about(
            "Write .devcontainer/devcontainer.json and a .devcontainer/Dockerfile seeded from \
             ctxloom's embedded default base, so the agent image's base is a file you can edit.\n\n\
             Refused when the project already has a devcontainer (.devcontainer/ or \
             .devcontainer.json): edit that one instead.",
        )
        .after_help("Examples:\n  ctxloom container scaffold")
}

fn tooling() -> Command {
    Command::new("tooling")
        .about("Agent-image tooling declarations from registered companions")
        .long_about(
            "Collect every registered companion's typed 'tooling' declaration - the tools its \
             content needs inside the agent container image - and emit them with instructions \
             for the LLM. Only companions you registered ('ctxloom companion add') contribute; \
             nothing is ever applied automatically.",
        )
        .subcommand(
            Command::new("list")
                .about("Emit registered companions' agent-image tooling declarations for the LLM to apply")
                .after_help("Examples:\n  ctxloom container tooling list"),
        )
}
