//! Go's acceptance scenarios for the companion noun's write side (S2),
//! `tests/acceptance/features/cli/companion.feature` at release/0.7
//! ca14d6046, ported: each test names its scenario. A companion is registered
//! as a name and the command that runs it (schema 9; nothing is searched
//! for), and answers `<command> loadout --format yaml`; the fakes here are
//! shell scripts in a directory of their own.
//!
//! Not here: what `ctxloom doctor` runs (S12): the witness-file scenarios'
//! registration and removal halves are, and the executions are read off the
//! fake's own witness file where the command itself runs it.

use std::path::PathBuf;

use super::library::{reports_as, Project};
use crate::tests::files;

const HOME_CONFIG: &str = ".ctxloom/config.yaml";

/// A directory of fake companions, and the `PATH` that finds them first.
struct Bin {
    dir: file_system::Rooted,
}

impl Bin {
    fn new() -> Bin {
        Bin { dir: files::root() }
    }

    /// `ctxloom-companion-<name>` (a file name only: the registration holds
    /// its path), answering the loadout probe and counting
    /// its runs in `ran.log`.
    fn companion(&self, name: &str) -> PathBuf {
        self.script(
            &format!("ctxloom-companion-{name}"),
            &format!(
                "#!/bin/sh\necho \"$@\" >> \"{}/ran.log\"\ncase \"$1\" in\n  loadout) printf 'run:\\n  version: 1.0.0\\n';;\n  version) printf '{{\"name\":\"{name}\",\"version\":\"1\"}}\\n';;\nesac\n",
                self.dir.root().display()
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
        let path = self.dir.root().join(file);
        files::put_program(&path, body);
        path
    }

    fn path(&self) -> String {
        let mut paths = vec![self.dir.root().to_path_buf()];
        paths.extend(files::search_path(
            &crate::tests::host_var("PATH").unwrap_or_default(),
        ));
        std::env::join_paths(paths)
            .expect("a PATH")
            .to_string_lossy()
            .into_owned()
    }

    fn ran(&self) -> String {
        files::read(self.dir.root().join("ran.log")).unwrap_or_default()
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
// and the command, which is the path it was given; doctor's execution is S12's)
#[test]
fn test_a_registered_companion_is_recorded_with_its_command_and_removing_it_unrecords_it() {
    let project = Project::initialized();
    let bin = Bin::new();
    let path = bin.script(
        "acme-bin",
        "#!/bin/sh\nprintf 'run:\\n  version: 1.0.0\\n'\n",
    );
    let program = path.to_str().expect("utf-8");
    let added = run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "acme",
            "--command",
            program,
            "--as",
            "acme",
        ],
    );
    added.succeeds();
    let home = home_companions(&project);
    assert!(
        home.contains(&format!(
            "companions:\n  acme:\n    command:\n      - {program}\n      - --as\n      - acme\n"
        )),
        "{home}"
    );
    assert!(
        home.contains("editor:"),
        "the rest of the home config was kept: {home}"
    );
    assert_eq!(
        project.read(".ctxloom/config.yaml").as_deref(),
        Some("schema_version: 9\n"),
        "the project's config was not touched"
    );
    run(&project, &bin, &["companion", "remove", "acme", "--yes"]).succeeds();
    assert!(!home_companions(&project).contains("acme"));
    assert_eq!(
        project.read_home(HOME_CONFIG).as_deref(),
        Some("schema_version: 9\neditor:\n  command: \"true\"\n"),
        "the empty map is not left behind"
    );
}

// companion.feature: Scenario: A project's companions are added to the home
// registrations
#[test]
fn test_a_projects_companions_are_added_to_the_home_registrations_by_add() {
    let project = Project::initialized();
    let bin = Bin::new();
    let path = bin.companion("acme");
    run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "acme",
            "--command",
            path.to_str().unwrap(),
        ],
    )
    .succeeds();
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 9\ncompanions:\n  beta: {command: [beta-bin]}\n",
    );
    let ran = run(&project, &bin, &["companion", "list", "--format", "json"]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "0.name"), "acme");
    assert_eq!(reports_as(&doc, "1.name"), "beta");
}

// companion.feature: Scenario: An empty project map does not unregister a
// home companion
#[test]
fn test_an_empty_project_map_does_not_unregister_a_home_companion_after_add() {
    let project = Project::initialized();
    let bin = Bin::new();
    let path = bin.companion("acme");
    run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "acme",
            "--command",
            path.to_str().unwrap(),
        ],
    )
    .succeeds();
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 9\ncompanions: {}\n",
    );
    let ran = run(&project, &bin, &["companion", "list", "--format", "json"]);
    ran.succeeds();
    assert_eq!(reports_as(&ran.json(), "0.name"), "acme");
}

// companion.feature: Scenario: A command whose program does not exist is
// refused, naming it, and nothing is recorded
#[test]
fn test_a_command_whose_program_does_not_exist_is_refused_naming_it() {
    let project = Project::initialized();
    let bin = Bin::new();
    let before = project.read_home(HOME_CONFIG);
    run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "nowhere",
            "--command",
            "/no/such/program",
        ],
    )
    .fails()
    .contains("/no/such/program")
    .contains("companion program not found");
    assert_eq!(project.read_home(HOME_CONFIG), before);
    run(&project, &bin, &["companion", "list", "--format", "text"])
        .succeeds()
        .contains("No companions registered");
}

// operations.AddCompanion (documented conditions): a name that is no name, a
// command that does not answer the probe, one that answers nothing: each is
// refused and records nothing. No name means a binary: a first-party name is
// registered with the command it is given like any other.
#[test]
fn test_add_checks_the_companion_before_it_records_anything() {
    let project = Project::initialized();
    let bin = Bin::new();
    let sick = bin.broken("sick");
    let mute = bin.silent("mute");
    let before = project.read_home(HOME_CONFIG);
    run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "sick",
            "--command",
            sick.to_str().unwrap(),
        ],
    )
    .fails()
    .contains("command does not answer the companion loadout probe");
    run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "mute",
            "--command",
            mute.to_str().unwrap(),
        ],
    )
    .fails()
    .contains("printed no loadout");
    for bad in ["-x", "a b", "../x", ""] {
        run(
            &project,
            &bin,
            &["companion", "add", bad, "--command", "true"],
        )
        .fails()
        .contains("invalid companion name");
    }
    run(&project, &bin, &["companion", "add", "x", "--command", ""])
        .fails()
        .contains("has no command");
    assert_eq!(project.read_home(HOME_CONFIG), before);
    let ltk = bin.script("ltk", "#!/bin/sh\nprintf 'run:\\n  version: 1.0.0\\n'\n");
    let ran = run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "ltk",
            "--format",
            "json",
            "--command",
            ltk.to_str().unwrap(),
        ],
    );
    ran.succeeds();
    assert_eq!(reports_as(&ran.json(), "name"), "ltk");
}

// operations.AddCompanion (documented condition): the answer, in each format,
// a second add of the same registration is not a second registration, and the
// probe is run once.
#[test]
fn test_add_answers_and_a_second_add_registers_nothing_twice() {
    let project = Project::initialized();
    let bin = Bin::new();
    let path = bin.companion("acme");
    let program = path.to_string_lossy().to_string();
    let first = run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "acme",
            "--format",
            "json",
            "--command",
            &program,
        ],
    );
    first.succeeds();
    let doc = first.json();
    assert_eq!(reports_as(&doc, "name"), "acme");
    assert_eq!(reports_as(&doc, "command.0"), program);
    assert_eq!(reports_as(&doc, "added"), "true");
    assert_eq!(bin.ran().lines().count(), 1, "one probe: {}", bin.ran());
    let again = run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "acme",
            "--format",
            "text",
            "--command",
            &program,
        ],
    );
    assert_eq!(
        again.stdout,
        format!("Companion 'acme' is already registered ({program})\n")
    );
    assert_eq!(home_companions(&project).matches("acme:").count(), 1);
    let project2 = Project::initialized();
    let text = run(
        &project2,
        &bin,
        &[
            "companion",
            "add",
            "acme",
            "--format",
            "text",
            "--command",
            &program,
        ],
    );
    assert_eq!(
        text.stdout,
        format!("Registered companion 'acme' (answers: {program})\n")
    );
}

// companion.feature: Scenario: Remove without --yes names the apply command
// and changes nothing
#[test]
fn test_remove_without_yes_names_the_apply_command_and_changes_nothing() {
    let project = Project::initialized();
    let bin = Bin::new();
    let path = bin.companion("acme");
    let program = path.to_string_lossy().to_string();
    let add = |project: &Project| {
        let ran = run(
            project,
            &bin,
            &["companion", "add", "acme", "--command", &program],
        );
        ran.succeeds();
    };
    add(&project);
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
        add(&project);
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

// companion.feature: Scenario: List answers each registered name's command and
// whether its program exists (after add); the program is looked for, never run.
#[test]
fn test_list_answers_each_names_command_and_whether_its_program_exists_after_add() {
    let project = Project::initialized();
    let bin = Bin::new();
    let path = bin.companion("acme");
    let program = path.to_string_lossy().to_string();
    run(
        &project,
        &bin,
        &["companion", "add", "acme", "--command", &program],
    )
    .succeeds();
    let probes = bin.ran().lines().count();
    let ran = run(&project, &bin, &["companion", "list", "--format", "json"]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "0.name"), "acme");
    assert_eq!(reports_as(&doc, "0.command.0"), program);
    assert_eq!(reports_as(&doc, "0.exists"), "true");
    assert_eq!(bin.ran().lines().count(), probes, "listing ran nothing");
}

// companion.feature: Scenario: Bare companion lists the registered companions
// (after add)
#[test]
fn test_bare_companion_lists_the_registered_companions_after_add() {
    let project = Project::initialized();
    let bin = Bin::new();
    let path = bin.companion("acme");
    run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "acme",
            "--command",
            path.to_str().unwrap(),
        ],
    )
    .succeeds();
    run(&project, &bin, &["companion", "--format", "text"])
        .succeeds()
        .contains("acme")
        .contains("found")
        .lacks("Available Commands:");
}

// companion.feature: Scenario: A companion an npm dependency drops on PATH is
// never run (adding or removing a name runs only the command given)
#[test]
fn test_only_the_registered_command_is_ever_run_by_add() {
    let project = Project::initialized();
    let bin = Bin::new();
    let acme = bin.companion("acme");
    bin.companion("evil");
    run(
        &project,
        &bin,
        &[
            "companion",
            "add",
            "acme",
            "--command",
            acme.to_str().unwrap(),
        ],
    )
    .succeeds();
    let ran = bin.ran();
    assert!(ran.contains("loadout"), "{ran}");
    run(&project, &bin, &["companion", "list"]).succeeds();
    run(&project, &bin, &["companion", "remove", "acme", "--yes"]).succeeds();
    let log = bin.ran();
    assert_eq!(log.lines().count(), 1, "something else ran: {log}");
}
