//! taskloom: per-project task tracking. The composition root: it reads the
//! process's environment, wires the tasks use cases to their adapters (the
//! project registry, harp names, the SQLite task store, the system's
//! clock, ids, timer and randomness, taskloom's configuration, and Go
//! taskloom's logs to import), mounts the `project` group and `mcp` beside
//! the task commands, and hands the command line to the CLI. The `project`
//! group runs over the registry and the SQLite store's integrity check;
//! `mcp` serves the task tools over stdio, each call on a service of its
//! own; `watch` streams the project's changes until SIGINT or SIGTERM
//! (never at a closed stdin); `run` hands a task to the `ctxloom` on PATH; `manage`
//! registers `taskloom mcp` in the agents' configurations under HOME. It
//! holds no logic of its own.

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use go_task_log::GoTaskLogDir;
use harp_names::HarpNames;
use project_application::Projects;
use project_registry::{Environment, ProjectRegistry};
use system::System;
use task_store_sqlite::SqliteStores;
use tasks_application::{NoGoLogs, Service};
use tasks_cli::Context;
use tasks_config::TaskloomConfig;

/// Every crate's en-US catalog the binary's diagnostics come from: annals'
/// crates the binary links (annals-diagnostic defines no codes), then its
/// own.
fn catalog() -> Result<presentation::Catalog, presentation::CatalogError> {
    tasks_cli::catalog(&[
        annals::EN_US,
        annals_store_sql::EN_US,
        project_cli::EN_US,
        project_application::EN_US,
        registration_cli::EN_US,
        registration_application::EN_US,
        agent_configs::EN_US,
        file_lock::EN_US,
        project_registry::EN_US,
        task_store_sqlite::EN_US,
        go_task_log::EN_US,
        tasks_config::EN_US,
        tasks_mcp::EN_US,
        tasks_run::EN_US,
        tasks_watch::EN_US,
    ])
}

/// The commands mounted beside the task commands.
#[derive(clap::Subcommand, Debug)]
enum Mounted {
    #[command(flatten)]
    Project(project_cli::Group),
    #[command(flatten)]
    Mcp(tasks_mcp::Group),
    #[command(flatten)]
    Watch(tasks_watch::Group),
    #[command(flatten)]
    Run(tasks_run::Group),
    #[command(flatten)]
    Manage(registration_cli::Group),
}

/// What runs once the command line has been read, on the runtime.
enum After {
    Mcp(Option<String>),
    Watch(tasks_watch::Settings),
    Run(tasks_run::Group, tasks_run::Settings),
}

/// The tasks use cases over the real adapters, configured from the work
/// root's config, as one call or command needs them. `vars` are the
/// environment's, as the config reads them.
fn service(
    env: &Environment,
    vars: Vec<(String, String)>,
) -> Service<
    ProjectRegistry<HarpNames>,
    SqliteStores,
    HarpNames,
    System,
    System,
    NoGoLogs,
    TaskloomConfig,
> {
    let identity = ProjectRegistry::new(env.clone(), HarpNames);
    // The work root's config, where there is a work root: a command that
    // needs one fails on resolving it, and says why.
    let config = TaskloomConfig::new(env.home.clone(), identity.work_root().ok(), vars);
    Service::new(identity, SqliteStores::default(), HarpNames, System, System)
        .with_configuration(config)
}

/// `taskloom mcp`: serve the task tools on stdio until the client goes
/// away, each call on a service of its own.
fn serve_mcp(
    runtime: tokio::runtime::Runtime,
    env: Environment,
    settings: tasks_mcp::Settings,
    catalog: presentation::Catalog,
) -> ExitCode {
    let catalog = Arc::new(catalog);
    let vars = utf8_vars();
    let tools = tasks_mcp::TaskTools::new(
        move || service(&env, vars.clone()),
        settings,
        Arc::clone(&catalog),
    );
    let served = runtime.block_on(tools.serve_stdio());
    // Nothing is left to wait for: a stdin read still blocked is
    // abandoned, not joined.
    runtime.shutdown_background();
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}: {}", tasks_cli::PROG, catalog.message(&e));
            ExitCode::FAILURE
        }
    }
}

/// Resolves at SIGINT or SIGTERM (Ctrl-C alone off Unix), listening from
/// the moment it is made, so a signal before the first await is not lost
/// to the default disposition. Made on the runtime.
fn interrupted() -> std::io::Result<impl std::future::Future<Output = ()>> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut interrupt = signal(SignalKind::interrupt())?;
        let mut terminate = signal(SignalKind::terminate())?;
        Ok(std::future::poll_fn(move |cx| {
            let interrupted = interrupt.poll_recv(cx).is_ready();
            let terminated = terminate.poll_recv(cx).is_ready();
            if interrupted || terminated {
                std::task::Poll::Ready(())
            } else {
                std::task::Poll::Pending
            }
        }))
    }
    #[cfg(not(unix))]
    {
        Ok(async {
            let _ = tokio::signal::ctrl_c().await;
        })
    }
}

/// `taskloom watch`: the project's changes on stdout, polled every
/// debounce, until interrupted.
fn watch<W: tasks_application::WatchService>(
    runtime: &tokio::runtime::Runtime,
    service: &mut W,
    settings: &tasks_watch::Settings,
    catalog: &presentation::Catalog,
) -> ExitCode {
    let code = runtime.block_on(async {
        let stop = match interrupted() {
            Ok(stop) => stop,
            Err(e) => {
                eprintln!("{}: cannot listen for signals: {e}", tasks_cli::PROG);
                return 1;
            }
        };
        // Stdin is not watched: the VS Code extension starts the watch
        // with stdin from /dev/null, and Go's watch ignores it too.
        let mut ticks = tasks_watch::Interval::every(tasks_watch::DEBOUNCE);
        tasks_watch::run(
            service,
            settings,
            &mut ticks,
            stop,
            catalog,
            &mut io::stdout().lock(),
            &mut io::stderr().lock(),
        )
        .await
    });
    ExitCode::from(code)
}

/// `taskloom manage`: `taskloom mcp` registered in the agents'
/// configurations under `home`.
fn manage(
    words: Vec<String>,
    format: presentation::Format,
    home: Option<PathBuf>,
    catalog: &presentation::Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let settings = registration_cli::Settings {
        format,
        stdout_is_terminal: io::stdout().is_terminal(),
        prog: tasks_cli::PROG,
    };
    let configs = agent_configs::AgentConfigFiles {
        home,
        path: std::env::var_os("PATH"),
        waits: file_lock::Waits::DEFAULT,
    };
    let mut registrations = registration_application::Registrations::taskloom(configs);
    registration_cli::run(words, &settings, &mut registrations, catalog, out, err)
}

/// `taskloom run`: the task handed to the `ctxloom` on PATH.
fn launch<S: tasks_application::TaskService>(
    runtime: &tokio::runtime::Runtime,
    service: &mut S,
    group: tasks_run::Group,
    settings: &tasks_run::Settings,
    catalog: &presentation::Catalog,
) -> ExitCode {
    let mut ctxloom = tasks_run::Ctxloom {
        path: std::env::var_os("PATH"),
    };
    ExitCode::from(runtime.block_on(tasks_run::run(
        service,
        group,
        settings,
        &mut ctxloom,
        &mut io::stdin().lock(),
        catalog,
        &mut io::stderr().lock(),
    )))
}

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("{}: cannot start: {e}", tasks_cli::PROG);
            return ExitCode::FAILURE;
        }
    };
    let catalog = match catalog() {
        Ok(catalog) => catalog,
        Err(e) => {
            eprintln!("{}: {e}", tasks_cli::PROG);
            return ExitCode::FAILURE;
        }
    };
    let env = Environment {
        home: std::env::var_os("HOME").map(PathBuf::from),
        cwd: std::env::current_dir().map_err(|e| e.to_string()),
        root: std::env::var_os("CTXLOOM_ROOT").map(PathBuf::from),
        project_id: std::env::var("CTXLOOM_PROJECT_ID").ok(),
    };
    let go_logs = GoTaskLogDir::in_home(env.home.as_deref());
    let mut projects = Projects::new(
        ProjectRegistry::new(env.clone(), HarpNames),
        SqliteStores::default(),
    );
    let mut service = service(&env, utf8_vars()).with_go_logs(go_logs);
    let context = Context {
        session: std::env::var("CTXLOOM_SESSION_HARP").unwrap_or_default(),
        stdout_is_terminal: io::stdout().is_terminal(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        // True of every lock file taskloom takes, each through file-lock:
        // the project registry's `index.yaml.lock`, and `manage`'s home
        // sidecar for the agent configuration it rewrites, with the reap
        // interlock held shared. It takes none on a Go task log (the
        // importer reads without one); its store is SQLite's to lock.
        lock_protocol: file_lock::LOCK_PROTOCOL,
    };
    // `mcp`, `watch` and `run` run once the command line has been read, on the
    // same runtime: the CLI's mounted commands are synchronous.
    let mut after = None;
    let mounted = |group,
                   flags: tasks_cli::Mounted,
                   catalog: &presentation::Catalog,
                   out: &mut dyn Write,
                   err: &mut dyn Write| match group {
        Mounted::Project(group) => {
            let settings = project_cli::Settings {
                format: flags.format,
                project: flags.project,
                prog: tasks_cli::PROG,
            };
            project_cli::run(group, &settings, catalog, &mut projects, out, err)
        }
        Mounted::Manage(registration_cli::Group::Manage { words }) => {
            manage(words, flags.format, env.home.clone(), catalog, out, err)
        }
        Mounted::Mcp(tasks_mcp::Group::Mcp) => {
            after = Some(After::Mcp(flags.project));
            0
        }
        Mounted::Run(group) => {
            let settings = tasks_run::Settings {
                format: flags.format,
                project: flags.project,
                prog: tasks_cli::PROG,
                interactive: io::stdin().is_terminal() && io::stdout().is_terminal(),
            };
            after = Some(After::Run(group, settings));
            0
        }
        Mounted::Watch(tasks_watch::Group::Watch) => {
            after = Some(After::Watch(tasks_watch::Settings {
                format: flags.format,
                project: flags.project,
                prog: tasks_cli::PROG,
            }));
            0
        }
    };
    let code = runtime.block_on(tasks_cli::run_mounted::<_, Mounted>(
        std::env::args_os(),
        &context,
        &catalog,
        &mut service,
        mounted,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ));
    match after {
        Some(After::Watch(settings)) => watch(&runtime, &mut service, &settings, &catalog),
        Some(After::Run(group, settings)) => {
            launch(&runtime, &mut service, group, &settings, &catalog)
        }
        Some(After::Mcp(project)) => {
            let settings = tasks_mcp::Settings {
                project,
                session: context.session,
                name: tasks_cli::PROG.to_string(),
                version: context.version,
            };
            serve_mcp(runtime, env, settings, catalog)
        }
        None => ExitCode::from(code),
    }
}

/// The environment's variables whose names and values are UTF-8.
fn utf8_vars() -> Vec<(String, String)> {
    std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .collect()
}

#[cfg(test)]
#[path = "main.test.rs"]
mod tests;
