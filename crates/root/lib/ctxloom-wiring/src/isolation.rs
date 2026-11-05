//! The isolation commands wired (S11): the worktree commands over the
//! sessions' locks and git, and `container` over the machine's runtimes, the
//! engines, the registered companions and the project's files.

use std::rc::Rc;

use angzarr_presentation::Catalog;
use companion_probe::ProcessProbe;
use container_runtime::HostRuntimes;
use content_fs::UserFilesFs;
use ctxloom_cli::{Call, Failure};
use isolation_application::Containers;
use isolation_ports::{Backends, Companions, Failure as PortFailure, Tooling};
use library_application::{Flags, Library};

use crate::{LibError, Wiring};

/// The engines `container` may be asked about, and the one a bare command
/// means: the configured default when it is an engine, else the first.
struct Engines {
    names: Vec<String>,
    library: Rc<Library>,
    flags: Flags,
}

impl Backends for Engines {
    fn names(&self) -> Vec<String> {
        self.names.clone()
    }

    fn default_backend(&self) -> Option<String> {
        let configured = self
            .library
            .llm_list(&self.flags)
            .result
            .ok()
            .and_then(|rows| {
                rows.into_iter()
                    .find(|row| row.default)
                    .map(|row| row.label)
                    .filter(|label| self.names.contains(label))
            });
        configured.or_else(|| self.names.first().cloned())
    }
}

/// The registered companions' declarations: only a REGISTERED companion is
/// ever run; one on the PATH nobody registered declares nothing.
struct Registered {
    library: Rc<Library>,
    flags: Flags,
}

impl Companions for Registered {
    fn tooling(&self) -> Result<Vec<Tooling>, PortFailure> {
        let probe = ProcessProbe::standard();
        let Ok(rows) = self.library.companion_list(&self.flags).result else {
            return Ok(Vec::new());
        };
        Ok(rows
            .into_iter()
            .filter(|row| row.resolves)
            .filter_map(|row| {
                let text = probe.loadout(&row.path).ok()?;
                let tooling = declared(&text)?;
                Some(Tooling {
                    source: row.name,
                    content: tooling,
                })
            })
            .collect())
    }
}

/// The `init.tooling` of a loadout document.
fn declared(text: &str) -> Option<String> {
    let doc = angzarr_yaml::parse(text).ok()?;
    let tooling = doc.get("init")?.get("tooling")?.as_str()?.to_string();
    (!tooling.is_empty()).then_some(tooling)
}

/// Run `ctxloom container ...`.
pub(crate) fn container(
    call: &Call<'_>,
    _catalog: &Catalog,
    out: &mut dyn std::io::Write,
) -> Result<u8, Failure> {
    let wired = |e: LibError| Failure {
        cause: Box::new(e),
        exit: 1,
    };
    let wiring = Wiring::from_process().map_err(wired)?;
    let library = Rc::new(crate::library::build(None).map_err(wired)?);
    let flags = call.globals.clone();
    let containers = Containers::new(
        HostRuntimes::from_process(wiring.fs()),
        Engines {
            names: crate::commands::engines().names(),
            library: Rc::clone(&library),
            flags: flags.clone(),
        },
        Registered {
            library: Rc::clone(&library),
            flags,
        },
        UserFilesFs::new(wiring.fs()),
    );
    container_cli::run(&containers, call, out)
}
