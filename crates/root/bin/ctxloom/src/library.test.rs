//! The library read side's acceptance harness (S1): a project and a home on
//! disk, and the real `ctxloom` binary run in them.
//!
//! A project is built the way Go's acceptance steps build one: the files
//! `ctxloom bundle create`, `fragment create` and `profile create` write
//! (`steps_fixture.go`: "an initialized ctxloom project", `a bundle "demo"
//! exists`, `a fragment "testing" in bundle "demo" exists`, `a profile "dev"
//! with bundle "demo"`), put in place directly, because the write side is
//! S2's. Or it is a copy of a fixture under `fixtures/`: a project tree and a
//! home tree that Go ctxloom's own output was captured against.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::{binary, Ran};

/// Go's `minimalConfig` (`steps_fixture.go`): the smallest project config.
pub(crate) const MINIMAL_CONFIG: &str = "schema_version: 8\n";

/// The editors Go's acceptance steps pin in the home config
/// (`steps_fixture.go`): `sh -c <script> editor <tmpfile>`, so `$1` is the
/// temporary file the content was written to.
#[derive(Clone, Copy)]
pub(crate) enum Editor {
    /// Appends `EDITED-BY-TEST` to the file ("a marker editor").
    Marker,
    /// Truncates the file ("an editor that empties the file").
    Empties,
    /// Rewrites the `description:` line ("a description-rewriting editor").
    Description,
    /// Rewrites the `command:` line ("a command-rewriting editor").
    Command,
}

impl Editor {
    pub(crate) fn home_config(self) -> &'static str {
        match self {
            Editor::Marker => "schema_version: 8\neditor:\n  command: sh\n  args:\n    - \"-c\"\n    - 'printf \"\\nEDITED-BY-TEST\\n\" >> \"$1\"'\n    - editor\n",
            Editor::Empties => "schema_version: 8\neditor:\n  command: sh\n  args:\n    - \"-c\"\n    - 'printf \"\" > \"$1\"'\n    - editor\n",
            Editor::Description => "schema_version: 8\neditor:\n  command: sh\n  args:\n    - \"-c\"\n    - 'sed \"s/^description:.*/description: EDITED-BY-TEST/\" \"$1\" > \"$1.edited\" && mv \"$1.edited\" \"$1\"'\n    - editor\n",
            Editor::Command => "schema_version: 8\neditor:\n  command: sh\n  args:\n    - \"-c\"\n    - 'sed \"s/^\\( *\\)command:.*/\\1command: EDITED-BY-TEST/\" \"$1\" > \"$1.edited\" && mv \"$1.edited\" \"$1\"'\n    - editor\n",
        }
    }
}

/// Where the fixtures are.
pub(crate) fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// A project directory and a home directory, both temporary.
pub(crate) struct Project {
    _root: tempfile::TempDir,
    project: PathBuf,
    home: PathBuf,
}

impl Project {
    /// Go's "an initialized ctxloom project": a config at the current
    /// schema, in the project and (with a no-op editor) in the home.
    pub(crate) fn initialized() -> Project {
        let project = Project::bare();
        project.write(".ctxloom/config.yaml", MINIMAL_CONFIG);
        project.write_home(
            ".ctxloom/config.yaml",
            "schema_version: 8\neditor:\n  command: \"true\"\n",
        );
        project
    }

    /// Go's "an empty project directory": nothing in it.
    pub(crate) fn bare() -> Project {
        let root = tempfile::tempdir().expect("a temporary directory");
        let project = root.path().join("project");
        let home = root.path().join("home");
        std::fs::create_dir_all(&project).expect("the project directory");
        std::fs::create_dir_all(&home).expect("the home directory");
        Project {
            _root: root,
            project,
            home,
        }
    }

    /// A copy of `fixtures/<name>/project` and `fixtures/<name>/home`.
    pub(crate) fn from_fixture(name: &str) -> Project {
        let project = Project::bare();
        let source = fixtures().join(name);
        copy_tree(&source.join("project"), &project.project);
        if source.join("home").is_dir() {
            copy_tree(&source.join("home"), &project.home);
        }
        project
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.project
    }

    pub(crate) fn home(&self) -> &Path {
        &self.home
    }

    /// Put `content` at `rel` in the project.
    pub(crate) fn write(&self, rel: &str, content: &str) {
        put(&self.project.join(rel), content);
    }

    /// Put `bytes` at `rel` in the project.
    pub(crate) fn write_bytes(&self, rel: &str, bytes: &[u8]) {
        let path = self.project.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("a directory");
        }
        std::fs::write(path, bytes).expect("a file");
    }

    /// Put `content` at `rel` in the home.
    pub(crate) fn write_home(&self, rel: &str, content: &str) {
        put(&self.home.join(rel), content);
    }

    /// Go's `ctxloom bundle create <name> -d "acceptance fixture bundle"`:
    /// the manifest, an example fragment and an example command.
    pub(crate) fn bundle(&self, name: &str) {
        let root = format!(".ctxloom/content/bundles/v2/{name}");
        self.write(
            &format!("{root}/bundle.yaml"),
            "schema_version: 4\nversion: 1.0.0\ndescription: acceptance fixture bundle\n",
        );
        self.write(
            &format!("{root}/fragments/example.md"),
            "---\ntags:\n  - example\nno_distill: true\n---\n# Example Fragment\n\nAdd your content here.",
        );
        self.write(
            &format!("{root}/prompts/example.md"),
            "---\ndescription: Example prompt\ntags:\n  - example\nno_distill: true\n---\nExample prompt content. Describe what this prompt does.",
        );
    }

    /// Go's `a fragment "<name>" in bundle "<bundle>" exists`: the body is
    /// the marker `FRAGMENT-BODY-<name>` (`fixtureFragmentBody`).
    pub(crate) fn fragment(&self, bundle: &str, name: &str) {
        self.write(
            &format!(".ctxloom/content/bundles/v2/{bundle}/fragments/{name}.md"),
            &format!("FRAGMENT-BODY-{name}"),
        );
    }

    /// Go's `a command "<name>" in bundle "<bundle>" exists`: the body is
    /// the marker `COMMAND-BODY-<name>`.
    pub(crate) fn command(&self, bundle: &str, name: &str) {
        self.write(
            &format!(".ctxloom/content/bundles/v2/{bundle}/prompts/{name}.md"),
            &format!("COMMAND-BODY-{name}"),
        );
    }

    /// Go's `a profile "<name>" with bundle "<bundle>"`: the file
    /// `profile create <name> --include <bundle> -d "acceptance fixture
    /// profile"` writes, in the project bundle.
    pub(crate) fn profile(&self, name: &str, bundle: &str) {
        self.write(
            ".ctxloom/content/bundles/v2/project/bundle.yaml",
            "schema_version: 4\nversion: 1.0.0\n",
        );
        self.write(
            &format!(".ctxloom/content/bundles/v2/project/profiles/{name}.yaml"),
            &format!("description: acceptance fixture profile\nbundles:\n  - {bundle}\n"),
        );
    }

    /// The text of the file at `rel` in the project; none where it is not there.
    pub(crate) fn read(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.project.join(rel)).ok()
    }

    /// The bytes of the file at `rel` in the project; none where it is not there.
    pub(crate) fn read_bytes(&self, rel: &str) -> Option<Vec<u8>> {
        std::fs::read(self.project.join(rel)).ok()
    }

    /// The text of the file at `rel` in the home.
    pub(crate) fn read_home(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.home.join(rel)).ok()
    }

    /// Whether anything is at `rel` in the project.
    pub(crate) fn exists(&self, rel: &str) -> bool {
        self.project.join(rel).exists()
    }

    /// Go's `the file "<rel>" contains "<text>"`.
    pub(crate) fn file_contains(&self, rel: &str, text: &str) -> &Project {
        let held = self.read(rel).unwrap_or_else(|| panic!("no file {rel}"));
        assert!(
            held.contains(text),
            "{rel} should contain {text:?}: {held:?}"
        );
        self
    }

    /// Go's `the file "<rel>" does not contain "<text>"`.
    pub(crate) fn file_lacks(&self, rel: &str, text: &str) -> &Project {
        let held = self.read(rel).unwrap_or_else(|| panic!("no file {rel}"));
        assert!(
            !held.contains(text),
            "{rel} should not contain {text:?}: {held:?}"
        );
        self
    }

    /// Go's `the file "<rel>" does not exist`.
    pub(crate) fn file_is_absent(&self, rel: &str) -> &Project {
        assert!(!self.exists(rel), "{rel} should not exist");
        self
    }

    /// Every file below `rel` in the project, by its path below `rel` (slash
    /// separated) and its bytes.
    pub(crate) fn tree(&self, rel: &str) -> std::collections::BTreeMap<String, Vec<u8>> {
        fn walk(top: &Path, dir: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    walk(top, &path, out);
                } else {
                    let below = path.strip_prefix(top).expect("below the top");
                    out.insert(
                        below.to_string_lossy().replace('\\', "/"),
                        std::fs::read(&path).expect("a file"),
                    );
                }
            }
        }
        let mut out = std::collections::BTreeMap::new();
        walk(&self.project.join(rel), &self.project.join(rel), &mut out);
        out
    }

    /// Give the file at `rel` the execute bits (unix).
    pub(crate) fn make_executable(&self, rel: &str) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                self.project.join(rel),
                std::fs::Permissions::from_mode(0o755),
            )
            .expect("the file made executable");
        }
        #[cfg(not(unix))]
        let _ = rel;
    }

    /// Whether the file at `rel` has an execute bit (unix; false elsewhere).
    pub(crate) fn is_executable(&self, rel: &str) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::metadata(self.project.join(rel))
                .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            let _ = rel;
            false
        }
    }

    /// Go's "a ctxloom project with a <kind> editor": the project's config
    /// minimal, the home's pinning the editor (`steps_fixture.go`).
    pub(crate) fn with_editor(editor: Editor) -> Project {
        let project = Project::bare();
        project.write(".ctxloom/config.yaml", MINIMAL_CONFIG);
        project.write_home(".ctxloom/config.yaml", editor.home_config());
        project
    }

    /// Run `ctxloom args` in the project, with the project root named by
    /// `CTXLOOM_ROOT`.
    pub(crate) fn run(&self, args: &[&str]) -> Ran {
        self.run_with(&[], args)
    }

    /// As [`run`](Project::run), with these environment variables too.
    pub(crate) fn run_with(&self, env: &[(&str, &str)], args: &[&str]) -> Ran {
        let mut all = vec![("CTXLOOM_ROOT", self.project.to_str().expect("utf-8"))];
        all.extend_from_slice(env);
        self.spawn(&self.project, &all, args)
    }

    /// Run from `dir` with no `CTXLOOM_ROOT`: the project is found by
    /// walking up from the working directory.
    pub(crate) fn run_in(&self, dir: &Path, args: &[&str]) -> Ran {
        self.spawn(dir, &[], args)
    }

    fn spawn(&self, cwd: &Path, env: &[(&str, &str)], args: &[&str]) -> Ran {
        let mut command = Command::new(binary());
        command
            .args(args)
            .env_clear()
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .current_dir(cwd);
        for name in ["PATH", "SystemRoot"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        for (name, value) in env {
            command.env(name, value);
        }
        Ran::of(command.output().expect("ctxloom runs"))
    }

    /// What a run printed with the project's and the home's paths written
    /// as `{PROJECT}` and `{HOME}`: the form the goldens are in.
    pub(crate) fn normalized(&self, text: &str) -> String {
        text.replace(self.project.to_str().expect("utf-8"), "{PROJECT}")
            .replace(self.home.to_str().expect("utf-8"), "{HOME}")
    }
}

fn put(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("a directory");
    }
    std::fs::write(path, content).expect("a file");
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a directory");
    for entry in std::fs::read_dir(from).expect("a fixture directory") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("a file copy");
        }
    }
}

impl Ran {
    /// Both streams, as Go's "the output contains" reads them.
    ///
    /// Off a terminal an error is a json envelope on the error stream; Go's
    /// text is its `message` (and `fix`), decoded, so the quotes read as the
    /// person reads them.
    pub(crate) fn output(&self) -> String {
        let said = serde_json::from_str::<Value>(&self.stderr)
            .ok()
            .and_then(|doc| {
                let message = doc.get("message")?.as_str()?.to_string();
                let fix = doc.get("fix").and_then(Value::as_str).unwrap_or_default();
                Some(format!("{message}\n{fix}\n"))
            })
            .unwrap_or_else(|| self.stderr.clone());
        format!("{}{}{}", self.stdout, self.stderr, said)
    }

    /// Go's "the command succeeds".
    pub(crate) fn succeeds(&self) -> &Ran {
        assert_eq!(self.code, 0, "expected success: {}", self.said());
        self
    }

    /// Go's "the command fails": a non-zero exit (any of the ladder's).
    pub(crate) fn fails(&self) -> &Ran {
        assert_ne!(self.code, 0, "expected failure: {}", self.said());
        self
    }

    /// Go's `the output contains "<text>"`.
    pub(crate) fn contains(&self, text: &str) -> &Ran {
        assert!(
            self.output().contains(text),
            "expected the output to contain {text:?}: {}",
            self.said()
        );
        self
    }

    /// Go's `the output does not contain "<text>"`.
    pub(crate) fn lacks(&self, text: &str) -> &Ran {
        assert!(
            !self.output().contains(text),
            "expected the output not to contain {text:?}: {}",
            self.said()
        );
        self
    }
}

/// The value at a path in a JSON document, in the notation of Go's
/// `the output reports "<path>" as ...` (`steps_cli.go`): `$` is the root, a
/// segment is a key, an index, or a selector `[key=value]` that picks the
/// element of an array whose `key` is `value`.
pub(crate) fn at<'a>(doc: &'a Value, path: &str) -> Option<&'a Value> {
    let mut here = doc;
    if path == "$" {
        return Some(here);
    }
    let path = path.strip_prefix("$.").unwrap_or(path);
    for segment in split_path(path) {
        here = match segment.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            Some(selector) => {
                let (key, value) = selector.split_once('=')?;
                here.as_array()?
                    .iter()
                    .find(|element| element[key].as_str() == Some(value))?
            }
            None => match segment.parse::<usize>() {
                Ok(index) if here.is_array() => here.get(index)?,
                _ => here.get(segment)?,
            },
        };
    }
    Some(here)
}

/// A path's segments: dotted, with `[k=v]` kept whole (the value of a
/// selector may hold a dot).
fn split_path(path: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let (mut start, mut depth) = (0, 0usize);
    for (i, c) in path.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            '.' if depth == 0 => {
                if i > start {
                    segments.push(&path[start..i]);
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < path.len() {
        segments.push(&path[start..]);
    }
    // `servers[name=x].command` is `servers` then `[name=x]` then `command`.
    segments
        .into_iter()
        .flat_map(|segment| match segment.find('[') {
            Some(at) if at > 0 => vec![&segment[..at], &segment[at..]],
            _ => vec![segment],
        })
        .collect()
}

/// Go's `the output reports "<path>" as "<value>"`: the scalar at `path`,
/// as text.
pub(crate) fn reports_as(doc: &Value, path: &str) -> String {
    match at(doc, path) {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => panic!("no {path} in {doc}"),
    }
}

/// Go's `the output reports "<path>" having "<n>" entries`.
pub(crate) fn entries(doc: &Value, path: &str) -> usize {
    match at(doc, path) {
        Some(Value::Array(a)) => a.len(),
        Some(Value::Object(o)) => o.len(),
        other => panic!("no list at {path}: {other:?} in {doc}"),
    }
}

/// Go's `the output reports "<path>" containing "<value>"`.
pub(crate) fn contains_value(doc: &Value, path: &str, value: &str) -> bool {
    at(doc, path)
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|e| e.as_str() == Some(value)))
}
