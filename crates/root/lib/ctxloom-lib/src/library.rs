//! The library commands (S1) wired: the content and config files over the
//! process's filesystem, the application over them, and the driving adapter
//! joined to the command line as an extension.

use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use angzarr_presentation::Catalog;
use companion_probe::ProcessProbe;
use config_files::{find_app_dir, ConfigFilesFs, ScaffoldFs, Surroundings};
use content_fs::{ContentFs, UserFilesFs};
use ctxloom_cli::{Call, Extension, Failure};
use ctxloom_ports::Executables;
use deps_ports::RemoteLibrary;
use editor_process::ProcessEditor;
use file_system::{Fs, Kind};
use library_application::{Library, WritePorts};

use crate::{LibError, Wiring};

/// Where a program is on the `PATH`, by the filesystem seam.
struct PathSearch {
    fs: Arc<dyn Fs>,
    dirs: Vec<PathBuf>,
}

impl Executables for PathSearch {
    fn find(&self, name: &str) -> Option<String> {
        self.dirs.iter().find_map(|dir| {
            let candidate = dir.join(name);
            let meta = self.fs.metadata(&candidate).ok()?;
            (meta.kind == Kind::File && meta.executable).then(|| candidate.display().to_string())
        })
    }
}

/// The library's commands as an extension of the command line. The library is
/// wired when a command first needs it, so a command that needs nothing of the
/// home (`version`) does not.
#[derive(Default)]
pub struct LibraryCommands {
    library: Option<Library>,
    /// What the library reads of the remotes, where the program has them.
    remotes: Option<Rc<dyn RemoteLibrary>>,
}

impl LibraryCommands {
    pub fn new() -> LibraryCommands {
        LibraryCommands::default()
    }

    /// The library reading the remotes too.
    pub fn with_remotes(mut self, remotes: Rc<dyn RemoteLibrary>) -> LibraryCommands {
        self.remotes = Some(remotes);
        self
    }

    fn wired(&mut self) -> Result<&Library, LibError> {
        if self.library.is_none() {
            self.library = Some(build(self.remotes.clone())?);
        }
        Ok(self
            .library
            .as_ref()
            .unwrap_or_else(|| unreachable!("just built")))
    }
}

fn build(remotes: Option<Rc<dyn RemoteLibrary>>) -> Result<Library, LibError> {
    let wiring = Wiring::from_process()?;
    let fs = wiring.fs();
    let home = wiring.home().to_path_buf();
    let around = Surroundings {
        root_env: std::env::var("CTXLOOM_ROOT").ok(),
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        home: home.clone(),
        temp_root: std::env::temp_dir(),
    };
    let app = find_app_dir(fs.as_ref(), &around);
    let content = ContentFs::new(Arc::clone(&fs), &app.path);
    let files = ConfigFilesFs::new(Arc::clone(&fs), app, &home);
    let dirs = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    let binaries = PathSearch {
        fs: Arc::clone(&fs),
        dirs,
    };
    let environ = std::env::vars().collect();
    let ports = WritePorts {
        user: Arc::new(UserFilesFs::new(Arc::clone(&fs))),
        scaffold: Arc::new(ScaffoldFs::new(Arc::clone(&fs), &around.cwd)),
        editor: Arc::new(ProcessEditor::new(Arc::clone(&fs), &around.temp_root)),
        probe: Arc::new(ProcessProbe::standard()),
    };
    let library = Library::new(
        Arc::new(content),
        Arc::new(files),
        Arc::new(binaries),
        environ,
    )
    .with_write_ports(ports);
    Ok(match remotes {
        Some(remotes) => library.with_remotes(remotes),
        None => library,
    })
}

impl Extension for LibraryCommands {
    fn nouns(&self) -> Vec<clap::Command> {
        library_cli::nouns()
    }

    fn run(
        &mut self,
        noun: &str,
        call: &Call<'_>,
        catalog: &Catalog,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<u8, Failure> {
        let library = self.wired().map_err(|e| Failure {
            cause: Box::new(e),
            exit: 1,
        })?;
        let inner = library_cli::Call {
            matches: call.matches,
            explicit_format: call.explicit_format,
            default_format: call.default_format,
            stdout_is_terminal: call.stdout_is_terminal,
            degraded: call.degraded,
            no_companions: call.no_companions,
            config_set: call.config_set.clone(),
        };
        library_cli::run(library, noun, &inner, catalog, out, err).map_err(|f| Failure {
            cause: f.cause,
            exit: f.exit,
        })
    }
}
