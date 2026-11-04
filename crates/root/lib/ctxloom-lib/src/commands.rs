//! The program's commands as one extension of the command line: the
//! library's (S1) and the remote and dependency ones (S3). They share a noun,
//! `bundle` (the library inspects bundles; `push` and `move` publish and
//! relocate them), and the library reads the remotes through the dependency
//! use cases, so both are built together.

use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use angzarr_presentation::Catalog;
use clap::Command;
use config_files::{find_app_dir, Surroundings};
use content_fs::ContentFs;
use ctxloom_cli::{Call, Extension, Failure};
use deps_application::Deps;
use deps_files::ProjectFiles;
use forge_github::GitHub;
use remote_git::GitRemotes;
use setup_application::{Puller, Setup};
use surface_files::SurfaceFiles;
use surface_ports::Registry;

use crate::{LibError, LibraryCommands, Wiring};

/// The nouns the dependency commands own outright.
const OWN_NOUNS: [&str; 2] = ["remote", "deps"];

/// The dependency use cases over the process's home, working directory and
/// project.
fn build_deps() -> Result<Deps, LibError> {
    let wiring = Wiring::from_process()?;
    let fs = wiring.fs();
    let around = Surroundings {
        root_env: std::env::var("CTXLOOM_ROOT").ok(),
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        home: wiring.home().to_path_buf(),
        temp_root: std::env::temp_dir(),
    };
    let app = find_app_dir(fs.as_ref(), &around);
    Ok(Deps::new(
        Arc::new(GitRemotes::new(
            Arc::clone(&fs),
            app.path.join("cache/repos"),
        )),
        Arc::new(ProjectFiles::new(Arc::clone(&fs), app.path.clone())),
        Arc::new(ContentFs::new(fs, &app.path)),
    )
    .with_forge(Arc::new(GitHub::from_process())))
}

/// The nouns the setup commands own outright.
const SETUP_NOUNS: [&str; 3] = ["manage", "materialize", "util"];

/// The dependency pull `manage install` ends with.
struct DepsPuller(Rc<Deps>);

impl Puller for DepsPuller {
    fn pull(&self) -> Result<u64, String> {
        let pulled = self.0.pull(false, true);
        match pulled.result {
            Ok(report) if report.errors == 0 => Ok(report.installed),
            Ok(report) => Err(format!("{} could not be installed", report.errors)),
            Err(e) => Err(angzarr_diagnostic::Diagnostic::code(&e).to_string()),
        }
    }
}

/// The engines composed in: claude first (the default), then the mock and its
/// home-keyed variant.
pub fn engines() -> Registry {
    Registry::new(vec![
        Box::new(engine_claude::Claude::new()),
        Box::new(engine_mock::Mock::standard()),
        Box::new(engine_mock::Mock::home_keyed()),
    ])
}

/// The setup use cases over the process's home, working directory and
/// project.
fn build_setup(deps: Rc<Deps>) -> Result<Setup, LibError> {
    let wiring = Wiring::from_process()?;
    let fs = wiring.fs();
    let home = wiring.home().to_path_buf();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let around = Surroundings {
        root_env: std::env::var("CTXLOOM_ROOT").ok(),
        cwd: cwd.clone(),
        home: home.clone(),
        temp_root: std::env::temp_dir(),
    };
    let app = find_app_dir(fs.as_ref(), &around);
    let project_dir = match app.source {
        ctxloom_ports::Source::Project => {
            app.path.parent().map_or_else(|| cwd.clone(), PathBuf::from)
        }
        ctxloom_ports::Source::Home => cwd.clone(),
    };
    let files = Arc::new(SurfaceFiles::new(home.join(".ctxloom/records")));
    let library = crate::library::build(Some(deps.clone()))?;
    Ok(Setup::new(
        library,
        engines(),
        files.clone(),
        Arc::new(config_files::ScaffoldFs::new(fs, &project_dir)),
        project_dir.display().to_string(),
        cwd.display().to_string(),
    )
    .with_foreign(files)
    .with_puller(Rc::new(DepsPuller(deps))))
}

/// The engines' callbacks that answer through the setup use cases.
pub struct Hooks<'a>(&'a Setup);

impl hook_cli::HookService for Hooks<'_> {
    fn session_start(
        &self,
        engine: &str,
        payload: &str,
    ) -> Result<String, Box<dyn angzarr_diagnostic::Diagnostic>> {
        self.0
            .session_start(engine, payload)
            .map_err(|e| Box::new(e) as _)
    }

    fn skill_mates(
        &self,
        engine: &str,
        payload: &str,
    ) -> Result<String, Box<dyn angzarr_diagnostic::Diagnostic>> {
        self.0
            .skill_mates(engine, payload)
            .map_err(|e| Box::new(e) as _)
    }
}

/// Every command of the program, wired when one first needs the home.
#[derive(Default)]
pub struct Commands {
    wired: Option<Wired>,
}

struct Wired {
    deps: Rc<Deps>,
    library: LibraryCommands,
    setup: Setup,
}

impl Commands {
    pub fn new() -> Commands {
        Commands::default()
    }

    /// The engines' callbacks, over the setup use cases.
    pub fn hooks(&mut self) -> Result<Hooks<'_>, LibError> {
        Ok(Hooks(&self.wired()?.setup))
    }

    fn wired(&mut self) -> Result<&mut Wired, LibError> {
        if self.wired.is_none() {
            let deps = Rc::new(build_deps()?);
            let library = LibraryCommands::new().with_remotes(deps.clone());
            let setup = build_setup(deps.clone())?;
            self.wired = Some(Wired {
                deps,
                library,
                setup,
            });
        }
        Ok(self
            .wired
            .as_mut()
            .unwrap_or_else(|| unreachable!("just built")))
    }
}

impl Extension for Commands {
    fn nouns(&self) -> Vec<Command> {
        let mut nouns: Vec<Command> = library_cli::nouns()
            .into_iter()
            .map(|noun| {
                if noun.get_name() != "bundle" {
                    return noun;
                }
                deps_cli::bundle_verbs()
                    .into_iter()
                    .fold(noun, |noun, verb| noun.subcommand(verb))
            })
            .collect();
        nouns.extend(deps_cli::nouns());
        nouns.extend(setup_cli::nouns());
        nouns
    }

    fn run(
        &mut self,
        noun: &str,
        call: &Call<'_>,
        catalog: &Catalog,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<u8, Failure> {
        let verb = call
            .matches
            .subcommand_matches(noun)
            .and_then(|m| m.subcommand_name())
            .unwrap_or_default()
            .to_string();
        let wired = self.wired().map_err(|e| Failure {
            cause: Box::new(e),
            exit: 1,
        })?;
        if SETUP_NOUNS.contains(&noun) {
            let inner = setup_cli::Call {
                matches: call.matches,
                explicit_format: call.explicit_format,
                default_format: call.default_format,
                stdout_is_terminal: call.stdout_is_terminal,
                degraded: call.degraded,
                no_companions: call.no_companions,
                config_set: call.config_set.clone(),
            };
            return setup_cli::run(&wired.setup, noun, &inner, catalog, out, err).map_err(|f| {
                Failure {
                    cause: f.cause,
                    exit: f.exit,
                }
            });
        }
        let ours =
            OWN_NOUNS.contains(&noun) || (noun == "bundle" && deps_cli::is_bundle_verb(&verb));
        if !ours {
            return wired.library.run(noun, call, catalog, out, err);
        }
        let inner = deps_cli::Call {
            matches: call.matches,
            explicit_format: call.explicit_format,
            default_format: call.default_format,
            stdout_is_terminal: call.stdout_is_terminal,
        };
        deps_cli::run(&wired.deps, noun, &inner, catalog, out, err).map_err(|f| Failure {
            cause: f.cause,
            exit: f.exit,
        })
    }
}

#[cfg(test)]
#[path = "commands.test.rs"]
mod tests;
