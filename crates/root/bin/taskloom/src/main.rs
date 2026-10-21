//! taskloom: per-project task tracking. The composition root: it reads the
//! process's environment, wires the tasks use cases to their adapters (the
//! project registry, harp names, the SQLite task store, the system's
//! clock, ids, timer and randomness, and Go taskloom's logs to import),
//! mounts the `project` group and `mcp` beside the task commands, and
//! hands the command line to the CLI. `mcp` serves the task tools over
//! stdio, each call on a service of its own. It holds no logic of its own.

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
use tasks_application::Service;
use tasks_cli::Context;

/// Every crate's en-US catalog the binary's diagnostics come from. annals'
/// own crates export none yet, so their errors print their English.
fn catalog() -> Result<presentation::Catalog, presentation::CatalogError> {
    tasks_cli::catalog(&[
        project_cli::EN_US,
        project_application::EN_US,
        project_registry::EN_US,
        task_store_sqlite::EN_US,
        go_task_log::EN_US,
        tasks_mcp::EN_US,
    ])
}

/// The commands mounted beside the task commands.
#[derive(clap::Subcommand, Debug)]
enum Mounted {
    #[command(flatten)]
    Project(project_cli::Group),
    #[command(flatten)]
    Mcp(tasks_mcp::Group),
}

/// The tasks use cases over the real adapters, as one call or command
/// needs them.
fn service(
    env: &Environment,
) -> Service<ProjectRegistry<HarpNames>, SqliteStores, HarpNames, System, System> {
    Service::new(
        ProjectRegistry::new(env.clone(), HarpNames),
        SqliteStores::default(),
        HarpNames,
        System,
        System,
    )
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
    let tools = tasks_mcp::TaskTools::new(move || service(&env), settings, Arc::clone(&catalog));
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

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
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
    let mut projects = Projects::new(ProjectRegistry::new(env.clone(), HarpNames));
    let mut service = service(&env).with_go_logs(go_logs);
    let context = Context {
        session: std::env::var("CTXLOOM_SESSION_HARP").unwrap_or_default(),
        stdout_is_terminal: io::stdout().is_terminal(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        lock_protocol: project_registry::LOCK_PROTOCOL,
    };
    // `mcp` is served once the command line has been read, on the same
    // runtime: the CLI's mounted commands are synchronous.
    let mut mcp = None;
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
        Mounted::Mcp(tasks_mcp::Group::Mcp) => {
            mcp = Some(flags.project);
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
    match mcp {
        Some(project) => {
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

#[cfg(test)]
#[path = "main.test.rs"]
mod tests;
