//! Go's acceptance scenarios for the companion noun's write side (S2),
//! `tests/acceptance/features/cli/companion.feature` at release/0.7
//! ca14d6046, ported: each test names its scenario. A companion is a program
//! on the `PATH` answering `<bin> loadout --format yaml`; the fakes here are
//! shell scripts in a directory put first on the `PATH`.
//!
//! Not here: what `ctxloom doctor` runs (S12): the witness-file scenarios'
//! registration and removal halves are, and the executions are read off the
//! fake's own witness file where the command itself runs it.

use std::path::PathBuf;

use super::library::{reports_as, Project};

const HOME_CONFIG: &str = ".ctxloom/config.yaml";

/// A directory of fake companions, and the `PATH` that finds them first.
struct Bin {
    dir: tempfile::TempDir,
}

impl Bin {
    fn new() -> Bin {
        Bin {
            dir: tempfile::tempdir().expect("a directory for the fakes"),
        }
    }

    /// `ctxloom-companion-<name>`, answering the loadout probe and counting
    /// its runs in `ran.log`.
    fn companion(&self, name: &str) -> PathBuf {
        self.script(
            &format!("ctxloom-companion-{name}"),
            &format!(
                "#!/bin/sh\necho \"$@\" >> \"{}/ran.log\"\ncase \"$1\" in\n  loadout) printf 'run:\\n  version: 1.0.0\\n';;\n  version) printf '{{\"name\":\"{name}\",\"version\":\"1\"}}\\n';;\nesac\n",
                self.dir.path().display()
            ),
        )
    }

    /// A binary that is no companion: it prints nothing and fails.
    fn broken(&self, name: &str) -> PathBuf {
        self.script(&format!("ctxloom-companion-{name}"), "#!/bin/sh\nexit 3\n")
    }

    /// A binary that answers with nothing.
    fn silent(&self, name: &str) -> PathBuf {
        self.script(&format!("ctxloom-companion-{name}"), "#!/bin/sh\nexit 0\n")
    }

    fn script(&self, file: &str, body: &str) -> PathBuf {
        let path = self.dir.path().join(file);
        std::fs::write(&path, body).expect("a fake companion");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("made executable");
        }
        path
    }

    fn path(&self) -> String {
        let mut paths = vec![self.dir.path().to_path_buf()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        std::env::join_paths(paths)
            .expect("a PATH")
            .to_string_lossy()
            .into_owned()
    }

    fn ran(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("ran.log")).unwrap_or_default()
    }
}

fn run(project: &Project, bin: &Bin, args: &[&str]) -> super::Ran {
    project.run_with(&[("PATH", &bin.path())], args)
}

fn home_companions(project: &Project) -> String {
    project.read_home(HOME_CONFIG).unwrap_or_default()
}

// companion.feature: Scenario: A registered companion runs, and removing it
// stops it (the registration and the removal: the home config holds the name
// and nothing else; doctor's execution is S12's)
#[test]
fn test_a_registered_companion_is_recorded_by_name_only_and_removing_it_unrecords_it() {
    let project = Project::initialized();
    let bin = Bin::new();
    let path = bin.companion("acme");
    let added = run(&project, &bin, &["companion", "add", "acme"]);
    added.succeeds();
    // Registration is the name, never a path.
    let home = home_companions(&project);
    assert!(home.contains("companions:\n  - acme\n"), "{home}");
    assert!(!home.contains(path.to_str().expect("utf-8")), "{home}");
    assert!(
        home.contains("editor:"),
        "the rest of the home config was kept: {home}"
    );
    // The project's config was not touched.
    assert_eq!(
        project.read(".ctxloom/config.yaml").as_deref(),
        Some("schema_version: 8\n")
    );
    run(&project, &bin, &["companion", "remove", "acme", "--yes"]).succeeds();
    assert!(!home_companions(&project).contains("acme"));
    assert_eq!(
        project.read_home(HOME_CONFIG).as_deref(),
        Some("schema_version: 8\neditor:\n  command: \"true\"\n"),
        "the empty list is not left behind"
    );
}

// companion.feature: Scenario: A project's companions are added to the home
// registrations
#[test]
fn test_a_projects_companions_are_added_to_the_home_registrations_by_add() {
    let project = Project::initialized();
    let bin = Bin::new();
    bin.companion("acme");
    run(&project, &bin, &["companion", "add", "acme"]).succeeds();
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 8\ncompanions: [beta]\n",
    );
    let ran = run(&project, &bin, &["companion", "list", "--format", "json"]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "0.name"), "acme");
    assert_eq!(reports_as(&doc, "1.name"), "beta");
}

// companion.feature: Scenario: An empty project list does not unregister a
// home companion
#[test]
fn test_an_empty_project_list_does_not_unregister_a_home_companion_after_add() {
    let project = Project::initialized();
    let bin = Bin::new();
    bin.companion("acme");
    run(&project, &bin, &["companion", "add", "acme"]).succeeds();
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 8\ncompanions: []\n",
    );
    let ran = run(&project, &bin, &["companion", "list", "--format", "json"]);
    ran.succeeds();
    assert_eq!(reports_as(&ran.json(), "0.name"), "acme");
}

// companion.feature: Scenario: A name with nothing on PATH is refused, naming
// the binary it looked for
#[test]
fn test_a_name_with_nothing_on_path_is_refused_naming_the_binary_it_looked_for() {
    let project = Project::initialized();
    let bin = Bin::new();
    let before = project.read_home(HOME_CONFIG);
    run(&project, &bin, &["companion", "add", "nowhere"])
        .fails()
        .contains("ctxloom-companion-nowhere")
        .contains("companion not found on PATH");
    assert_eq!(project.read_home(HOME_CONFIG), before);
    run(&project, &bin, &["companion", "list", "--format", "text"])
        .succeeds()
        .contains("No companions registered");
}

// operations.AddCompanion (documented conditions): a name that is no name, a
// binary that does not answer the probe, one that answers nothing: each is
// refused and records nothing. The first-party names are their own binaries.
#[test]
fn test_add_checks_the_companion_before_it_records_anything() {
    let project = Project::initialized();
    let bin = Bin::new();
    bin.broken("sick");
    bin.silent("mute");
    let before = project.read_home(HOME_CONFIG);
    run(&project, &bin, &["companion", "add", "sick"])
        .fails()
        .contains("binary does not answer the companion loadout probe");
    run(&project, &bin, &["companion", "add", "mute"])
        .fails()
        .contains("printed no loadout");
    for bad in ["-x", "a b", "../x", ""] {
        run(&project, &bin, &["companion", "add", bad])
            .fails()
            .contains("invalid companion name");
    }
    assert_eq!(project.read_home(HOME_CONFIG), before);
    // ltk, taskloom and reprise are their own binaries, not ctxloom-companion-*.
    bin.script("ltk", "#!/bin/sh\nprintf 'run:\\n  version: 1.0.0\\n'\n");
    let ran = run(
        &project,
        &bin,
        &["companion", "add", "ltk", "--format", "json"],
    );
    ran.succeeds();
    assert_eq!(reports_as(&ran.json(), "bin"), "ltk");
}

// operations.AddCompanion (documented condition): the answer, in each format,
// and a second add is not a second registration.
#[test]
fn test_add_answers_and_a_second_add_registers_nothing_twice() {
    let project = Project::initialized();
    let bin = Bin::new();
    let path = bin.companion("acme");
    let first = run(
        &project,
        &bin,
        &["companion", "add", "acme", "--format", "json"],
    );
    first.succeeds();
    let doc = first.json();
    assert_eq!(reports_as(&doc, "name"), "acme");
    assert_eq!(reports_as(&doc, "bin"), "ctxloom-companion-acme");
    assert_eq!(reports_as(&doc, "path"), path.to_string_lossy());
    assert_eq!(reports_as(&doc, "added"), "true");
    let again = run(
        &project,
        &bin,
        &["companion", "add", "acme", "--format", "text"],
    );
    assert_eq!(
        again.stdout,
        format!(
            "Companion 'acme' is already registered ({})\n",
            path.display()
        )
    );
    assert_eq!(home_companions(&project).matches("acme").count(), 1);
    let project2 = Project::initialized();
    let text = run(
        &project2,
        &bin,
        &["companion", "add", "acme", "--format", "text"],
    );
    assert_eq!(
        text.stdout,
        format!(
            "Registered companion 'acme' (ctxloom-companion-acme answers at {})\n",
            path.display()
        )
    );
}

// companion.feature: Scenario: Remove without --yes names the apply command
// and changes nothing
#[test]
fn test_remove_without_yes_names_the_apply_command_and_changes_nothing() {
    let project = Project::initialized();
    let bin = Bin::new();
    bin.companion("acme");
    run(&project, &bin, &["companion", "add", "acme"]).succeeds();
    let before = project.read_home(HOME_CONFIG);
    let ran = run(
        &project,
        &bin,
        &["companion", "remove", "acme", "--format", "text"],
    );
    ran.succeeds()
        .contains("ctxloom companion remove acme --yes");
    assert_eq!(
        ran.stdout,
        "Would remove companion \"acme\".\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom companion remove acme --yes\n"
    );
    assert_eq!(project.read_home(HOME_CONFIG), before);
    let json = run(&project, &bin, &["companion", "remove", "acme"]);
    assert_eq!(reports_as(&json.json(), "applied"), "false");
    assert_eq!(project.read_home(HOME_CONFIG), before);
    let applied = run(
        &project,
        &bin,
        &["companion", "remove", "acme", "--yes", "--format", "json"],
    );
    assert_eq!(reports_as(&applied.json(), "removed"), "true");
    assert_eq!(reports_as(&applied.json(), "name"), "acme");
    let text = {
        run(&project, &bin, &["companion", "add", "acme"]).succeeds();
        run(
            &project,
            &bin,
            &["companion", "remove", "acme", "--yes", "--format", "text"],
        )
    };
    assert_eq!(text.stdout, "Removed companion 'acme'\n");
    for yes in [vec![], vec!["--yes"]] {
        let mut args = vec!["companion", "remove", "acme"];
        args.extend(yes);
        run(&project, &bin, &args)
            .fails()
            .contains("companion not registered: \"acme\"");
    }
}

// companion.feature: Scenario: List answers which binary each registered name
// resolves to (after add)
#[test]
fn test_list_answers_which_binary_each_registered_name_resolves_to_after_add() {
    let project = Project::initialized();
    let bin = Bin::new();
    let path = bin.companion("acme");
    run(&project, &bin, &["companion", "add", "acme"]).succeeds();
    let ran = run(&project, &bin, &["companion", "list", "--format", "json"]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "0.name"), "acme");
    assert_eq!(reports_as(&doc, "0.bin"), "ctxloom-companion-acme");
    assert_eq!(reports_as(&doc, "0.resolves"), "true");
    assert_eq!(reports_as(&doc, "0.path"), path.to_string_lossy());
}

// companion.feature: Scenario: Bare companion lists the registered companions
// (after add)
#[test]
fn test_bare_companion_lists_the_registered_companions_after_add() {
    let project = Project::initialized();
    let bin = Bin::new();
    bin.companion("acme");
    run(&project, &bin, &["companion", "add", "acme"]).succeeds();
    run(&project, &bin, &["companion"])
        .succeeds()
        .contains("acme")
        .contains("ctxloom-companion-acme")
        .lacks("Available Commands:");
}

// companion.feature: Scenario: A companion an npm dependency drops on PATH is
// never run (adding or removing a name runs only that name's binary)
#[test]
fn test_only_the_named_companion_is_ever_run_by_add() {
    let project = Project::initialized();
    let bin = Bin::new();
    bin.companion("acme");
    bin.companion("evil");
    run(&project, &bin, &["companion", "add", "acme"]).succeeds();
    let ran = bin.ran();
    assert!(ran.contains("loadout"), "{ran}");
    run(&project, &bin, &["companion", "list"]).succeeds();
    run(&project, &bin, &["companion", "remove", "acme", "--yes"]).succeeds();
    let log = bin.ran();
    assert_eq!(log.lines().count(), 1, "something else ran: {log}");
}
