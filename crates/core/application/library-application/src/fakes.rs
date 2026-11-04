//! In-memory ports for the application's tests: bundles as files, config
//! layers as text, a PATH of named programs.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ctxloom_ports::{ConfigFiles, ContentRepo, Executables, Failure, LayerText, Source, TreeFiles};

use crate::{Flags, Library};

/// Bundles by name, each a list of `(path, text)`.
struct Repo {
    root: bool,
    trees: BTreeMap<String, TreeFiles>,
    executable: BTreeSet<String>,
}

impl ContentRepo for Repo {
    fn has_root(&self) -> Result<bool, Failure> {
        Ok(self.root)
    }

    fn bundle_names(&self) -> Result<Vec<String>, Failure> {
        Ok(self.trees.keys().cloned().collect())
    }

    fn read_tree(&self, name: &str) -> Result<TreeFiles, Failure> {
        Ok(self.trees.get(name).cloned().unwrap_or_default())
    }

    fn manifest_path(&self, name: &str) -> String {
        format!("/p/.ctxloom/content/bundles/v2/{name}/bundle.yaml")
    }

    fn executable_files(&self, _name: &str) -> Result<BTreeSet<String>, Failure> {
        Ok(self.executable.clone())
    }
}

struct Layers {
    layers: Vec<LayerText>,
    source: Source,
}

impl ConfigFiles for Layers {
    fn source(&self) -> Source {
        self.source
    }

    fn app_dir(&self) -> String {
        "/p/.ctxloom".to_string()
    }

    fn layers(&self) -> Result<Vec<LayerText>, Failure> {
        Ok(self.layers.clone())
    }
}

struct Path(BTreeMap<String, String>);

impl Executables for Path {
    fn find(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

/// What a test sets up.
#[derive(Default)]
pub struct Setup<'a> {
    pub bundles: &'a [(&'a str, &'a [(&'a str, &'a str)])],
    pub layers: &'a [&'a str],
    pub home_only: bool,
    /// The project has no bundles directory.
    pub no_root: bool,
    pub executable: &'a [&'a str],
    pub path: &'a [(&'a str, &'a str)],
    pub environ: &'a [(&'a str, &'a str)],
}

impl Setup<'_> {
    pub fn library(&self) -> Library {
        let trees = self
            .bundles
            .iter()
            .map(|(name, files)| {
                (
                    (*name).to_string(),
                    files
                        .iter()
                        .map(|(path, text)| ((*path).to_string(), text.as_bytes().to_vec()))
                        .collect(),
                )
            })
            .collect();
        let layers = self
            .layers
            .iter()
            .enumerate()
            .map(|(i, text)| LayerText {
                path: format!("/p/.ctxloom/layer-{i}.yaml"),
                text: (*text).to_string(),
            })
            .collect();
        Library::new(
            Arc::new(Repo {
                root: !self.no_root,
                trees,
                executable: self.executable.iter().map(ToString::to_string).collect(),
            }),
            Arc::new(Layers {
                layers,
                source: if self.home_only {
                    Source::Home
                } else {
                    Source::Project
                },
            }),
            Arc::new(Path(
                self.path
                    .iter()
                    .map(|(n, p)| ((*n).to_string(), (*p).to_string()))
                    .collect(),
            )),
            self.environ
                .iter()
                .map(|(n, v)| ((*n).to_string(), (*v).to_string()))
                .collect(),
        )
    }
}

/// The flags of an ordinary invocation.
pub fn plain() -> Flags {
    Flags::default()
}

/// A bundle envelope with a version, for a tree.
pub const ENVELOPE: (&str, &str) = ("bundle.yaml", "schema_version: 4\nversion: 1.0.0\n");

pub const CONFIG: &str = "schema_version: 8\n";
