//! taskloom: per-project task tracking. The composition root: it reads the
//! process's environment, wires the tasks use cases to their adapters (the
//! project registry, harp names, the SQLite task store, the system's
//! clock, ids, timer and randomness, and Go taskloom's logs to import),
//! mounts the `project` group beside the task commands, and hands the
//! command line to the CLI. It holds no logic of its own.

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use go_task_log::GoTaskLogDir;
use harp_names::HarpNames;
use project_application::Projects;
use project_registry::{Environment, ProjectRegistry};
use system::System;
use task_store_sqlite::SqliteStores;
use tasks_application::Service;
use tasks_cli::Context;

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
    let env = Environment {
        home: std::env::var_os("HOME").map(PathBuf::from),
        cwd: std::env::current_dir().map_err(|e| e.to_string()),
        root: std::env::var_os("CTXLOOM_ROOT").map(PathBuf::from),
        project_id: std::env::var("CTXLOOM_PROJECT_ID").ok(),
    };
    let go_logs = GoTaskLogDir::in_home(env.home.as_deref());
    let mut projects = Projects::new(ProjectRegistry::new(env.clone(), HarpNames));
    let identity = ProjectRegistry::new(env, HarpNames);
    let mut service = Service::new(identity, SqliteStores::default(), HarpNames, System, System)
        .with_go_logs(go_logs);
    let context = Context {
        session: std::env::var("CTXLOOM_SESSION_HARP").unwrap_or_default(),
        stdout_is_terminal: io::stdout().is_terminal(),
    };
    let project_group =
        |group, flags: tasks_cli::Mounted, out: &mut dyn Write, err: &mut dyn Write| {
            let settings = project_cli::Settings {
                json: flags.json,
                project: flags.project,
                prog: tasks_cli::PROG,
            };
            project_cli::run(group, &settings, &mut projects, out, err)
        };
    let code = runtime.block_on(tasks_cli::run_mounted::<_, project_cli::Group>(
        std::env::args_os(),
        &context,
        &mut service,
        project_group,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ));
    ExitCode::from(code)
}

#[cfg(test)]
#[path = "main.test.rs"]
mod tests;
