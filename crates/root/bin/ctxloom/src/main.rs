//! ctxloom: the composition root. It builds the catalog from every crate's
//! `EN_US`, reads what it needs from the process (its environment, once;
//! stdout's terminal, the version, the lock protocol), and hands the command
//! line the wiring. It
//! holds no logic of its own.

use std::io::{self, IsTerminal};
use std::process::ExitCode;
use std::sync::Arc;

use ctxloom_cli::{Catalog, Context, Extension, Services};
use process_env::EnvSnapshot;

/// The binary's en-US catalog: every crate's own.
fn catalog() -> Result<Catalog, angzarr_presentation::CatalogError> {
    let mut sources = ctxloom_wiring::catalog_sources();
    sources.extend([
        ctxloom_cli::EN_US,
        session_cli::EN_US,
        hook_cli::EN_US,
        angzarr_yaml::EN_US,
        library::EN_US,
        library_application::EN_US,
        deps::EN_US,
        deps_application::EN_US,
        content_fs::EN_US,
        config_files::EN_US,
        editor_process::EN_US,
        surface_ports::EN_US,
        setup_application::EN_US,
        cache_files::EN_US,
        init_application::EN_US,
        reclaim_application::EN_US,
        setup_cli::EN_US,
    ]);
    Catalog::en_us(&sources)
}

/// What the commands call, over the default wiring.
struct Wired {
    /// The environment the process was launched in, read once: the test
    /// commands open stores and plant sessions under it.
    #[cfg(feature = "test-support")]
    env: Arc<EnvSnapshot>,
    /// Every command of the program, wired when one first needs the home.
    library: ctxloom_wiring::Commands,
}

impl Wired {
    fn new(env: Arc<EnvSnapshot>) -> Wired {
        Wired {
            library: ctxloom_wiring::Commands::new(Arc::clone(&env)),
            #[cfg(feature = "test-support")]
            env,
        }
    }
}

impl Services for Wired {
    fn extension(&mut self) -> &mut dyn Extension {
        &mut self.library
    }

    #[cfg(feature = "test-support")]
    fn session_name(&mut self) -> String {
        ctxloom_wiring::session_name()
    }

    #[cfg(feature = "test-support")]
    fn run_id(
        &mut self,
        session: &str,
        incarnation: u64,
    ) -> Result<String, Box<dyn angzarr_diagnostic::Diagnostic>> {
        match ctxloom_wiring::run_id(session, incarnation) {
            Ok(id) => Ok(id.to_string()),
            Err(e) => Err(Box::new(e)),
        }
    }

    #[cfg(feature = "test-support")]
    fn open_store(
        &mut self,
        project: &str,
    ) -> Result<String, Box<dyn angzarr_diagnostic::Diagnostic>> {
        let opened =
            ctxloom_wiring::Wiring::new(Arc::clone(&self.env)).and_then(|w| w.open_store(project));
        match opened {
            Ok(path) => Ok(path.display().to_string()),
            Err(e) => Err(Box::new(e)),
        }
    }

    #[cfg(feature = "test-support")]
    fn seed_session(
        &mut self,
        seed: &ctxloom_cli::SeedArgs,
    ) -> Result<String, Box<dyn angzarr_diagnostic::Diagnostic>> {
        let native = match (&seed.native_id, &seed.native_transcript) {
            (None, None) => None,
            (id, transcript) => Some((
                id.clone().unwrap_or_default(),
                transcript.clone().unwrap_or_default(),
            )),
        };
        let planted = ctxloom_wiring::Wiring::new(Arc::clone(&self.env)).and_then(|w| {
            w.seed_session(&ctxloom_wiring::Seed {
                harp: seed.harp.clone(),
                project: seed.project.clone(),
                started_ago: seed.started_ago,
                ended_ago: seed.ended_ago,
                lock_free: seed.lock == "free",
                transcript: seed.transcript,
                keep: seed.keep,
                oneshot: seed.oneshot,
                native,
                other_files: seed.other_file.clone(),
            })
        });
        planted.map_err(|e| Box::new(e) as _)
    }

    #[cfg(feature = "test-support")]
    fn diagnostic_samples(&self) -> Vec<Box<dyn angzarr_diagnostic::Diagnostic>> {
        let mut all = ctxloom_store_sqlite::diagnostic_samples();
        all.extend(project_dir::diagnostic_samples());
        all.extend(duration_text::diagnostic_samples());
        all.extend(ctxloom_wiring::diagnostic_samples());
        all.extend(library::diagnostic_samples());
        all.extend(library_application::diagnostic_samples());
        all.extend(deps::diagnostic_samples());
        all.extend(deps_application::diagnostic_samples());
        all.extend(content_fs::diagnostic_samples());
        all.extend(config_files::diagnostic_samples());
        all.extend(editor_process::diagnostic_samples());
        all.extend(surface_ports::diagnostic_samples());
        all.extend(setup_application::diagnostic_samples());
        all.extend(session_cli::diagnostic_samples());
        all.extend(hook_cli::diagnostic_samples());
        all.extend(session_files::diagnostic_samples());
        all.extend(sessions_application::diagnostic_samples());
        all.extend(sessions::diagnostic_samples());
        all.extend(cache_files::diagnostic_samples());
        all.extend(init_application::diagnostic_samples());
        all.extend(reclaim_application::diagnostic_samples());
        all.extend(setup_cli::diagnostic_samples());
        all.extend(isolation_application::diagnostic_samples());
        all.extend(worktree_git::diagnostic_samples());
        all.extend(tasks_ports::diagnostic_samples());
        all.extend(container_cli::diagnostic_samples());
        all.extend(scheduler::diagnostic_samples());
        all.extend(scheduler_loop::diagnostic_samples());
        all
    }
}

fn main() -> ExitCode {
    let catalog = match catalog() {
        Ok(catalog) => catalog,
        Err(e) => {
            eprintln!("{}: {e}", ctxloom_cli::PROG);
            return ExitCode::FAILURE;
        }
    };
    // The process's environment, read once: everything below is handed it.
    let env = Arc::new(EnvSnapshot::from_process());
    let context = Context {
        version: env!("CARGO_PKG_VERSION").to_string(),
        lock_protocol: file_lock::LOCK_PROTOCOL,
        stdout_is_terminal: io::stdout().is_terminal(),
    };
    let (stdout, stderr) = (io::stdout(), io::stderr());
    ExitCode::from(ctxloom_cli::run(
        std::env::args_os(),
        &context,
        &catalog,
        &mut Wired::new(env),
        &mut stdout.lock(),
        &mut stderr.lock(),
    ))
}

#[cfg(test)]
#[path = "main.test.rs"]
mod tests;
