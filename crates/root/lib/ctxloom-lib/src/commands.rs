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
use content_fs::ContentFs;
use ctxloom_cli::{Call, Extension, Failure};
use deps::forge;
use deps::registry::Registry;
use deps_application::{Deps, ProjectRefresh, Refresh, RefreshEngine, RefusedKind};
use deps_files::ProjectFiles;
use deps_ports::DepsFiles;
use forge_github::GitHub;
use remote_git::GitRemotes;
use setup_application::{MaterializeRequest, Puller, Setup};
use surface_files::SurfaceFiles;
use surface_ports::Registry as Engines;

use crate::setup::{SetupUseCases, NOUNS as SETUP_NOUNS};
use crate::{LibError, LibraryCommands, Wiring};

/// The nouns the dependency commands own outright.
const OWN_NOUNS: [&str; 2] = ["remote", "deps"];

/// The dependency use cases over the process's home, working directory and
/// project.
fn build_deps() -> Result<Deps, LibError> {
    let wiring = Wiring::from_process()?;
    let fs = wiring.fs();
    let (_, app) = wiring.locate();
    let files = Arc::new(ProjectFiles::new(Arc::clone(&fs), app.path.clone()));
    Ok(Deps::new(
        Arc::new(
            GitRemotes::new(Arc::clone(&fs), app.path.join("cache/repos")).with_credentials(
                Arc::new(|name| std::env::var(name).ok()),
                token_vars(Arc::clone(&files)),
            ),
        ),
        files,
        Arc::new(ContentFs::new(fs, &app.path)),
    )
    .with_forge(Arc::new(GitHub::from_process())))
}

/// The nouns the surface commands own outright.
const SURFACE_NOUNS: [&str; 3] = ["manage", "materialize", "util"];

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

/// The project refresh a consented pull ends with, through `materialize`:
/// into the project's directory, with the context of an engine whose context
/// rides the launch left alone. Without consent it is a dry run, and answers
/// only what would change.
struct SetupRefresh<'a> {
    setup: &'a Setup,
    flags: library_application::Flags,
}

impl ProjectRefresh for SetupRefresh<'_> {
    fn refresh(&self, write: bool) -> Result<Refresh, Box<dyn angzarr_diagnostic::Diagnostic>> {
        let done = self
            .setup
            .materialize(&MaterializeRequest {
                yes: true,
                dry_run: !write,
                leave_launch_context: true,
                flags: self.flags.clone(),
                ..MaterializeRequest::default()
            })
            .map_err(|e| Box::new(e) as Box<dyn angzarr_diagnostic::Diagnostic>)?;
        let result = done.value;
        if write {
            if let Some(first) = result.failures.into_iter().next() {
                return Err(Box::new(first));
            }
            return Ok(Refresh {
                target: result.target,
                engines: Vec::new(),
            });
        }
        Ok(Refresh {
            target: result.target,
            engines: result
                .engines
                .into_iter()
                .map(|e| RefreshEngine {
                    engine: e.engine,
                    wrote: e.wrote,
                    released: e.released,
                    refused: e
                        .refused
                        .into_iter()
                        .map(|r| RefusedKind {
                            kind: r.kind,
                            cause: Box::new(r.cause),
                        })
                        .collect(),
                })
                .collect(),
        })
    }
}

/// The engines composed in: claude first (the default), then the mock and its
/// home-keyed variant.
pub fn engines() -> Engines {
    Engines::new(vec![
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
    let (_, app) = wiring.locate();
    let project_dir = match app.source {
        ctxloom_ports::Source::Project => {
            app.path.parent().map_or_else(|| cwd.clone(), PathBuf::from)
        }
        ctxloom_ports::Source::Home => cwd.clone(),
    };
    let files = Arc::new(SurfaceFiles::new(
        Arc::clone(&fs),
        home.join(".ctxloom/records"),
    ));
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

/// The variables a clone address's token is read from: the forge the
/// registry (or the built-in rules) binds the address to says so, the same
/// answer the API calls get. A registry that cannot be read is an empty one
/// here: the clone then goes with the user's own git credentials, and the
/// commands that need the registry report it.
fn token_vars(files: Arc<ProjectFiles>) -> remote_git::TokenVars {
    Arc::new(move |url| {
        let registry = files
            .read_registry()
            .ok()
            .flatten()
            .and_then(|text| Registry::decode(&text, &files.registry_path()).ok())
            .unwrap_or_else(Registry::empty);
        forge::resolve(url, "", &registry).token_vars()
    })
}

/// Every command of the program, wired when one first needs the home.
#[derive(Default)]
pub struct Commands {
    wired: Option<Wired>,
}

struct Wired {
    deps: Rc<Deps>,
    library: LibraryCommands,
    surface: Setup,
    setup: SetupUseCases,
}

impl Commands {
    pub fn new() -> Commands {
        Commands::default()
    }

    /// The engines' callbacks, over the setup use cases.
    pub fn hooks(&mut self) -> Result<Hooks<'_>, LibError> {
        Ok(Hooks(&self.wired()?.surface))
    }

    fn wired(&mut self) -> Result<&mut Wired, LibError> {
        if self.wired.is_none() {
            let deps = Rc::new(build_deps()?);
            let library = LibraryCommands::new().with_remotes(deps.clone());
            let surface = build_setup(deps.clone())?;
            let setup = SetupUseCases::build(deps.clone())?;
            self.wired = Some(Wired {
                deps,
                library,
                setup,
                surface,
            });
        }
        Ok(self
            .wired
            .as_mut()
            .unwrap_or_else(|| unreachable!("just built")))
    }
}

impl Commands {
    /// A setup command: `clean`, `doctor` or `init`.
    fn setup(
        &mut self,
        noun: &str,
        call: &Call<'_>,
        catalog: &Catalog,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<u8, Failure> {
        let wired = self.wired().map_err(|e| Failure::new(e, 1))?;
        let uses = setup_cli::UseCases {
            cleaner: &wired.setup.cleaner,
            doctor: &wired.setup.doctor,
            init: &wired.setup.init,
        };
        setup_cli::run(&uses, noun, call, catalog, out, err)
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
        nouns.extend(surface_cli::nouns());
        nouns.extend(setup_cli::nouns());
        nouns.extend(container_cli::nouns());
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
        if noun == "container" {
            return crate::isolation::container(call, catalog, out);
        }
        if SETUP_NOUNS.contains(&noun) {
            return self.setup(noun, call, catalog, out, err);
        }
        let verb = call
            .matches
            .subcommand_matches(noun)
            .and_then(|m| m.subcommand_name())
            .unwrap_or_default()
            .to_string();
        let wired = self.wired().map_err(|e| Failure::new(e, 1))?;
        if SURFACE_NOUNS.contains(&noun) {
            return surface_cli::run(&wired.surface, noun, call, catalog, out, err);
        }
        let ours =
            OWN_NOUNS.contains(&noun) || (noun == "bundle" && deps_cli::is_bundle_verb(&verb));
        if !ours {
            return wired.library.run(noun, call, catalog, out, err);
        }
        let refresh = SetupRefresh {
            setup: &wired.surface,
            flags: library_application::Flags {
                degraded: call.degraded,
                config_set: call.config_set.clone(),
                no_companions: call.no_companions,
            },
        };
        deps_cli::run(&wired.deps, &refresh, noun, call, catalog, out, err)
    }
}

#[cfg(test)]
#[path = "commands.test.rs"]
mod tests;
