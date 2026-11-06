//! In-memory ports for the application's tests: bundles as files, config
//! layers as text, a PATH of named programs, and (S2) the write side's ports:
//! the user's files, the scaffold, an editor, and a companion probe. One
//! [`Mem`] holds the state of them all, so a test can see what a command left
//! and how many times it wrote.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use angzarr_diagnostic::{code, Arg, Diagnostic};
use ctxloom_ports::{
    Change, CompanionLoadout, CompanionLoadouts, ConfigFiles, ContentRepo, Editor, EditorCommand,
    Executables, Failure, LayerText, LoadoutProbe, Put, Scaffold, Scope, Source, TreeFiles,
    Unanswered, UserFiles,
};

/// The scope `Mem::config` is asked for.
pub use ctxloom_ports::Scope as ConfigScope;

use crate::{Flags, Library, WritePorts};

/// A failure of a fake port.
#[derive(Debug)]
pub struct FakeFailure(pub String);

impl fmt::Display for FakeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Diagnostic for FakeFailure {
    fn code(&self) -> &str {
        code!("fake")
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        vec![("ext_err_txt", self.0.as_str().into())]
    }
}

fn fail<T>(text: &str) -> Result<T, Failure> {
    Err(Box::new(FakeFailure(text.to_string())))
}

#[derive(Clone, Default)]
struct File {
    bytes: Vec<u8>,
    exec: bool,
}

type Editing = Box<dyn FnMut(&str, &str) -> Result<String, String> + Send>;
type Probing = Box<dyn FnMut(&str) -> Result<(), Unanswered> + Send>;

struct State {
    root: bool,
    home_only: bool,
    /// Bundle name to its tree.
    trees: BTreeMap<String, BTreeMap<String, File>>,
    /// Bundle directories claimed and so far empty.
    claimed: BTreeSet<String>,
    layers: Vec<LayerText>,
    project: Option<usize>,
    home: Option<usize>,
    executables: BTreeSet<String>,
    user: BTreeMap<String, File>,
    scaffold: BTreeMap<String, File>,
    /// Every mutating call, in order.
    log: Vec<String>,
    editor: Option<Editing>,
    probe: Option<Probing>,
    /// What each companion answers, by registered name; absent unless set.
    companions: BTreeMap<String, CompanionLoadout>,
    /// The companions asked for their loadout, in order.
    asked: Vec<String>,
    edits: Vec<(String, String)>,
    opened: Vec<String>,
    refuse_apply: Option<String>,
}

/// The whole world of the fakes.
pub struct Mem {
    state: Mutex<State>,
}

fn tree_of(files: &[(&str, &str)]) -> BTreeMap<String, File> {
    files
        .iter()
        .map(|(path, text)| {
            (
                (*path).to_string(),
                File {
                    bytes: text.as_bytes().to_vec(),
                    exec: false,
                },
            )
        })
        .collect()
}

impl Mem {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("the fake's lock")
    }

    /// Put a user file (as typed) in place.
    pub fn put_user(&self, path: &str, bytes: &[u8], exec: bool) {
        self.lock().user.insert(
            path.to_string(),
            File {
                bytes: bytes.to_vec(),
                exec,
            },
        );
    }

    /// A user file's text.
    pub fn user_text(&self, path: &str) -> Option<String> {
        self.lock()
            .user
            .get(path)
            .map(|f| String::from_utf8_lossy(&f.bytes).into_owned())
    }

    /// A user file's bytes and exec bit.
    pub fn user_file(&self, path: &str) -> Option<(Vec<u8>, bool)> {
        self.lock()
            .user
            .get(path)
            .map(|f| (f.bytes.clone(), f.exec))
    }

    /// The paths of the user's files.
    pub fn user_paths(&self) -> Vec<String> {
        self.lock().user.keys().cloned().collect()
    }

    /// A bundle's file's text.
    pub fn text(&self, bundle: &str, path: &str) -> Option<String> {
        self.lock()
            .trees
            .get(bundle)
            .and_then(|t| t.get(path))
            .map(|f| String::from_utf8_lossy(&f.bytes).into_owned())
    }

    /// A bundle's file's bytes and exec bit.
    pub fn file(&self, bundle: &str, path: &str) -> Option<(Vec<u8>, bool)> {
        self.lock()
            .trees
            .get(bundle)
            .and_then(|t| t.get(path))
            .map(|f| (f.bytes.clone(), f.exec))
    }

    /// The paths of a bundle's files, sorted.
    pub fn paths(&self, bundle: &str) -> Vec<String> {
        self.lock()
            .trees
            .get(bundle)
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Whether the bundle is there (files or claimed).
    pub fn has_bundle(&self, name: &str) -> bool {
        let state = self.lock();
        state.trees.contains_key(name) || state.claimed.contains(name)
    }

    /// The text of a config scope's file.
    pub fn config(&self, scope: Scope) -> Option<String> {
        let state = self.lock();
        let at = match scope {
            Scope::Project => state.project,
            Scope::Home => state.home,
        }?;
        Some(state.layers[at].text.clone())
    }

    /// Put a config scope's text in place.
    pub fn set_config(&self, scope: Scope, text: &str) {
        let mut state = self.lock();
        set_layer(&mut state, scope, text);
    }

    /// A scaffold file's text.
    pub fn scaffold_text(&self, rel: &str) -> Option<String> {
        self.lock()
            .scaffold
            .get(rel)
            .map(|f| String::from_utf8_lossy(&f.bytes).into_owned())
    }

    /// The scaffold's paths.
    pub fn scaffold_paths(&self) -> Vec<String> {
        self.lock().scaffold.keys().cloned().collect()
    }

    /// Every mutating call so far, in order.
    pub fn log(&self) -> Vec<String> {
        self.lock().log.clone()
    }

    /// How many mutating calls were made.
    pub fn writes(&self) -> usize {
        self.lock().log.len()
    }

    /// What the editor is asked to do: file name and content in, content out
    /// (or why it failed).
    pub fn set_editor(
        &self,
        edit: impl FnMut(&str, &str) -> Result<String, String> + Send + 'static,
    ) {
        self.lock().editor = Some(Box::new(edit));
    }

    /// The probe's answer for a program path.
    pub fn set_probe(&self, probe: impl FnMut(&str) -> Result<(), Unanswered> + Send + 'static) {
        self.lock().probe = Some(Box::new(probe));
    }

    /// The (file name, content) pairs the editor was given.
    pub fn edits(&self) -> Vec<(String, String)> {
        self.lock().edits.clone()
    }

    /// The paths the editor was opened on.
    pub fn opened(&self) -> Vec<String> {
        self.lock().opened.clone()
    }

    /// Give a file of a bundle the execute bit, as the disk would.
    pub fn set_exec_for_test(&self, bundle: &str, path: &str) {
        if let Some(file) = self
            .lock()
            .trees
            .get_mut(bundle)
            .and_then(|t| t.get_mut(path))
        {
            file.exec = true;
        }
    }

    /// A file of the scaffold already there.
    pub fn set_scaffold_for_test(&self, rel: &str, text: &str) {
        self.lock().scaffold.insert(
            rel.to_string(),
            File {
                bytes: text.as_bytes().to_vec(),
                exec: false,
            },
        );
    }

    /// Make the next `apply` fail with this text.
    pub fn refuse_apply(&self, text: &str) {
        self.lock().refuse_apply = Some(text.to_string());
    }
}

fn set_layer(state: &mut State, scope: Scope, text: &str) {
    let existing = match scope {
        Scope::Project => state.project,
        Scope::Home => state.home,
    };
    if let Some(at) = existing {
        state.layers[at].text = text.to_string();
        return;
    }
    match scope {
        Scope::Project => {
            state.layers.push(LayerText {
                path: "/p/.ctxloom/config.yaml".to_string(),
                text: text.to_string(),
            });
            state.project = Some(state.layers.len() - 1);
        }
        Scope::Home => {
            state.layers.insert(
                0,
                LayerText {
                    path: "/h/.ctxloom/config.yaml".to_string(),
                    text: text.to_string(),
                },
            );
            state.home = Some(0);
            if let Some(p) = state.project.as_mut() {
                *p += 1;
            }
        }
    }
}

/// A path as a bundle's file name below the tree: slash separated, no escape.
fn clean(path: &str) -> Result<(), Failure> {
    if !ctxloom_ports::is_plain_relative(path) {
        return fail(&format!("bad path {path}"));
    }
    Ok(())
}

impl ContentRepo for Mem {
    fn has_root(&self) -> Result<bool, Failure> {
        Ok(self.lock().root)
    }

    fn bundle_names(&self) -> Result<Vec<String>, Failure> {
        Ok(self.lock().trees.keys().cloned().collect())
    }

    fn read_tree(&self, name: &str) -> Result<TreeFiles, Failure> {
        Ok(self
            .lock()
            .trees
            .get(name)
            .map(|t| {
                t.iter()
                    .map(|(p, f)| (p.clone(), f.bytes.clone()))
                    .collect()
            })
            .unwrap_or_default())
    }

    fn manifest_path(&self, name: &str) -> String {
        format!("/p/.ctxloom/content/bundles/v2/{name}/bundle.yaml")
    }

    fn executable_files(&self, name: &str) -> Result<BTreeSet<String>, Failure> {
        let state = self.lock();
        let mut out: BTreeSet<String> = state.executables.clone();
        if let Some(tree) = state.trees.get(name) {
            out.extend(tree.iter().filter(|(_, f)| f.exec).map(|(p, _)| p.clone()));
        }
        Ok(out)
    }

    fn apply(&self, name: &str, changes: &[Change]) -> Result<(), Failure> {
        let mut state = self.lock();
        if let Some(why) = state.refuse_apply.take() {
            return fail(&why);
        }
        for change in changes {
            match change {
                Change::Write(Put { path, .. })
                | Change::Remove { path }
                | Change::RemoveDir { path } => clean(path)?,
            }
        }
        state.log.push(format!("apply {name}"));
        let tree = state.trees.entry(name.to_string()).or_default();
        for change in changes {
            match change {
                Change::Write(put) => {
                    tree.insert(
                        put.path.clone(),
                        File {
                            bytes: put.bytes.clone(),
                            exec: put.executable,
                        },
                    );
                }
                Change::Remove { path } => {
                    tree.remove(path);
                }
                Change::RemoveDir { path } => {
                    let prefix = format!("{path}/");
                    tree.retain(|p, _| !p.starts_with(&prefix));
                }
            }
        }
        Ok(())
    }

    fn claim(&self, name: &str) -> Result<bool, Failure> {
        let mut state = self.lock();
        if state.trees.contains_key(name) || state.claimed.contains(name) {
            return Ok(false);
        }
        state.log.push(format!("claim {name}"));
        state.claimed.insert(name.to_string());
        state.trees.entry(name.to_string()).or_default();
        Ok(true)
    }

    fn remove_bundle(&self, name: &str) -> Result<(), Failure> {
        let mut state = self.lock();
        state.log.push(format!("remove_bundle {name}"));
        state.trees.remove(name);
        state.claimed.remove(name);
        Ok(())
    }

    fn bundle_dir(&self, name: &str) -> String {
        format!("/p/.ctxloom/content/bundles/v2/{name}")
    }
}

impl ConfigFiles for Mem {
    fn source(&self) -> Source {
        if self.lock().home_only {
            Source::Home
        } else {
            Source::Project
        }
    }

    fn app_dir(&self) -> String {
        "/p/.ctxloom".to_string()
    }

    fn layers(&self) -> Result<Vec<LayerText>, Failure> {
        Ok(self.lock().layers.clone())
    }

    fn layer(&self, scope: Scope) -> Result<Option<LayerText>, Failure> {
        let state = self.lock();
        let at = match scope {
            Scope::Project => state.project,
            Scope::Home => state.home,
        };
        Ok(at.map(|i| state.layers[i].clone()))
    }

    fn layer_path(&self, scope: Scope) -> String {
        match scope {
            Scope::Project => "/p/.ctxloom/config.yaml".to_string(),
            Scope::Home => "/h/.ctxloom/config.yaml".to_string(),
        }
    }

    fn write_layer(&self, scope: Scope, text: &str) -> Result<(), Failure> {
        let mut state = self.lock();
        state.log.push(format!("write_layer {scope:?}"));
        set_layer(&mut state, scope, text);
        Ok(())
    }
}

impl UserFiles for Mem {
    fn exists(&self, path: &str) -> Result<bool, Failure> {
        let state = self.lock();
        let dir = format!("{path}/");
        Ok(state.user.contains_key(path) || state.user.keys().any(|p| p.starts_with(&dir)))
    }

    fn is_dir(&self, path: &str) -> Result<bool, Failure> {
        let dir = format!("{path}/");
        Ok(self.lock().user.keys().any(|p| p.starts_with(&dir)))
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, Failure> {
        match self.lock().user.get(path) {
            Some(f) => Ok(f.bytes.clone()),
            None => fail(&format!("no file {path}")),
        }
    }

    fn read_tree(&self, dir: &str) -> Result<Vec<Put>, Failure> {
        let state = self.lock();
        let prefix = format!("{dir}/");
        let found: Vec<Put> = state
            .user
            .iter()
            .filter_map(|(p, f)| {
                p.strip_prefix(&prefix).map(|rest| Put {
                    path: rest.to_string(),
                    bytes: f.bytes.clone(),
                    executable: f.exec,
                })
            })
            .collect();
        if found.is_empty() {
            return fail(&format!("no directory {dir}"));
        }
        Ok(found)
    }

    fn write_file(&self, path: &str, bytes: &[u8], overwrite: bool) -> Result<bool, Failure> {
        let mut state = self.lock();
        if !overwrite && state.user.contains_key(path) {
            return Ok(false);
        }
        state.log.push(format!("write_file {path}"));
        state.user.insert(
            path.to_string(),
            File {
                bytes: bytes.to_vec(),
                exec: false,
            },
        );
        Ok(true)
    }

    fn write_tree(&self, dir: &str, files: &[Put]) -> Result<(), Failure> {
        let mut state = self.lock();
        state.log.push(format!("write_tree {dir}"));
        for put in files {
            state.user.insert(
                format!("{dir}/{}", put.path),
                File {
                    bytes: put.bytes.clone(),
                    exec: put.executable,
                },
            );
        }
        Ok(())
    }
}

impl Scaffold for Mem {
    fn app_dir(&self) -> String {
        "/work/.ctxloom".to_string()
    }

    fn exists(&self, rel: &str) -> Result<bool, Failure> {
        let state = self.lock();
        let dir = format!("{rel}/");
        Ok(state.scaffold.contains_key(rel) || state.scaffold.keys().any(|p| p.starts_with(&dir)))
    }

    fn write(&self, files: &[Put]) -> Result<(), Failure> {
        let mut state = self.lock();
        state.log.push("scaffold".to_string());
        for put in files {
            state.scaffold.insert(
                put.path.clone(),
                File {
                    bytes: put.bytes.clone(),
                    exec: put.executable,
                },
            );
        }
        Ok(())
    }
}

impl Editor for Mem {
    fn edit(
        &self,
        _command: &EditorCommand,
        file_name: &str,
        content: &str,
    ) -> Result<String, Failure> {
        let mut state = self.lock();
        state
            .edits
            .push((file_name.to_string(), content.to_string()));
        match state.editor.as_mut() {
            Some(editor) => editor(file_name, content).or_else(|why| fail(&why)),
            None => Ok(content.to_string()),
        }
    }

    fn open(&self, _command: &EditorCommand, path: &str) -> Result<(), Failure> {
        self.lock().opened.push(path.to_string());
        Ok(())
    }
}

impl LoadoutProbe for Mem {
    fn answers(&self, path: &str) -> Result<(), Unanswered> {
        match self.lock().probe.as_mut() {
            Some(probe) => probe(path),
            None => Ok(()),
        }
    }
}

impl Mem {
    /// The companions asked for their loadout, in the order they were asked.
    pub fn asked_companions(&self) -> Vec<String> {
        self.lock().asked.clone()
    }
}

impl CompanionLoadouts for Mem {
    fn loadout(&self, name: &str) -> CompanionLoadout {
        let mut state = self.lock();
        state.asked.push(name.to_string());
        state
            .companions
            .get(name)
            .cloned()
            .unwrap_or(CompanionLoadout::NoLoadout)
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
    /// The loadout each registered companion answers with; one not listed has no
    /// loadout (and is not said).
    pub companions: &'a [(&'a str, CompanionLoadout)],
}

impl Setup<'_> {
    /// The library, and the world it works in.
    pub fn fixture(&self) -> (Library, Arc<Mem>) {
        let mut state = State {
            root: !self.no_root,
            home_only: self.home_only,
            trees: BTreeMap::new(),
            claimed: BTreeSet::new(),
            layers: Vec::new(),
            project: None,
            home: None,
            executables: self.executable.iter().map(ToString::to_string).collect(),
            user: BTreeMap::new(),
            scaffold: BTreeMap::new(),
            log: Vec::new(),
            editor: None,
            probe: None,
            companions: self
                .companions
                .iter()
                .map(|(n, l)| ((*n).to_string(), l.clone()))
                .collect(),
            asked: Vec::new(),
            edits: Vec::new(),
            opened: Vec::new(),
            refuse_apply: None,
        };
        for (name, files) in self.bundles {
            state.trees.insert((*name).to_string(), tree_of(files));
        }
        for (i, text) in self.layers.iter().enumerate() {
            state.layers.push(LayerText {
                path: format!("/p/.ctxloom/layer-{i}.yaml"),
                text: (*text).to_string(),
            });
        }
        let count = state.layers.len();
        if count > 0 {
            state.project = Some(count - 1);
        }
        if count > 1 {
            state.home = Some(0);
        }
        let mem = Arc::new(Mem {
            state: Mutex::new(state),
        });
        let library = Library::new(
            mem.clone(),
            mem.clone(),
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
        .with_write_ports(WritePorts {
            user: mem.clone(),
            scaffold: mem.clone(),
            editor: mem.clone(),
            probe: mem.clone(),
        })
        .with_companions(mem.clone());
        (library, mem)
    }

    pub fn library(&self) -> Library {
        self.fixture().0
    }
}

/// The flags of an ordinary invocation.
pub fn plain() -> Flags {
    Flags::default()
}

/// A bundle envelope with a version, for a tree.
pub const ENVELOPE: (&str, &str) = ("bundle.yaml", "schema_version: 4\nversion: 1.0.0\n");

pub const CONFIG: &str = "schema_version: 8\n";
