//! Acceptance for S3 (remotes and dependencies): the real `ctxloom` binary,
//! run in a project and a home on disk, against local git remotes made in
//! temporary directories (ADR 0015's fixture approach; Go's `gitserve`
//! fixtures: `SeedRemote`, `AdvanceRemote`, `UnpublishFromRemote`,
//! `BreakRemote`, `RemoteFile`).
//!
//! The steps are Go's (`tests/acceptance/steps_fixture.go`), as methods of
//! [`World`]. Where a step runs a write command that is S2's (`profile
//! create`, `profile modify`), the file it writes is put in place directly,
//! as the S1 harness does (`Project::profile`).
//!
//! plan supersedes Go: `ctxloom materialize` is S9 (plan 8.2), so the
//! scenarios that read what an assembly carries after a pull read it through
//! `context assemble` (the same claim: what reaches the agent is the pinned
//! content). The four scenarios of "Pull writes the project's engine files
//! only with --yes" read the engine files the project refresh writes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::library::Project;
use super::Ran;

#[path = "bundle_remote.test.rs"]
mod bundle_remote;

#[path = "deps_feature.test.rs"]
mod deps_feature;

#[path = "remote_feature.test.rs"]
mod remote_feature;

/// Where a bundle is in a repository (Go's `treeBundlePath`).
pub(crate) fn tree_bundle(name: &str) -> String {
    format!(".ctxloom/content/bundles/v2/{name}")
}

/// A git command in `dir`, in an environment of its own: nothing of the
/// test's, an identity as a user's own configuration gives.
pub(crate) fn git_in(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", dir)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.test")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.test")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn write_files(dir: &Path, files: &[(String, String)]) {
    for (rel, body) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(&path, body).expect("a file");
    }
}

/// Go's `fixtureDemoTreeFiles`: the "demo" bundle every "a git remote ...
/// serving a ctxloom bundle" step seeds or advances.
fn demo_tree(
    version: &str,
    description: &str,
    fragment: &str,
    content: &str,
    with_command: bool,
) -> Vec<(String, String)> {
    let root = tree_bundle("demo");
    let mut files = vec![
        (
            format!("{root}/bundle.yaml"),
            format!("version: \"{version}\"\ndescription: \"{description}\"\n"),
        ),
        (
            format!("{root}/fragments/{fragment}.md"),
            format!("---\ntags: [demo]\n---\n\n{content}\n"),
        ),
    ];
    if with_command {
        files.push((
            format!("{root}/prompts/demo-skill.md"),
            "---\ndescription: demo prompt\n---\n\nDemo prompt content.\n".to_string(),
        ));
    }
    files
}

/// A project, and the git remotes its scenario seeds.
pub(crate) struct World {
    pub(crate) project: Project,
    scratch: tempfile::TempDir,
    /// Remote name to its bare repository (moved by `becomes unreachable`).
    bare: BTreeMap<String, PathBuf>,
    /// Remote name to the bare repository it was seeded at: the address its
    /// references were written with.
    seeded: BTreeMap<String, PathBuf>,
    count: usize,
}

impl World {
    /// Go's "an initialized ctxloom project".
    pub(crate) fn initialized() -> World {
        World {
            project: Project::initialized(),
            scratch: tempfile::tempdir().expect("a scratch directory"),
            bare: BTreeMap::new(),
            seeded: BTreeMap::new(),
            count: 0,
        }
    }

    pub(crate) fn run(&self, args: &[&str]) -> Ran {
        self.project.run(args)
    }

    /// `ran` with these environment variables too.
    pub(crate) fn run_with(&self, env: &[(&str, &str)], args: &[&str]) -> Ran {
        self.project.run_with(env, args)
    }

    /// Go's `SeedRemote`: a bare repository whose `main` holds `files`.
    fn seed_bare(&mut self, files: &[(String, String)]) -> PathBuf {
        self.count += 1;
        let root = self.scratch.path().join(format!("remote-{}", self.count));
        let (bare, work) = (root.join("remote.git"), root.join("work"));
        std::fs::create_dir_all(&root).expect("a directory");
        git_in(&root, &["init", "--bare", "-b", "main", "remote.git"]);
        git_in(&root, &["init", "-b", "main", "work"]);
        write_files(&work, files);
        git_in(&work, &["add", "-A"]);
        git_in(&work, &["commit", "-m", "seed"]);
        git_in(
            &work,
            &["remote", "add", "origin", bare.to_str().expect("utf-8")],
        );
        git_in(&work, &["push", "origin", "main"]);
        git_in(&bare, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        bare
    }

    fn register(&mut self, name: &str, bare: PathBuf) {
        let url = format!("file://{}", bare.display());
        self.bare.insert(name.to_string(), bare.clone());
        self.seeded.insert(name.to_string(), bare);
        self.run(&["remote", "create", name, &url, "--forge", "git"])
            .succeeds();
    }

    /// Go's `a git remote "<name>" serving a ctxloom bundle`.
    pub(crate) fn git_remote(&mut self, name: &str) {
        let files = demo_tree(
            "1.0.0",
            "Demo bundle",
            "demo-frag",
            "Demo fragment content.",
            true,
        );
        let bare = self.seed_bare(&files);
        self.register(name, bare);
    }

    /// Go's `a git remote "<name>" serving a bundle "<bundle>" whose profile
    /// "<profile>" draws on bundle "<target>" of the remote "<other>"`.
    pub(crate) fn git_remote_reaching(
        &mut self,
        name: &str,
        bundle: &str,
        profile: &str,
        target: &str,
        other: &str,
    ) {
        let other_bare = self.seeded[other].display().to_string();
        let root = tree_bundle(bundle);
        let files = vec![
            (
                format!("{root}/bundle.yaml"),
                "version: \"1.0.0\"\ndescription: \"reaches elsewhere\"\n".to_string(),
            ),
            (
                format!("{root}/profiles/{profile}.yaml"),
                format!("bundles:\n  - file://{other_bare}@bundles/{target}\n"),
            ),
        ];
        let bare = self.seed_bare(&files);
        self.register(name, bare);
    }

    /// Go's `the profile "<profile>" draws on a bundle straight from an
    /// unregistered git repository`.
    pub(crate) fn profile_from_unregistered(&mut self, profile: &str) {
        let files = demo_tree(
            "1.0.0",
            "Demo bundle",
            "demo-frag",
            "Demo fragment content.",
            true,
        );
        let bare = self.seed_bare(&files);
        self.project.profile(
            profile,
            &format!("ctxloom+file://{}//bundles/demo", bare.display()),
        );
    }

    /// The canonical reference to `bundle` of the remote `name`, as `profile
    /// create --include <name>/<bundle>` stores it.
    pub(crate) fn reference(&self, name: &str, bundle: &str) -> String {
        format!(
            "ctxloom+file://{}//bundles/{bundle}",
            self.seeded[name].display()
        )
    }

    /// Go's `ctxloom profile create dev --include <name>/<bundle> ...`.
    pub(crate) fn profile_including(&self, profile: &str, bundles: &[(&str, &str)]) {
        let refs: Vec<String> = bundles
            .iter()
            .map(|(remote, bundle)| self.reference(remote, bundle))
            .collect();
        self.write_profile(profile, &refs);
    }

    /// A profile of the project bundle naming exactly these bundles.
    pub(crate) fn write_profile(&self, profile: &str, bundles: &[String]) {
        let list: String = bundles.iter().map(|b| format!("  - {b}\n")).collect();
        self.project.write(
            ".ctxloom/content/bundles/v2/project/bundle.yaml",
            "schema_version: 4\nversion: 1.0.0\n",
        );
        self.project.write(
            &format!(".ctxloom/content/bundles/v2/project/profiles/{profile}.yaml"),
            &format!("bundles:\n{list}"),
        );
    }

    fn work_clone(&mut self, name: &str) -> PathBuf {
        self.count += 1;
        let work = self.scratch.path().join(format!("work-{}", self.count));
        let bare = self.bare[name].clone();
        git_in(
            self.scratch.path(),
            &[
                "clone",
                bare.to_str().expect("utf-8"),
                work.to_str().expect("utf-8"),
            ],
        );
        work
    }

    /// Go's `AdvanceRemote`: a commit that rewrites `files`.
    pub(crate) fn advance_with(&mut self, name: &str, files: &[(String, String)]) {
        let work = self.work_clone(name);
        write_files(&work, files);
        git_in(&work, &["add", "-A"]);
        git_in(&work, &["commit", "-m", "advance"]);
        git_in(&work, &["push", "origin", "main"]);
    }

    /// Go's `the remote "<name>" advances its bundle`.
    pub(crate) fn advances(&mut self, name: &str) {
        let files = demo_tree(
            "2.0.0",
            "Demo bundle v2",
            "demo-frag",
            "Demo fragment content, version two.",
            false,
        );
        self.advance_with(name, &files);
    }

    /// Go's `the remote "<name>" changes fragment "<fragment>" to "<content>"`.
    pub(crate) fn changes_fragment(&mut self, name: &str, fragment: &str, content: &str) {
        let files = demo_tree("1.1.0", "Demo bundle", fragment, content, true);
        self.advance_with(name, &files);
    }

    /// Go's `the remote "<name>" ships an MCP server "<server>" running
    /// "<command>" and a skill script printing "<say>"`.
    pub(crate) fn ships(&mut self, name: &str, server: &str, command: &str, say: &str) {
        let root = tree_bundle("demo");
        let files = vec![
            (
                format!("{root}/mcp/{server}.yaml"),
                format!("command: {command}\nenv:\n  API_KEY: secret-{say}\n"),
            ),
            (
                format!("{root}/skills/runner/SKILL.md"),
                "---\nname: runner\ndescription: Runs things.\n---\n\nbody\n".to_string(),
            ),
            (
                format!("{root}/skills/runner/scripts/run.sh"),
                format!("#!/bin/sh\necho {say}\n"),
            ),
        ];
        self.advance_with(name, &files);
    }

    /// Go's `UnpublishFromRemote`: a commit that deletes `paths`.
    pub(crate) fn unpublish(&mut self, name: &str, paths: &[String]) {
        let work = self.work_clone(name);
        for rel in paths {
            std::fs::remove_file(work.join(rel)).expect("a file to unpublish");
        }
        git_in(&work, &["add", "-A"]);
        git_in(&work, &["commit", "-m", "unpublish"]);
        git_in(&work, &["push", "origin", "main"]);
    }

    /// Go's `the remote "<name>" stops publishing bundle "<bundle>"`.
    pub(crate) fn stops_publishing(&mut self, name: &str, bundle: &str) {
        let root = tree_bundle(bundle);
        self.unpublish(
            name,
            &[
                format!("{root}/bundle.yaml"),
                format!("{root}/fragments/demo-frag.md"),
                format!("{root}/prompts/demo-skill.md"),
            ],
        );
    }

    /// Go's `BreakRemote`: the bare repository renamed out from under its
    /// `file://` address, so every later clone or fetch fails.
    pub(crate) fn becomes_unreachable(&mut self, name: &str) {
        let bare = self.bare[name].clone();
        let broken = PathBuf::from(format!("{}.unreachable", bare.display()));
        std::fs::rename(&bare, &broken).expect("the remote moves away");
        self.bare.insert(name.to_string(), broken);
    }

    /// Tag the remote's default branch tip `tag`.
    pub(crate) fn tag_remote(&self, name: &str, tag: &str) {
        git_in(&self.bare[name], &["tag", tag, "HEAD"]);
    }

    /// Go's `RemoteFile`: `path` as the remote's default branch holds it.
    pub(crate) fn remote_file(&self, name: &str, path: &str) -> String {
        git_in(&self.bare[name], &["show", &format!("HEAD:{path}")])
    }

    /// Go's `the remote "<name>"'s cached clone is forced back to its first
    /// commit`: the one clone under `.ctxloom/cache/repos`.
    pub(crate) fn clone_forced_back_to_first_commit(&self) {
        let clone = find_clone(&self.project.dir().join(".ctxloom/cache/repos"))
            .expect("a cached clone of the remote");
        let first = git_in(&clone, &["rev-list", "--max-parents=0", "HEAD"]);
        git_in(&clone, &["reset", "--hard", first.trim()]);
    }

    /// The text of a file of the project, if it is there.
    pub(crate) fn file(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.project.dir().join(rel)).ok()
    }

    pub(crate) fn lock(&self) -> String {
        self.file(".ctxloom/lock.yaml").expect("a lockfile")
    }

    /// `dev` including `origin/demo`, pulled once: the state most scenarios
    /// start from.
    pub(crate) fn pulled_dev() -> World {
        let mut world = World::initialized();
        world.git_remote("origin");
        world.run(&["remote", "default", "origin"]).succeeds();
        world.profile_including("dev", &[("origin", "demo")]);
        world.run(&["deps", "pull"]).succeeds();
        world
    }

    /// `dev` including `origin/demo`, not yet pulled.
    pub(crate) fn unpulled_dev() -> World {
        let mut world = World::initialized();
        world.git_remote("origin");
        world.run(&["remote", "default", "origin"]).succeeds();
        world.profile_including("dev", &[("origin", "demo")]);
        world
    }
}

/// The first directory below `dir` holding a `.git`.
fn find_clone(dir: &Path) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.join(".git").is_dir() {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_clone(&path) {
                return Some(found);
            }
        }
    }
    None
}
