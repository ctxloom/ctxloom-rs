//! A project opened: its configuration and its bundles, read.

use std::collections::BTreeMap;
use std::sync::Arc;

use ctxloom_ports::{ConfigFiles, ContentRepo, Executables, Source};
use library::catalog::{Catalog, Entry};
use library::tree;
use library::BundleError;

use crate::config::{self, Config, Flags};
use crate::error::{AppError, Notice};
use crate::write::WritePorts;

/// What a command came to: its answer or its error, and what it noticed.
pub struct Outcome<T> {
    pub result: Result<T, AppError>,
    pub notices: Vec<Notice>,
}

/// The library, over its ports.
pub struct Library {
    content: Arc<dyn ContentRepo>,
    files: Arc<dyn ConfigFiles>,
    binaries: Arc<dyn Executables>,
    pub(crate) environ: Vec<(String, String)>,
    pub(crate) ports: Option<WritePorts>,
}

/// A project opened.
pub struct Project {
    pub(crate) config: Config,
    pub(crate) catalog: Catalog,
    pub(crate) content: Arc<dyn ContentRepo>,
    pub(crate) files: Arc<dyn ConfigFiles>,
    pub(crate) binaries: Arc<dyn Executables>,
    pub(crate) environ: Vec<(String, String)>,
    pub(crate) ports: Option<WritePorts>,
    pub(crate) flags: Flags,
    /// The project has a bundles directory.
    pub(crate) has_root: bool,
    /// Bundles that are there and cannot be read, by name.
    pub(crate) unreadable: BTreeMap<String, BundleError>,
    pub(crate) notices: Vec<Notice>,
}

impl Library {
    /// The library over `content` and `files`; `environ` is the process's
    /// environment.
    pub fn new(
        content: Arc<dyn ContentRepo>,
        files: Arc<dyn ConfigFiles>,
        binaries: Arc<dyn Executables>,
        environ: Vec<(String, String)>,
    ) -> Library {
        Library {
            content,
            files,
            binaries,
            environ,
            ports: None,
        }
    }

    /// The library with the write side's ports: the user's files, the
    /// scaffold, an editor and a companion probe.
    pub fn with_write_ports(mut self, ports: WritePorts) -> Library {
        self.ports = Some(ports);
        self
    }

    /// Whether `CTXLOOM_DEGRADED=1` is set.
    pub fn degraded_by_environment(&self) -> bool {
        self.environ
            .iter()
            .any(|(name, value)| name == "CTXLOOM_DEGRADED" && value == "1")
    }

    /// The format the configuration prefers off a terminal
    /// (`non_tty_format`), if it says.
    pub fn preferred_format(&self, flags: &Flags) -> Option<String> {
        let mut flags = flags.clone();
        flags.degraded = true;
        let mut notices = Vec::new();
        let config = config::load(self.files.as_ref(), &self.environ, &flags, &mut notices).ok()?;
        let format = config.text("non_tty_format");
        (!format.is_empty()).then_some(format)
    }

    /// Open the project and run `command` on it.
    pub fn run<T>(
        &self,
        flags: &Flags,
        command: impl FnOnce(&mut Project) -> Result<T, AppError>,
    ) -> Outcome<T> {
        let mut flags = flags.clone();
        flags.degraded |= self.degraded_by_environment();
        let mut notices = Vec::new();
        match self.open(&flags, &mut notices) {
            Err(error) => Outcome {
                result: Err(error),
                notices,
            },
            Ok(mut project) => {
                project.notices = notices;
                let result = command(&mut project);
                Outcome {
                    result,
                    notices: project.notices,
                }
            }
        }
    }

    fn open(&self, flags: &Flags, notices: &mut Vec<Notice>) -> Result<Project, AppError> {
        let config = config::load(self.files.as_ref(), &self.environ, flags, notices)?;
        // Without a project there is the home's configuration and nothing
        // else: the home's content is not read as a project's.
        let reads_content = config.source == Source::Project;
        let has_root = reads_content && self.content.has_root().map_err(AppError::Cause)?;
        let names = if has_root {
            self.content.bundle_names().map_err(AppError::Cause)?
        } else {
            Vec::new()
        };
        let mut entries = Vec::new();
        let mut unreadable = BTreeMap::new();
        for name in names {
            let files = self.content.read_tree(&name).map_err(AppError::Cause)?;
            let manifest = self.content.manifest_path(&name);
            match tree::read_tree(&name, &files) {
                Ok(bundle) => entries.push(Entry {
                    name,
                    manifest_path: manifest,
                    bundle,
                }),
                Err(cause) => {
                    notices.push(Notice::BundleSkipped {
                        dir: manifest
                            .strip_suffix("/bundle.yaml")
                            .unwrap_or(&manifest)
                            .to_string(),
                        manifest,
                        cause: cause.clone(),
                    });
                    unreadable.insert(name, cause);
                }
            }
        }
        Ok(Project {
            config,
            catalog: Catalog::new(entries),
            content: Arc::clone(&self.content),
            files: Arc::clone(&self.files),
            binaries: Arc::clone(&self.binaries),
            environ: self.environ.clone(),
            ports: self.ports.clone(),
            flags: flags.clone(),
            has_root,
            unreadable,
            notices: Vec::new(),
        })
    }
}

impl Project {
    /// Whether the configuration came from the home alone (no project).
    pub(crate) fn no_project(&self) -> bool {
        self.config.source == Source::Home
    }

    /// What to say under an empty listing.
    pub(crate) fn listing_hint(&self) -> String {
        if self.no_project() {
            "No project here: run 'ctxloom init' to set one up with its default content."
                .to_string()
        } else {
            "Add remote bundles to a profile (ctxloom profile create/modify), then ctxloom deps pull"
                .to_string()
        }
    }
}
