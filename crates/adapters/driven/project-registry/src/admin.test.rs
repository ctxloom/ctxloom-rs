//! The survey and its changes over a real filesystem: a temporary home and
//! work tree, and repositories git makes.

use std::fs;

use annals_diagnostic::Diagnostic;
use ports::{Change, IdSource, ProjectAdmin, Registered, Survey};

use crate::index;
use crate::tests::{git, names, text, World};
use crate::ProjectRegistry;

impl World {
    fn register(&self, id: &str, path: &str) {
        let file = index::path(self.home.path());
        let mut registry = index::load(&file).unwrap();
        registry.register(id, path);
        index::save(&file, &registry).unwrap();
    }

    fn database(&self, id: &str) {
        let dir = self.home.path().join(".ctxloom/projects").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("state.db"), "tasks").unwrap();
    }

    fn projects(&self) -> std::path::PathBuf {
        self.home.path().join(".ctxloom/projects")
    }

    fn survey(&self) -> Survey {
        self.registry(&[]).survey(None).unwrap()
    }

    fn apply(&self, changes: &[Change]) -> Result<(), annals_diagnostic::Report> {
        let mut registry = self.registry(&[]);
        let seen = registry.survey(None).unwrap();
        registry.apply(None, &seen, changes)
    }
}

fn s(text: &str) -> String {
    text.to_string()
}

// ------------------------------------------------------------ survey

#[test]
fn test_a_survey_reads_the_markers_the_registry_and_the_databases() {
    let world = World::new();
    let tree = text(&world.tree());
    fs::write(world.tree().join(".project-id"), "mark-id\n").unwrap();
    fs::create_dir(world.tree().join(".ctxloom")).unwrap();
    fs::write(world.tree().join(".ctxloom/project-id"), "go-id\n").unwrap();
    world.register("mark-id", &format!("{tree}/./"));
    world.register("gone-id", "/no/such/tree");
    world.database("mark-id");
    world.database("b-orphan");
    fs::write(world.projects().join("not-a-dir"), "").unwrap();
    fs::create_dir(world.projects().join(".hidden")).unwrap();
    std::os::unix::fs::symlink(world.tree(), world.projects().join("linked-id")).unwrap();
    let survey = world.survey();
    assert_eq!(
        survey,
        Survey {
            work_root: tree.clone(),
            root: tree.clone(),
            resolved: Some((s("mark-id"), IdSource::Marker)),
            marker: Some(s("mark-id")),
            legacy_marker: Some(s("go-id")),
            registry: vec![
                Registered {
                    id: s("mark-id"),
                    path: format!("{tree}/./"),
                    canonical: tree.clone(),
                    exists: true,
                },
                Registered {
                    id: s("gone-id"),
                    path: s("/no/such/tree"),
                    canonical: s("/no/such/tree"),
                    exists: false,
                },
            ],
            databases: vec![s("b-orphan"), s("mark-id")],
            projects: world.projects(),
        }
    );
    assert_eq!(
        world.marker().as_deref(),
        Some("mark-id\n"),
        "nothing written"
    );
}

#[test]
fn test_a_survey_names_where_the_id_came_from() {
    let world = World::new();
    assert_eq!(world.survey().resolved, None);
    assert_eq!(world.survey().databases, Vec::<String>::new());
    world.register("reg-id", &text(&world.tree()));
    assert_eq!(
        world.survey().resolved,
        Some((s("reg-id"), IdSource::Registry))
    );
    fs::create_dir(world.tree().join(".ctxloom")).unwrap();
    fs::write(world.tree().join(".ctxloom/project-id"), "go-id\n").unwrap();
    assert_eq!(
        world.survey().resolved,
        Some((s("go-id"), IdSource::LegacyMarker))
    );
    assert!(world.marker().is_none(), "a survey adopts nothing");
    let mut env = world.env();
    env.project_id = Some(s("env-id"));
    let mut registry = ProjectRegistry::new(env, names(&[]));
    assert_eq!(
        registry.survey(None).unwrap().resolved,
        Some((s("env-id"), IdSource::Environment))
    );
    assert_eq!(
        registry.survey(Some("flag-id")).unwrap().resolved,
        Some((s("flag-id"), IdSource::Flag))
    );
}

#[test]
fn test_a_survey_in_a_linked_worktree_is_rooted_at_its_primary_checkout() {
    let world = World::new();
    let repo = git::Repo::new();
    let linked = repo.worktree("linked");
    fs::write(repo.root().join(".project-id"), "primary-id\n").unwrap();
    let survey = ProjectRegistry::new(world.env_in(&linked), names(&[]))
        .survey(None)
        .unwrap();
    assert_eq!(survey.work_root, text(&linked));
    assert_eq!(survey.root, text(&repo.root()));
    assert_eq!(survey.marker, Some(s("primary-id")));
}

#[test]
fn test_a_survey_fails_on_a_projects_directory_it_cannot_list() {
    use std::os::unix::fs::PermissionsExt;

    let world = World::new();
    fs::create_dir_all(world.projects()).unwrap();
    // Its files can be opened by name (there is no registry yet), but it
    // cannot be listed.
    fs::set_permissions(world.projects(), fs::Permissions::from_mode(0o300)).unwrap();
    let error = world.registry(&[]).survey(None).unwrap_err();
    fs::set_permissions(world.projects(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(error.code(), "project-registry-io");
    assert!(error.to_string().contains("projects"), "{error}");
}

// ------------------------------------------------------------ apply

#[test]
fn test_changes_register_unregister_and_mark() {
    let world = World::new();
    let tree = text(&world.tree());
    world.register("old-id", &format!("{tree}/"));
    world.register("keep-id", "/elsewhere");
    world
        .apply(&[
            Change::Register {
                id: s("new-id"),
                path: tree.clone(),
            },
            Change::Unregister {
                id: s("old-id"),
                path: tree.clone(),
            },
            Change::WriteMarker {
                root: tree.clone(),
                id: s("new-id"),
            },
        ])
        .unwrap();
    let registry = world.index();
    let held: Vec<(&str, &str)> = registry
        .projects
        .iter()
        .map(|e| (e.project_id.as_str(), e.path.as_str()))
        .collect();
    assert_eq!(held, [("keep-id", "/elsewhere"), ("new-id", tree.as_str())]);
    assert_eq!(world.marker().as_deref(), Some("new-id\n"));
}

#[test]
fn test_unregistering_matches_the_id_and_the_path_both() {
    let world = World::new();
    let tree = text(&world.tree());
    world.register("a", &tree);
    world.register("b", &tree);
    world.register("a", "/other");
    world
        .apply(&[Change::Unregister {
            id: s("a"),
            path: tree.clone(),
        }])
        .unwrap();
    let held: Vec<(String, String)> = world
        .index()
        .projects
        .into_iter()
        .map(|e| (e.project_id, e.path))
        .collect();
    assert_eq!(held, [(s("b"), tree), (s("a"), s("/other"))]);
}

#[test]
fn test_a_change_without_registry_changes_leaves_the_registry_unwritten() {
    let world = World::new();
    world.database("stray-id");
    world
        .apply(&[Change::RemoveDatabase { id: s("stray-id") }])
        .unwrap();
    assert!(!index::path(world.home.path()).exists());
}

#[test]
fn test_a_stale_survey_changes_nothing() {
    let world = World::new();
    let mut registry = world.registry(&[]);
    let seen = registry.survey(None).unwrap();
    fs::write(world.tree().join(".project-id"), "raced-in\n").unwrap();
    let refused = registry.apply(
        None,
        &seen,
        &[Change::Register {
            id: s("planned"),
            path: text(&world.tree()),
        }],
    );
    assert_eq!(refused.unwrap_err().code(), "project-registry-changed");
    assert!(world.index().projects.is_empty());
}

#[test]
fn test_a_change_naming_an_invalid_id_is_refused() {
    let world = World::new();
    for change in [
        Change::Register {
            id: s("../x"),
            path: text(&world.tree()),
        },
        Change::WriteMarker {
            root: text(&world.tree()),
            id: s("../x"),
        },
        Change::RemoveDatabase { id: s("..") },
    ] {
        let refused = world.apply(std::slice::from_ref(&change));
        assert_eq!(
            refused.unwrap_err().code(),
            "project-registry-invalid-id",
            "{change:?}"
        );
    }
    assert!(world.marker().is_none());
    assert!(world.home.path().exists());
}

#[test]
fn test_removing_a_database_deletes_only_its_directory() {
    let world = World::new();
    world.database("stray-id");
    world.database("kept-id");
    world.register("kept-id", "/elsewhere");
    world
        .apply(&[Change::RemoveDatabase { id: s("stray-id") }])
        .unwrap();
    assert!(!world.projects().join("stray-id").exists());
    assert!(world.projects().join("kept-id/state.db").exists());
    assert_eq!(world.index().projects.len(), 1, "the registry kept");
}

#[test]
fn test_removing_what_is_not_a_database_directory_is_refused() {
    let world = World::new();
    fs::create_dir_all(world.projects()).unwrap();
    let target = world.tree().join("precious");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep"), "").unwrap();
    std::os::unix::fs::symlink(&target, world.projects().join("link-id")).unwrap();
    fs::write(world.projects().join("file-id"), "").unwrap();
    for id in ["link-id", "file-id"] {
        let mut registry = world.registry(&[]);
        let seen = registry.survey(None).unwrap();
        let refused = registry.apply(None, &seen, &[Change::RemoveDatabase { id: s(id) }]);
        assert_eq!(
            refused.unwrap_err().code(),
            "project-registry-not-a-database",
            "{id}"
        );
    }
    assert!(target.join("keep").exists());
    let missing = world.apply(&[Change::RemoveDatabase { id: s("no-such") }]);
    assert_eq!(missing.unwrap_err().code(), "project-registry-io");
}

#[test]
fn test_a_change_is_made_under_the_registry_lock() {
    let world = World::new();
    let file = index::path(world.home.path());
    let held = index::lock_within(&file, index::LOCK_WAIT).unwrap();
    let mut registry = world
        .registry(&[])
        .with_lock_wait(std::time::Duration::ZERO);
    let seen = registry.survey(None).unwrap();
    let change = [Change::Register {
        id: s("a"),
        path: s("/p"),
    }];
    let refused = registry.apply(None, &seen, &change);
    assert_eq!(refused.unwrap_err().code(), "project-registry-lock-timeout");
    assert!(world.index().projects.is_empty());
    drop(held);
    // A change waits as it does in use: a child forked by another thread
    // here (git) can hold the released lock until it execs.
    let mut registry = registry.with_lock_wait(index::LOCK_WAIT);
    registry.apply(None, &seen, &change).unwrap();
    assert_eq!(world.index().projects.len(), 1);
}
