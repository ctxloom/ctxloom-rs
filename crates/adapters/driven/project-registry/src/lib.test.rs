//! Resolution over a real filesystem: a temporary home and work tree.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use annals_diagnostic::Diagnostic;
use ports::{Names, ProjectIdentity};
use tempfile::TempDir;

use super::{index, Environment, ProjectRegistry, MINT_ATTEMPTS};

/// Project ids in the order given, then `then` forever.
struct Scripted {
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

fn names(ids: &[&str]) -> Scripted {
    Scripted {
        ids: ids.iter().map(|s| s.to_string()).collect(),
        then: "always-the-same".to_string(),
        minted: 0,
    }
}

/// A home and a work tree, apart.
struct World {
    home: TempDir,
    tree: TempDir,
}

impl World {
    fn new() -> World {
        World {
            home: tempfile::tempdir().unwrap(),
            tree: tempfile::tempdir().unwrap(),
        }
    }

    fn tree(&self) -> &Path {
        self.tree.path()
    }

    fn env(&self) -> Environment {
        Environment {
            home: Some(self.home.path().to_path_buf()),
            cwd: self.tree().to_path_buf(),
            root: None,
            project_id: None,
        }
    }

    fn registry(&self, ids: &[&str]) -> ProjectRegistry<Scripted> {
        ProjectRegistry::new(self.env(), names(ids))
    }

    fn index(&self) -> index::Index {
        index::load(&index::path(self.home.path())).unwrap()
    }

    fn marker(&self) -> Option<String> {
        fs::read_to_string(self.tree().join(".project-id")).ok()
    }

    fn store(&self, id: &str) -> PathBuf {
        self.home
            .path()
            .join(".ctxloom/projects")
            .join(id)
            .join("state.db")
    }
}

fn text(path: &Path) -> String {
    path.display().to_string()
}

// ------------------------------------------------------------ work root

#[test]
fn test_the_work_root_is_ctxloom_root_when_set() {
    let world = World::new();
    let mut env = world.env();
    env.root = Some(PathBuf::from("/somewhere/else"));
    let registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(registry.work_root(), Path::new("/somewhere/else"));
}

#[test]
fn test_the_work_root_is_the_nearest_ancestor_holding_git() {
    let world = World::new();
    fs::create_dir(world.tree().join(".git")).unwrap();
    let deep = world.tree().join("a/b");
    fs::create_dir_all(&deep).unwrap();
    fs::create_dir_all(world.tree().join("a/.git")).unwrap();
    let mut env = world.env();
    env.cwd = deep;
    env.root = Some(PathBuf::new());
    let registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(
        registry.work_root(),
        world.tree().join("a"),
        "an empty CTXLOOM_ROOT is unset"
    );
}

#[test]
fn test_a_linked_worktrees_git_file_marks_its_root() {
    let world = World::new();
    fs::write(world.tree().join(".git"), "gitdir: /elsewhere\n").unwrap();
    let mut env = world.env();
    env.cwd = world.tree().join("sub");
    let registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(registry.work_root(), world.tree());
}

#[test]
fn test_with_no_git_above_the_work_root_is_the_working_directory() {
    let world = World::new();
    assert_eq!(world.registry(&[]).work_root(), world.tree());
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
    assert_eq!(project.root, text(world.tree()));
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
    index.register("high-tight-gulf", &text(world.tree()));
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
    assert_eq!(project.root, text(world.tree()));
    assert_eq!(project.store, world.store("fresh-new-id"));
    assert_eq!(world.marker().as_deref(), Some("fresh-new-id\n"));
    let index = world.index();
    assert_eq!(index.schema_version, 1);
    assert_eq!(index.id_for(&text(world.tree())), Some("fresh-new-id"));
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
    assert_eq!(registry.bind(world.tree()).unwrap(), "won-the-race");
    assert_eq!(registry.names.minted, 0);
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
