//! In-memory ports for the application's tests: git repositories with commits,
//! branches and tags (and a clone that is as stale as the last fetch left it),
//! the project's files, and its authored bundles.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};

use ctxloom_ports::{Change, ContentRepo, Failure, TreeFiles};
use deps_ports::{
    CopyOutcome, DepsFiles, Endpoint, Forge, ForgeRepo, Opened, PullRequest, Remotes, RepoFile,
    RepoTree, Why,
};

use crate::Deps;

/// Where bundles are in a repository.
pub const BUNDLES: &str = ".ctxloom/content/bundles/v2";

/// One commit: the whole tree at it.
#[derive(Clone)]
struct Commit {
    sha: String,
    files: BTreeMap<String, (Vec<u8>, bool)>,
}

#[derive(Default)]
struct Repo {
    commits: Vec<Commit>,
    branches: BTreeMap<String, String>,
    tags: BTreeMap<String, String>,
    unreachable: bool,
    cloned: bool,
    /// The refs and tags and how many commits the last fetch saw.
    seen_branches: BTreeMap<String, String>,
    seen_tags: BTreeMap<String, String>,
    seen_commits: usize,
}

impl Repo {
    fn fetch(&mut self) {
        self.cloned = true;
        self.seen_branches.clone_from(&self.branches);
        self.seen_tags.clone_from(&self.tags);
        self.seen_commits = self.commits.len();
    }

    fn tree_at(&self, sha: &str) -> Option<&Commit> {
        self.commits[..self.seen_commits]
            .iter()
            .find(|c| c.sha == sha)
    }

    fn tip(&self) -> Option<&Commit> {
        let sha = self.seen_branches.get("main")?;
        self.tree_at(sha)
    }
}

/// Git repositories by address.
#[derive(Default)]
pub struct FakeRemotes {
    repos: Mutex<BTreeMap<String, Repo>>,
    /// Every `refresh` and `ensure`, in order, as `refresh <url>`.
    calls: Mutex<Vec<String>>,
}

impl FakeRemotes {
    fn repos(&self) -> MutexGuard<'_, BTreeMap<String, Repo>> {
        self.repos.lock().expect("the repositories")
    }

    /// A repository to clone from, with no commit yet.
    pub fn serve(&self, url: &str) {
        self.repos().insert(url.to_string(), Repo::default());
    }

    /// A commit on `main` that writes `files` over the last one's tree.
    pub fn commit(&self, url: &str, files: &[(&str, &str)]) -> String {
        self.commit_modes(
            url,
            &files
                .iter()
                .map(|(p, t)| (*p, *t, false))
                .collect::<Vec<_>>(),
        )
    }

    /// As [`commit`](FakeRemotes::commit), each file with its execute bit.
    pub fn commit_modes(&self, url: &str, files: &[(&str, &str, bool)]) -> String {
        let mut repos = self.repos();
        let repo = repos.get_mut(url).expect("a served repository");
        let mut tree = repo
            .commits
            .last()
            .map(|c| c.files.clone())
            .unwrap_or_default();
        for (path, text, executable) in files {
            tree.insert((*path).to_string(), (text.as_bytes().to_vec(), *executable));
        }
        let sha = format!("{:040x}", repo.commits.len() + 1);
        repo.commits.push(Commit {
            sha: sha.clone(),
            files: tree,
        });
        repo.branches.insert("main".to_string(), sha.clone());
        sha
    }

    /// A commit on `main` that deletes `paths`.
    pub fn delete(&self, url: &str, paths: &[&str]) -> String {
        let mut repos = self.repos();
        let repo = repos.get_mut(url).expect("a served repository");
        let mut tree = repo
            .commits
            .last()
            .map(|c| c.files.clone())
            .unwrap_or_default();
        for path in paths {
            tree.remove(*path);
        }
        let sha = format!("{:040x}", repo.commits.len() + 1);
        repo.commits.push(Commit {
            sha: sha.clone(),
            files: tree,
        });
        repo.branches.insert("main".to_string(), sha.clone());
        sha
    }

    pub fn tag(&self, url: &str, name: &str, sha: &str) {
        self.repos()
            .get_mut(url)
            .expect("a served repository")
            .tags
            .insert(name.to_string(), sha.to_string());
    }

    pub fn branch(&self, url: &str, name: &str, sha: &str) {
        self.repos()
            .get_mut(url)
            .expect("a served repository")
            .branches
            .insert(name.to_string(), sha.to_string());
    }

    /// Take the repository off the air (or put it back).
    pub fn set_unreachable(&self, url: &str, off: bool) {
        self.repos()
            .get_mut(url)
            .expect("a served repository")
            .unreachable = off;
    }

    /// The `refresh`es and `ensure`s made, in order.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("the calls").clone()
    }

    /// The commit at the tip of `main` on the remote itself.
    pub fn head(&self, url: &str) -> String {
        self.repos()[url].branches["main"].clone()
    }

    /// A file of the remote's tip.
    pub fn file(&self, url: &str, path: &str) -> Option<String> {
        let repos = self.repos();
        let repo = &repos[url];
        let tip = repo
            .commits
            .iter()
            .find(|c| Some(&c.sha) == repo.branches.get("main"))?;
        tip.files
            .get(path)
            .map(|(data, _)| String::from_utf8_lossy(data).into_owned())
    }

    fn cloned<T>(&self, url: &str, read: impl FnOnce(&Repo) -> Result<T, Why>) -> Result<T, Why> {
        let repos = self.repos();
        match repos.get(url) {
            Some(repo) if repo.cloned => read(repo),
            _ => Err(format!("{url} is not cloned yet")),
        }
    }
}

impl Remotes for FakeRemotes {
    fn local_repository(&self, path: &str) -> Result<String, Why> {
        if path.starts_with("/repos/") {
            Ok(format!("file://{path}"))
        } else {
            Err(format!("{path:?} holds no git repository"))
        }
    }

    fn ensure(&self, url: &str) -> Result<(), Why> {
        self.calls
            .lock()
            .expect("the calls")
            .push(format!("ensure {url}"));
        let mut repos = self.repos();
        let repo = repos
            .get_mut(url)
            .ok_or_else(|| format!("'{url}' does not appear to be a git repository"))?;
        if !repo.cloned {
            if repo.unreachable {
                return Err(format!("could not read from {url}"));
            }
            repo.fetch();
        }
        Ok(())
    }

    fn refresh(&self, url: &str) -> Result<(), Why> {
        self.calls
            .lock()
            .expect("the calls")
            .push(format!("refresh {url}"));
        let mut repos = self.repos();
        let repo = repos
            .get_mut(url)
            .ok_or_else(|| format!("'{url}' does not appear to be a git repository"))?;
        if repo.unreachable {
            return Err(format!("could not read from {url}"));
        }
        repo.fetch();
        Ok(())
    }

    fn tags(&self, url: &str) -> Result<Vec<String>, Why> {
        self.cloned(url, |repo| Ok(repo.seen_tags.keys().cloned().collect()))
    }

    fn resolve_ref(&self, url: &str, name: &str) -> Result<String, Why> {
        self.cloned(url, |repo| {
            repo.seen_branches
                .get(name)
                .or_else(|| repo.seen_tags.get(name))
                .cloned()
                .or_else(|| repo.tree_at(name).map(|c| c.sha.clone()))
                .ok_or_else(|| format!("unknown revision {name}"))
        })
    }

    fn resolve_tag(&self, url: &str, name: &str) -> Result<String, Why> {
        self.cloned(url, |repo| {
            repo.seen_tags
                .get(name)
                .cloned()
                .ok_or_else(|| format!("unknown tag {name}"))
        })
    }

    fn default_branch(&self, url: &str) -> Result<String, Why> {
        self.cloned(url, |_| Ok("main".to_string()))
    }

    fn read_bundle(&self, url: &str, sha: &str, bundle: &str) -> Result<Option<RepoTree>, Why> {
        self.cloned(url, |repo| {
            let commit = repo
                .tree_at(sha)
                .ok_or_else(|| format!("bad object {sha}"))?;
            let prefix = format!("{BUNDLES}/{bundle}/");
            let tree: RepoTree = commit
                .files
                .iter()
                .filter_map(|(path, (data, executable))| {
                    path.strip_prefix(&prefix).map(|rel| {
                        (
                            rel.to_string(),
                            RepoFile {
                                data: data.clone(),
                                executable: *executable,
                            },
                        )
                    })
                })
                .collect();
            Ok((!tree.is_empty()).then_some(tree))
        })
    }

    fn list_bundles(&self, url: &str) -> Result<Vec<String>, Why> {
        self.cloned(url, |repo| {
            let tip = repo.tip().ok_or("no default branch")?;
            let prefix = format!("{BUNDLES}/");
            let mut names: Vec<String> = tip
                .files
                .keys()
                .filter_map(|p| p.strip_prefix(&prefix)?.strip_suffix("/bundle.yaml"))
                .map(str::to_string)
                .collect();
            names.sort();
            Ok(names)
        })
    }

    fn has_bundle(&self, url: &str, bundle: &str) -> Result<bool, Why> {
        self.cloned(url, |repo| {
            let tip = repo.tip().ok_or("no default branch")?;
            Ok(tip
                .files
                .contains_key(&format!("{BUNDLES}/{bundle}/bundle.yaml")))
        })
    }

    fn publish(
        &self,
        url: &str,
        files: &BTreeMap<String, Vec<u8>>,
        _message: &str,
    ) -> Result<String, Why> {
        {
            let repos = self.repos();
            let repo = repos.get(url).ok_or("no such repository")?;
            if repo.unreachable {
                return Err(format!("could not read from {url}"));
            }
        }
        let pairs: Vec<(String, String)> = files
            .iter()
            .map(|(p, d)| (p.clone(), String::from_utf8_lossy(d).into_owned()))
            .collect();
        let refs: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(p, t)| (p.as_str(), t.as_str()))
            .collect();
        let unchanged = {
            let repos = self.repos();
            let repo = &repos[url];
            repo.commits.last().is_some_and(|tip| {
                refs.iter()
                    .all(|(p, t)| tip.files.get(*p).is_some_and(|(d, _)| d == t.as_bytes()))
            })
        };
        if unchanged {
            return Ok(self.head(url));
        }
        Ok(self.commit(url, &refs))
    }
}

#[derive(Default)]
struct FilesState {
    registry: Option<String>,
    lock: Option<String>,
    installed: BTreeMap<(String, String), RepoTree>,
    local: BTreeMap<String, BTreeMap<String, Vec<u8>>>,
    directories: BTreeSet<String>,
    placed: BTreeMap<String, BTreeMap<String, Vec<u8>>>,
    fail_install: bool,
    lock_writes: u64,
    registry_writes: u64,
}

/// The project's files.
#[derive(Default)]
pub struct FakeFiles {
    state: Mutex<FilesState>,
}

impl FakeFiles {
    fn state(&self) -> MutexGuard<'_, FilesState> {
        self.state.lock().expect("the files")
    }

    pub fn set_lock(&self, text: &str) {
        self.state().lock = Some(text.to_string());
    }

    pub fn lock_text(&self) -> Option<String> {
        self.state().lock.clone()
    }

    pub fn set_registry(&self, text: &str) {
        self.state().registry = Some(text.to_string());
    }

    pub fn registry_text(&self) -> Option<String> {
        self.state().registry.clone()
    }

    pub fn lock_writes(&self) -> u64 {
        self.state().lock_writes
    }

    pub fn registry_writes(&self) -> u64 {
        self.state().registry_writes
    }

    /// An authored bundle of the project.
    pub fn author(&self, name: &str, files: &[(&str, &str)]) {
        self.state().local.insert(
            name.to_string(),
            files
                .iter()
                .map(|(p, t)| ((*p).to_string(), t.as_bytes().to_vec()))
                .collect(),
        );
    }

    pub fn has_authored(&self, name: &str) -> bool {
        self.state().local.contains_key(name)
    }

    /// A directory that exists.
    pub fn directory(&self, path: &str) {
        self.state().directories.insert(path.to_string());
    }

    /// What was copied to `dest`.
    pub fn placed(&self, dest: &str) -> Option<BTreeMap<String, Vec<u8>>> {
        self.state().placed.get(dest).cloned()
    }

    pub fn fail_installs(&self, on: bool) {
        self.state().fail_install = on;
    }

    pub fn installed_at(&self, identity: &str, sha: &str) -> Option<RepoTree> {
        self.state()
            .installed
            .get(&(identity.to_string(), sha.to_string()))
            .cloned()
    }

    pub fn installed_identities(&self) -> Vec<(String, String)> {
        self.state().installed.keys().cloned().collect()
    }

    /// Delete an installed tree, as a cleaned cache would.
    pub fn uninstall(&self, identity: &str, sha: &str) {
        self.state()
            .installed
            .remove(&(identity.to_string(), sha.to_string()));
    }
}

impl DepsFiles for FakeFiles {
    fn registry_path(&self) -> String {
        "/p/.ctxloom/remotes.yaml".to_string()
    }

    fn read_registry(&self) -> Result<Option<String>, Why> {
        Ok(self.state().registry.clone())
    }

    fn write_registry(&self, text: &str) -> Result<(), Why> {
        let mut state = self.state();
        state.registry = Some(text.to_string());
        state.registry_writes += 1;
        Ok(())
    }

    fn lock_path(&self) -> String {
        "/p/.ctxloom/lock.yaml".to_string()
    }

    fn read_lock(&self) -> Result<Option<String>, Why> {
        Ok(self.state().lock.clone())
    }

    fn write_lock(&self, text: &str) -> Result<(), Why> {
        let mut state = self.state();
        state.lock = Some(text.to_string());
        state.lock_writes += 1;
        Ok(())
    }

    fn is_installed(&self, identity: &str, sha: &str) -> bool {
        self.installed_at(identity, sha).is_some()
    }

    fn install(&self, identity: &str, sha: &str, tree: &RepoTree) -> Result<(), Why> {
        let mut state = self.state();
        if state.fail_install {
            return Err("disk full".to_string());
        }
        state.installed.retain(|(i, _), _| i != identity);
        state
            .installed
            .insert((identity.to_string(), sha.to_string()), tree.clone());
        Ok(())
    }

    fn installed_path(&self, _identity: &str, sha: &str) -> String {
        format!("/p/.ctxloom/cache/bundles/{sha}")
    }

    fn read_installed(&self, identity: &str, sha: &str) -> Result<Option<RepoTree>, Why> {
        Ok(self.installed_at(identity, sha))
    }

    fn prune(&self, keep: &[(String, String)]) -> Result<Vec<String>, Why> {
        let mut state = self.state();
        let mut gone = Vec::new();
        let doomed: Vec<(String, String)> = state
            .installed
            .keys()
            .filter(|key| !keep.contains(key))
            .cloned()
            .collect();
        for key in doomed {
            state.installed.remove(&key);
            if !keep.iter().any(|(i, _)| *i == key.0) && !gone.contains(&key.0) {
                gone.push(key.0);
            }
        }
        Ok(gone)
    }

    fn local_bundle(&self, name: &str) -> Result<Option<String>, Why> {
        Ok(self
            .state()
            .local
            .contains_key(name)
            .then(|| format!("/p/.ctxloom/content/bundles/v2/{name}/bundle.yaml")))
    }

    fn read_local_bundle(&self, name: &str) -> Result<BTreeMap<String, Vec<u8>>, Why> {
        Ok(self.state().local.get(name).cloned().unwrap_or_default())
    }

    fn remove_local_bundle(&self, name: &str) -> Result<(), Why> {
        self.state().local.remove(name);
        Ok(())
    }

    fn is_directory(&self, path: &str) -> bool {
        self.state().directories.contains(path)
    }

    fn exists(&self, path: &str) -> bool {
        let state = self.state();
        state.directories.contains(path) || state.placed.contains_key(path)
    }

    fn copy_bundle(
        &self,
        dir: &str,
        name: &str,
        files: &BTreeMap<String, Vec<u8>>,
        force: bool,
    ) -> Result<CopyOutcome, Why> {
        let dest = format!("{dir}/.ctxloom/content/bundles/v2/{name}");
        let mut state = self.state();
        if dir == "." {
            return Ok(CopyOutcome::Itself(dest));
        }
        if state.placed.contains_key(&dest) && !force {
            return Ok(CopyOutcome::Exists(dest));
        }
        state.placed.insert(dest.clone(), files.clone());
        Ok(CopyOutcome::Copied(dest))
    }
}

/// The project's authored bundles, for the closure's roots.
#[derive(Default)]
pub struct FakeContent {
    bundles: Mutex<BTreeMap<String, TreeFiles>>,
}

impl FakeContent {
    /// A bundle with these files (the envelope included where given).
    pub fn bundle(&self, name: &str, files: &[(&str, &str)]) {
        self.bundles.lock().expect("the bundles").insert(
            name.to_string(),
            files
                .iter()
                .map(|(p, t)| ((*p).to_string(), t.as_bytes().to_vec()))
                .collect(),
        );
    }
}

impl ContentRepo for FakeContent {
    fn has_root(&self) -> Result<bool, Failure> {
        Ok(!self.bundles.lock().expect("the bundles").is_empty())
    }

    fn bundle_names(&self) -> Result<Vec<String>, Failure> {
        Ok(self
            .bundles
            .lock()
            .expect("the bundles")
            .keys()
            .cloned()
            .collect())
    }

    fn read_tree(&self, name: &str) -> Result<TreeFiles, Failure> {
        Ok(self
            .bundles
            .lock()
            .expect("the bundles")
            .get(name)
            .cloned()
            .unwrap_or_default())
    }

    fn manifest_path(&self, name: &str) -> String {
        format!("/p/.ctxloom/content/bundles/v2/{name}/bundle.yaml")
    }

    fn executable_files(&self, _name: &str) -> Result<BTreeSet<String>, Failure> {
        Ok(BTreeSet::new())
    }

    fn apply(&self, name: &str, changes: &[Change]) -> Result<(), Failure> {
        let mut bundles = self.bundles.lock().expect("the bundles");
        let tree = bundles.entry(name.to_string()).or_default();
        for change in changes {
            match change {
                Change::Write(put) => {
                    tree.insert(put.path.clone(), put.bytes.clone());
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
        let mut bundles = self.bundles.lock().expect("the bundles");
        if bundles.contains_key(name) {
            return Ok(false);
        }
        bundles.insert(name.to_string(), TreeFiles::default());
        Ok(true)
    }

    fn remove_bundle(&self, name: &str) -> Result<(), Failure> {
        self.bundles.lock().expect("the bundles").remove(name);
        Ok(())
    }

    fn bundle_dir(&self, name: &str) -> String {
        format!("/p/.ctxloom/content/bundles/v2/{name}")
    }
}

/// A forge's API: the repositories it serves, and a record of what it was
/// asked.
#[derive(Default)]
pub struct FakeForge {
    state: Mutex<ForgeState>,
}

#[derive(Default)]
struct ForgeState {
    repos: Vec<ForgeRepo>,
    search_error: Option<String>,
    pull_request_error: Option<String>,
    searches: Vec<(Endpoint, String, usize)>,
    opened: Vec<(Endpoint, PullRequest)>,
}

impl FakeForge {
    fn state(&self) -> MutexGuard<'_, ForgeState> {
        self.state.lock().expect("the forge's state")
    }

    /// The repositories every search finds.
    pub fn serves(&self, repos: Vec<ForgeRepo>) {
        self.state().repos = repos;
    }

    /// Make every search fail with `why`.
    pub fn fail_search(&self, why: &str) {
        self.state().search_error = Some(why.to_string());
    }

    /// Make every pull request fail with `why`.
    pub fn fail_pull_requests(&self, why: &str) {
        self.state().pull_request_error = Some(why.to_string());
    }

    /// Every search asked: endpoint, query, limit.
    pub fn searches(&self) -> Vec<(Endpoint, String, usize)> {
        self.state().searches.clone()
    }

    /// Every pull request opened, with the endpoint it went to.
    pub fn opened(&self) -> Vec<(Endpoint, PullRequest)> {
        self.state().opened.clone()
    }
}

impl Forge for FakeForge {
    fn search_repositories(
        &self,
        endpoint: &Endpoint,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ForgeRepo>, Why> {
        let mut state = self.state();
        state
            .searches
            .push((endpoint.clone(), query.to_string(), limit));
        match &state.search_error {
            Some(why) => Err(why.clone()),
            None => Ok(state.repos.iter().take(limit).cloned().collect()),
        }
    }

    fn open_pull_request(&self, endpoint: &Endpoint, request: &PullRequest) -> Result<Opened, Why> {
        let mut state = self.state();
        if let Some(why) = &state.pull_request_error {
            return Err(why.clone());
        }
        state.opened.push((endpoint.clone(), request.clone()));
        Ok(Opened {
            url: format!(
                "https://github.com/{}/{}/pull/7",
                request.owner, request.repo
            ),
            commit: "c0ffee".repeat(6) + "c0ff",
            branch: format!("{}-1700000000", request.branch),
        })
    }
}

/// The use cases over the fakes, and the fakes themselves.
pub struct Fixture {
    pub remotes: Arc<FakeRemotes>,
    pub files: Arc<FakeFiles>,
    pub content: Arc<FakeContent>,
    pub forge: Arc<FakeForge>,
    pub deps: Deps,
}

/// The envelope every authored bundle has.
pub const ENVELOPE: (&str, &str) = ("bundle.yaml", "schema_version: 4\nversion: 1.0.0\n");

impl Fixture {
    pub fn new() -> Fixture {
        let remotes = Arc::new(FakeRemotes::default());
        let files = Arc::new(FakeFiles::default());
        let content = Arc::new(FakeContent::default());
        let forge = Arc::new(FakeForge::default());
        let deps =
            Deps::new(remotes.clone(), files.clone(), content.clone()).with_forge(forge.clone());
        Fixture {
            remotes,
            files,
            content,
            forge,
            deps,
        }
    }

    /// A repository serving the demo bundle, registered as `name`; the commit.
    pub fn origin(&self, name: &str, url: &str) -> String {
        self.remotes.serve(url);
        let sha = self
            .remotes
            .commit(url, &demo("1.0.0", "Demo fragment content."));
        self.register(name, url);
        sha
    }

    /// Register `name` at `url` as the registry file says it.
    pub fn register(&self, name: &str, url: &str) {
        self.register_on(name, url, "git");
    }

    /// Register `name` at `url`, bound to the forge `forge`.
    pub fn register_on(&self, name: &str, url: &str, forge: &str) {
        let mut registry = match self.files.registry_text() {
            Some(text) => {
                deps::registry::Registry::decode(&text, "remotes.yaml").expect("a registry")
            }
            None => deps::registry::Registry::empty(),
        };
        registry.add(name, url, forge).expect("registers");
        self.files.set_registry(&registry.encode());
    }

    /// A profile of the project bundle naming these bundles.
    pub fn profile(&self, name: &str, bundles: &[&str]) {
        let list: String = bundles.iter().map(|b| format!("  - {b}\n")).collect();
        self.content.bundle(
            "project",
            &[
                ENVELOPE,
                (
                    &format!("profiles/{name}.yaml"),
                    &format!("bundles:\n{list}"),
                ),
            ],
        );
    }

    /// The canonical reference to `bundle` of the repository at `url`.
    pub fn reference(url: &str, bundle: &str) -> String {
        deps::address::canonical_ref(url, bundle).expect("a reference")
    }
}

impl Default for Fixture {
    fn default() -> Fixture {
        Fixture::new()
    }
}

/// The demo bundle's files in a repository.
pub fn demo(version: &str, fragment: &str) -> Vec<(&'static str, &'static str)> {
    // Leaked: a test's strings live as long as the test run.
    let leak = |text: String| -> &'static str { Box::leak(text.into_boxed_str()) };
    vec![
        (
            leak(format!("{BUNDLES}/demo/bundle.yaml")),
            leak(format!(
                "version: \"{version}\"\ndescription: \"Demo bundle\"\n"
            )),
        ),
        (
            leak(format!("{BUNDLES}/demo/fragments/demo-frag.md")),
            leak(format!("---\ntags: [demo]\n---\n\n{fragment}\n")),
        ),
    ]
}

#[cfg(test)]
#[path = "fakes.test.rs"]
mod tests;

/// A project refresh that answers as it is told: what the preview would
/// change (`planned`), or what refused it (`refused`), and the same when
/// written. With neither, the project is current.
#[derive(Default)]
pub struct FakeRefresh {
    pub planned: Vec<crate::RefreshEngine>,
    pub refused: Option<String>,
    pub asked: Mutex<Vec<bool>>,
}

impl crate::ProjectRefresh for FakeRefresh {
    // A test double: the tests that use it are what hold it, not its own.
    #[cfg_attr(test, mutants::skip)]
    fn refresh(
        &self,
        write: bool,
    ) -> Result<crate::Refresh, Box<dyn angzarr_diagnostic::Diagnostic>> {
        self.asked.lock().expect("a lock").push(write);
        if let Some(detail) = &self.refused {
            return Err(Box::new(crate::AppError::Required {
                what: detail.clone(),
            }));
        }
        Ok(crate::Refresh {
            target: "/project".to_string(),
            engines: self
                .planned
                .iter()
                .map(|e| crate::RefreshEngine {
                    engine: e.engine.clone(),
                    wrote: e.wrote.clone(),
                    released: e.released.clone(),
                    refused: Vec::new(),
                })
                .collect(),
        })
    }
}
