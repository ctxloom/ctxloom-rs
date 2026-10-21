//! Resolution over a real filesystem: a temporary home and work tree.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use annals_diagnostic::Diagnostic;
use ports::{Names, ProjectIdentity};
use tempfile::TempDir;

use super::{index, Environment, ProjectRegistry, MINT_ATTEMPTS};

/// Project ids in the order given, then `then` forever.
pub(crate) struct Scripted {
    ids: VecDeque<String>,
    then: String,
    minted: u32,
}

impl Names for Scripted {
    fn task_id(&mut self) -> String {
        unreachable!("no task ids here")
    }

    fn project_id(&mut self) -> String {
        self.minted += 1;
        self.ids.pop_front().unwrap_or_else(|| self.then.clone())
    }
}

pub(crate) fn names(ids: &[&str]) -> Scripted {
    Scripted {
        ids: ids.iter().map(|s| s.to_string()).collect(),
        then: "always-the-same".to_string(),
        minted: 0,
    }
}

/// A home and a work tree, apart.
pub(crate) struct World {
    pub(crate) home: TempDir,
    pub(crate) tree: TempDir,
}

impl World {
    pub(crate) fn new() -> World {
        World {
            home: tempfile::tempdir().unwrap(),
            tree: tempfile::tempdir().unwrap(),
        }
    }

    /// The work tree, canonical (a temporary directory's path may run
    /// through a symlink, as macOS's /tmp does).
    pub(crate) fn tree(&self) -> PathBuf {
        fs::canonicalize(self.tree.path()).unwrap()
    }

    pub(crate) fn env(&self) -> Environment {
        Environment {
            home: Some(self.home.path().to_path_buf()),
            cwd: Ok(self.tree()),
            root: None,
            project_id: None,
        }
    }

    pub(crate) fn registry(&self, ids: &[&str]) -> ProjectRegistry<Scripted> {
        ProjectRegistry::new(self.env(), names(ids))
    }

    pub(crate) fn index(&self) -> index::Index {
        index::load(&index::path(self.home.path())).unwrap()
    }

    pub(crate) fn marker(&self) -> Option<String> {
        fs::read_to_string(self.tree().join(".project-id")).ok()
    }

    pub(crate) fn store(&self, id: &str) -> PathBuf {
        self.home
            .path()
            .join(".ctxloom/projects")
            .join(id)
            .join("state.db")
    }
}

pub(crate) fn text(path: &Path) -> String {
    path.display().to_string()
}

// ------------------------------------------------------------ work root

#[test]
fn test_the_work_root_is_ctxloom_root_when_set() {
    let world = World::new();
    let mut env = world.env();
    env.root = Some(PathBuf::from("/somewhere/else"));
    let registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(registry.work_root().unwrap(), Path::new("/somewhere/else"));
}

#[test]
fn test_the_work_root_is_the_nearest_ancestor_holding_git() {
    let world = World::new();
    fs::create_dir(world.tree().join(".git")).unwrap();
    let deep = world.tree().join("a/b");
    fs::create_dir_all(&deep).unwrap();
    fs::create_dir_all(world.tree().join("a/.git")).unwrap();
    let mut env = world.env();
    env.cwd = Ok(deep);
    env.root = Some(PathBuf::new());
    let registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(
        registry.work_root().unwrap(),
        world.tree().join("a"),
        "an empty CTXLOOM_ROOT is unset"
    );
}

#[test]
fn test_a_linked_worktrees_git_file_marks_its_root() {
    let world = World::new();
    fs::write(world.tree().join(".git"), "gitdir: /elsewhere\n").unwrap();
    let mut env = world.env();
    env.cwd = Ok(world.tree().join("sub"));
    let registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(registry.work_root().unwrap(), world.tree());
}

#[test]
fn test_with_no_git_above_the_work_root_is_the_working_directory() {
    let world = World::new();
    assert_eq!(world.registry(&[]).work_root().unwrap(), world.tree());
}

#[test]
fn test_the_work_root_is_canonical_symlinks_resolved_and_slashes_cleaned() {
    let world = World::new();
    let link = world.home.path().join("link-to-tree");
    std::os::unix::fs::symlink(world.tree(), &link).unwrap();
    for root in [link.clone(), link.join(""), link.join(".")] {
        let mut env = world.env();
        env.root = Some(root.clone());
        let registry = ProjectRegistry::new(env, names(&[]));
        assert_eq!(registry.work_root().unwrap(), world.tree(), "{root:?}");
    }
}

#[test]
fn test_a_work_root_that_does_not_resolve_is_cleaned_lexically() {
    let world = World::new();
    let mut env = world.env();
    env.root = Some(PathBuf::from("/no/such/./tree/../root/"));
    let registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(registry.work_root().unwrap(), Path::new("/no/such/root"));
}

#[test]
fn test_a_relative_ctxloom_root_is_under_the_working_directory() {
    let world = World::new();
    fs::create_dir(world.tree().join("sub")).unwrap();
    let mut env = world.env();
    env.root = Some(PathBuf::from("sub/"));
    let registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(registry.work_root().unwrap(), world.tree().join("sub"));
}

#[test]
fn test_a_working_directory_that_cannot_be_read_is_an_error() {
    let world = World::new();
    let mut env = world.env();
    env.cwd = Err("No such file or directory (os error 2)".to_string());
    let mut registry = ProjectRegistry::new(env.clone(), names(&["fresh-new-id"]));
    let error = registry.for_write(None).unwrap_err();
    assert_eq!(error.code(), "project-registry-no-cwd");
    assert!(error.to_string().contains("os error 2"), "{error}");
    assert_eq!(registry.names.minted, 0);
    env.root = Some(world.tree());
    let registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(
        registry.work_root().unwrap(),
        world.tree(),
        "CTXLOOM_ROOT needs no working directory"
    );
}

// ------------------------------------------------------------ pinned ids

#[test]
fn test_a_pinned_id_wins_over_the_environment_and_the_marker() {
    let world = World::new();
    fs::write(world.tree().join(".project-id"), "from-marker\n").unwrap();
    let mut env = world.env();
    env.project_id = Some("from-env".to_string());
    let mut registry = ProjectRegistry::new(env, names(&[]));
    let project = registry.for_write(Some("from-flag")).unwrap();
    assert_eq!(project.id, "from-flag");
    assert_eq!(project.root, text(&world.tree()));
    assert_eq!(project.store, world.store("from-flag"));
    let read = registry.for_read(Some("from-flag")).unwrap().unwrap();
    assert_eq!(read, project);
}

#[test]
fn test_ctxloom_project_id_wins_over_the_marker() {
    let world = World::new();
    fs::write(world.tree().join(".project-id"), "from-marker\n").unwrap();
    let mut env = world.env();
    env.project_id = Some("from-env".to_string());
    let mut registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(registry.for_read(None).unwrap().unwrap().id, "from-env");
    assert_eq!(registry.for_read(Some("")).unwrap().unwrap().id, "from-env");
}

#[test]
fn test_an_empty_ctxloom_project_id_is_unset() {
    let world = World::new();
    let mut env = world.env();
    env.project_id = Some(String::new());
    let mut registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(registry.for_read(None).unwrap(), None);
}

#[test]
fn test_an_invalid_pinned_id_is_refused_naming_where_it_came_from() {
    let world = World::new();
    let error = world
        .registry(&[])
        .for_write(Some("../escape"))
        .unwrap_err();
    assert_eq!(error.code(), "project-registry-invalid-id");
    assert!(error.to_string().contains("--project"), "{error}");
    let mut env = world.env();
    env.project_id = Some("a/b".to_string());
    let error = ProjectRegistry::new(env, names(&[]))
        .for_read(None)
        .unwrap_err();
    assert!(error.to_string().contains("CTXLOOM_PROJECT_ID"), "{error}");
}

// ------------------------------------------------------------ markers

#[test]
fn test_the_marker_names_the_project() {
    let world = World::new();
    fs::write(world.tree().join(".project-id"), "  jolly-mute-magma \n").unwrap();
    let project = world.registry(&[]).for_read(None).unwrap().unwrap();
    assert_eq!(project.id, "jolly-mute-magma");
    assert_eq!(project.store, world.store("jolly-mute-magma"));
}

#[test]
fn test_a_blank_marker_names_nothing() {
    let world = World::new();
    fs::write(world.tree().join(".project-id"), "\n").unwrap();
    assert_eq!(world.registry(&[]).for_read(None).unwrap(), None);
}

#[test]
fn test_an_invalid_marker_is_refused() {
    let world = World::new();
    fs::write(world.tree().join(".project-id"), "../../etc\n").unwrap();
    let error = world.registry(&[]).for_read(None).unwrap_err();
    assert_eq!(error.code(), "project-registry-invalid-id");
}

#[test]
fn test_an_unreadable_marker_fails() {
    let world = World::new();
    fs::create_dir(world.tree().join(".project-id")).unwrap();
    let error = world.registry(&[]).for_read(None).unwrap_err();
    assert_eq!(error.code(), "project-registry-io");
}

#[test]
fn test_gos_older_marker_is_read_and_adopted_only_by_a_write() {
    let world = World::new();
    fs::create_dir(world.tree().join(".ctxloom")).unwrap();
    fs::write(world.tree().join(".ctxloom/project-id"), "old-go-id\n").unwrap();
    let mut registry = world.registry(&[]);
    assert_eq!(registry.for_read(None).unwrap().unwrap().id, "old-go-id");
    assert_eq!(world.marker(), None, "a read writes nothing");
    assert_eq!(registry.for_write(None).unwrap().id, "old-go-id");
    assert_eq!(world.marker().as_deref(), Some("old-go-id\n"));
    assert!(
        world.index().projects.is_empty(),
        "adopting registers nothing yet"
    );
}

// ------------------------------------------------------------ registry

#[test]
fn test_the_registry_names_the_project_of_a_registered_root() {
    let world = World::new();
    let mut index = index::Index::default();
    index.register("high-tight-gulf", &text(&world.tree()));
    index::save(&index::path(world.home.path()), &index).unwrap();
    let mut registry = world.registry(&[]);
    assert_eq!(
        registry.for_read(None).unwrap().unwrap().id,
        "high-tight-gulf"
    );
    assert_eq!(registry.for_write(None).unwrap().id, "high-tight-gulf");
    assert_eq!(registry.names.minted, 0);
}

#[test]
fn test_a_symlinked_or_slashed_ctxloom_root_finds_gos_entry() {
    // Go registers the path cleanPath gives: symlinks resolved.
    let world = World::new();
    let mut index = index::Index::default();
    index.register("high-tight-gulf", &text(&world.tree()));
    index::save(&index::path(world.home.path()), &index).unwrap();
    let link = world.home.path().join("link-to-tree");
    std::os::unix::fs::symlink(world.tree(), &link).unwrap();
    for root in [link.clone(), link.join(""), world.tree().join("")] {
        let mut env = world.env();
        env.root = Some(root.clone());
        let mut registry = ProjectRegistry::new(env, names(&[]));
        let project = registry.for_read(None).unwrap().expect("Go's entry");
        assert_eq!(project.id, "high-tight-gulf", "{root:?}");
        assert_eq!(project.root, text(&world.tree()));
    }
}

#[test]
fn test_an_entry_registered_through_a_symlink_matches_its_target() {
    let world = World::new();
    let link = world.home.path().join("link-to-tree");
    std::os::unix::fs::symlink(world.tree(), &link).unwrap();
    let mut index = index::Index::default();
    index.register("high-tight-gulf", &format!("{}/", link.display()));
    index::save(&index::path(world.home.path()), &index).unwrap();
    let project = world.registry(&[]).for_read(None).unwrap().expect("found");
    assert_eq!(project.id, "high-tight-gulf");
}

#[test]
fn test_a_mint_registers_the_canonical_path() {
    let world = World::new();
    let link = world.home.path().join("link-to-tree");
    std::os::unix::fs::symlink(world.tree(), &link).unwrap();
    let mut env = world.env();
    env.root = Some(link);
    ProjectRegistry::new(env, names(&["fresh-new-id"]))
        .for_write(None)
        .unwrap();
    assert_eq!(world.index().projects[0].path, text(&world.tree()));
}

#[test]
fn test_an_unreadable_registry_fails_a_read() {
    let world = World::new();
    let path = index::path(world.home.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "projects: [").unwrap();
    let error = world.registry(&[]).for_read(None).unwrap_err();
    assert_eq!(error.code(), "project-registry-unreadable");
}

// ------------------------------------------------------------ minting

#[test]
fn test_a_read_with_no_project_finds_none_and_writes_nothing() {
    let world = World::new();
    assert_eq!(
        world.registry(&["fresh-new-id"]).for_read(None).unwrap(),
        None
    );
    assert_eq!(world.marker(), None);
    assert!(!index::path(world.home.path()).exists());
}

#[test]
fn test_a_write_with_no_project_mints_and_binds_one() {
    let world = World::new();
    let mut registry = world.registry(&["fresh-new-id"]);
    let project = registry.for_write(None).unwrap();
    assert_eq!(project.id, "fresh-new-id");
    assert_eq!(project.root, text(&world.tree()));
    assert_eq!(project.store, world.store("fresh-new-id"));
    assert_eq!(world.marker().as_deref(), Some("fresh-new-id\n"));
    let index = world.index();
    assert_eq!(index.schema_version, 1);
    assert_eq!(index.id_for(&world.tree()), Some("fresh-new-id"));
    assert_eq!(
        registry.for_write(None).unwrap(),
        project,
        "bound: not minted again"
    );
    assert_eq!(registry.names.minted, 1);
}

#[test]
fn test_a_mint_skips_ids_already_registered() {
    let world = World::new();
    let mut index = index::Index::default();
    index.register("taken-id", "/another/tree");
    index::save(&index::path(world.home.path()), &index).unwrap();
    let project = world
        .registry(&["taken-id", "free-id"])
        .for_write(None)
        .unwrap();
    assert_eq!(project.id, "free-id");
    assert_eq!(world.index().projects.len(), 2);
}

#[test]
fn test_a_mint_gives_up_when_every_id_is_taken() {
    let world = World::new();
    let mut index = index::Index::default();
    index.register("always-the-same", "/another/tree");
    index::save(&index::path(world.home.path()), &index).unwrap();
    let mut registry = world.registry(&[]);
    let error = registry.for_write(None).unwrap_err();
    assert_eq!(error.code(), "project-registry-exhausted");
    assert_eq!(registry.names.minted, MINT_ATTEMPTS);
}

#[test]
fn test_binding_under_the_lock_takes_the_id_another_process_bound_first() {
    // The marker check before the lock found nothing; by the time the lock
    // is held another process has bound the root. `bind` re-checks.
    let world = World::new();
    let mut registry = world.registry(&["never-minted"]);
    fs::write(world.tree().join(".project-id"), "won-the-race\n").unwrap();
    assert_eq!(registry.bind(&world.tree()).unwrap(), "won-the-race");
    assert_eq!(registry.names.minted, 0);
}

#[test]
fn test_a_custom_lock_wait_is_the_one_a_write_waits_for() {
    // The lock is held here, through the registry's own lock file, so the
    // write must wait: it gives up after its short wait, long before the
    // default would have.
    let world = World::new();
    let held = index::lock_within(&index::path(world.home.path()), index::LOCK_WAIT).unwrap();
    let wait = std::time::Duration::from_millis(50);
    let mut registry = world.registry(&["never-minted"]).with_lock_wait(wait);
    let started = std::time::Instant::now();
    let error = registry.for_write(None).unwrap_err();
    let waited = started.elapsed();
    assert_eq!(error.code(), "project-registry-lock-timeout");
    assert!(waited >= wait, "it waited its wait: {waited:?}");
    assert!(
        waited < index::LOCK_WAIT / 2,
        "it gave up long before the default wait: {waited:?}"
    );
    assert_eq!(registry.names.minted, 0);
    drop(held);
}

#[test]
fn test_without_a_home_there_is_no_store_and_no_registry() {
    let world = World::new();
    for home in [None, Some(PathBuf::new())] {
        let mut env = world.env();
        env.home = home;
        let error = ProjectRegistry::new(env, names(&[]))
            .for_write(Some("pinned"))
            .unwrap_err();
        assert_eq!(error.code(), "project-registry-no-home");
    }
}

// ------------------------------------------------------------ linked worktrees

/// Repositories and worktrees made by git itself, in temporary
/// directories, with no configuration but their own.
pub(crate) mod git {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use tempfile::TempDir;

    pub struct Repo {
        dir: TempDir,
        bare: bool,
    }

    fn run(cwd: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(["-c", "init.defaultBranch=main"])
            .args(args)
            .current_dir(cwd)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    impl Repo {
        /// A repository with one commit, checked out.
        pub fn new() -> Repo {
            let repo = Repo {
                dir: tempfile::tempdir().unwrap(),
                bare: false,
            };
            fs::create_dir(repo.root()).unwrap();
            run(&repo.root(), &["init", "-q"]);
            run(
                &repo.root(),
                &["commit", "-q", "--allow-empty", "-m", "first"],
            );
            repo
        }

        /// A bare repository with one commit.
        pub fn bare() -> Repo {
            let source = Repo::new();
            let repo = Repo {
                dir: tempfile::tempdir().unwrap(),
                bare: true,
            };
            let root = repo.root();
            run(
                source.dir.path(),
                &[
                    "clone",
                    "-q",
                    "--bare",
                    &source.root().display().to_string(),
                    &root.display().to_string(),
                ],
            );
            repo
        }

        /// The checkout (or the bare repository), canonical.
        pub fn root(&self) -> PathBuf {
            fs::canonicalize(self.dir.path()).unwrap().join("main")
        }

        /// A linked worktree named `name` beside the checkout, canonical.
        pub fn worktree(&self, name: &str) -> PathBuf {
            let at = fs::canonicalize(self.dir.path()).unwrap().join(name);
            self.worktree_at(&at)
        }

        /// A linked worktree at `at`, canonical.
        pub fn worktree_at(&self, at: &Path) -> PathBuf {
            let branch = at.file_name().unwrap().to_string_lossy().to_string();
            let mut args = vec!["worktree", "add", "-q", "-b", &branch];
            let shown = at.display().to_string();
            args.push(&shown);
            if self.bare {
                args.push("main");
            }
            run(&self.root(), &args);
            fs::canonicalize(at).unwrap()
        }
    }
}

impl World {
    /// The world, working in `cwd`.
    pub(crate) fn env_in(&self, cwd: &Path) -> Environment {
        Environment {
            cwd: Ok(cwd.to_path_buf()),
            ..self.env()
        }
    }
}

#[test]
fn test_a_linked_worktree_reads_its_primary_checkouts_project() {
    let world = World::new();
    let repo = git::Repo::new();
    let linked = repo.worktree("linked");
    fs::write(repo.root().join(".project-id"), "jolly-mute-magma\n").unwrap();
    let mut registry = ProjectRegistry::new(world.env_in(&linked.join(".")), names(&[]));
    let project = registry.for_read(None).unwrap().unwrap();
    assert_eq!(project.id, "jolly-mute-magma");
    assert_eq!(
        project.root,
        text(&repo.root()),
        "the project's root is the primary"
    );
    assert_eq!(
        registry.work_root().unwrap(),
        linked,
        "the work root is the worktree"
    );
}

#[test]
fn test_a_linked_worktree_finds_its_primary_checkouts_registry_entry() {
    let world = World::new();
    let repo = git::Repo::new();
    let linked = repo.worktree("linked");
    let mut index = index::Index::default();
    index.register("high-tight-gulf", &text(&repo.root()));
    index.register("own-old-id", &text(&linked));
    index::save(&index::path(world.home.path()), &index).unwrap();
    let project = ProjectRegistry::new(world.env_in(&linked), names(&[]))
        .for_read(None)
        .unwrap()
        .unwrap();
    assert_eq!(
        project.id, "high-tight-gulf",
        "the primary's, not its own entry's"
    );
}

#[test]
fn test_a_write_in_a_new_worktree_binds_the_primary_checkout_and_marks_only_it() {
    let world = World::new();
    let repo = git::Repo::new();
    let linked = repo.worktree("linked");
    let project = ProjectRegistry::new(world.env_in(&linked), names(&["fresh-new-id"]))
        .for_write(None)
        .unwrap();
    assert_eq!(project.id, "fresh-new-id");
    assert!(
        !linked.join(".project-id").exists(),
        "no marker in the worktree, to be committed and merged back"
    );
    assert_eq!(
        fs::read_to_string(repo.root().join(".project-id")).unwrap(),
        "fresh-new-id\n"
    );
    assert_eq!(world.index().id_for(&repo.root()), Some("fresh-new-id"));
    assert_eq!(world.index().id_for(&linked), None);
    let primary = ProjectRegistry::new(world.env_in(&repo.root()), names(&[]))
        .for_read(None)
        .unwrap()
        .unwrap();
    assert_eq!(primary.id, "fresh-new-id", "one project for both");
}

#[test]
fn test_a_linked_worktree_with_its_own_marker_keeps_its_own_project() {
    let world = World::new();
    let repo = git::Repo::new();
    let linked = repo.worktree("linked");
    fs::write(repo.root().join(".project-id"), "jolly-mute-magma\n").unwrap();
    fs::write(linked.join(".project-id"), "its-own-id\n").unwrap();
    let project = ProjectRegistry::new(world.env_in(&linked), names(&[]))
        .for_read(None)
        .unwrap()
        .unwrap();
    assert_eq!(project.id, "its-own-id");
    assert_eq!(project.root, text(&linked));
}

#[test]
fn test_a_linked_worktree_with_gos_older_marker_keeps_its_own_project() {
    let world = World::new();
    let repo = git::Repo::new();
    let linked = repo.worktree("linked");
    fs::write(repo.root().join(".project-id"), "jolly-mute-magma\n").unwrap();
    fs::create_dir(linked.join(".ctxloom")).unwrap();
    fs::write(linked.join(".ctxloom/project-id"), "go-minted-id\n").unwrap();
    let project = ProjectRegistry::new(world.env_in(&linked), names(&[]))
        .for_write(None)
        .unwrap();
    assert_eq!(project.id, "go-minted-id");
    assert_eq!(
        fs::read_to_string(linked.join(".project-id")).unwrap(),
        "go-minted-id\n",
        "adopted where it was"
    );
}

// ------------------------------------------------------------ every project

#[test]
fn test_every_project_is_each_database_directory_by_id_with_its_registered_root() {
    let world = World::new();
    let projects = world.home.path().join(".ctxloom/projects");
    for id in ["zesty-lone-fern", "brisk-tall-oak"] {
        fs::create_dir_all(projects.join(id)).unwrap();
    }
    fs::write(projects.join("not-a-dir"), "").unwrap();
    fs::create_dir_all(projects.join("Not An Id")).unwrap();
    let mut index = index::Index::default();
    let link = world.home.path().join("link-to-tree");
    std::os::unix::fs::symlink(world.tree(), &link).unwrap();
    index.register("zesty-lone-fern", &text(&link));
    index.register("zesty-lone-fern", "/elsewhere");
    index::save(&index::path(world.home.path()), &index).unwrap();

    let every = world.registry(&[]).every_project().unwrap();
    let seen: Vec<(&str, &str)> = every
        .iter()
        .map(|p| (p.id.as_str(), p.root.as_str()))
        .collect();
    let tree = text(&world.tree());
    assert_eq!(
        seen,
        [("brisk-tall-oak", ""), ("zesty-lone-fern", tree.as_str())],
        "by id; the first registered path, canonical; none unregistered"
    );
    assert_eq!(every[1].store, world.store("zesty-lone-fern"));
}

#[test]
fn test_every_project_is_none_before_any_and_creates_nothing() {
    let world = World::new();
    assert_eq!(world.registry(&[]).every_project().unwrap(), []);
    assert!(!world.home.path().join(".ctxloom").exists());
}

#[test]
fn test_every_project_without_a_home_fails() {
    let world = World::new();
    let mut env = world.env();
    env.home = None;
    let error = ProjectRegistry::new(env, names(&[]))
        .every_project()
        .unwrap_err();
    assert_eq!(error.code(), "project-registry-no-home");
}

// ------------------------------------------------------------ rootless

#[test]
fn test_a_directory_in_no_git_repository_and_no_ctxloom_root_is_rootless() {
    let world = World::new();
    assert_eq!(
        world.registry(&[]).rootless_directory().unwrap(),
        Some(text(&world.tree()))
    );
}

#[test]
fn test_a_directory_in_a_git_repository_or_under_ctxloom_root_is_not_rootless() {
    let world = World::new();
    let mut env = world.env();
    env.root = Some(world.tree());
    let mut registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(registry.rootless_directory().unwrap(), None);
    fs::create_dir(world.tree().join(".git")).unwrap();
    let deep = world.tree().join("a");
    fs::create_dir_all(&deep).unwrap();
    let mut env = world.env();
    env.cwd = Ok(deep);
    let mut registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(registry.rootless_directory().unwrap(), None);
}

#[test]
fn test_rootlessness_with_no_working_directory_fails() {
    let world = World::new();
    let mut env = world.env();
    env.cwd = Err("gone".to_string());
    let error = ProjectRegistry::new(env, names(&[]))
        .rootless_directory()
        .unwrap_err();
    assert_eq!(error.code(), "project-registry-no-cwd");
}
