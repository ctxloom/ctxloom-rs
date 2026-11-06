//! Go's acceptance scenarios for the config noun's write side (S2),
//! `tests/acceptance/features/cli/config.feature` at release/0.7 ca14d6046,
//! ported: each test names its scenario. The bytes `config create` writes are
//! Go's own (captured from a Go ctxloom run, and Go's `resources/`: the
//! default remotes registry and the seeded default profile).
//!
//! `config create` and `config edit` answer in text only (Go's format debt):
//! an explicit `--format json` is refused before anything is written.
//!
//! A config write is a section merge with sorted keys and never heals a
//! corrupt file: the section a command owns is replaced whole, with its keys
//! sorted; every other key, comment and the order of the file stay as the
//! author left them; a file that does not parse is refused and left as it
//! was.

use super::library::{Project, MINIMAL_CONFIG};
use crate::tests::files;

const CONFIG: &str = ".ctxloom/config.yaml";

/// `resources/default-remotes.yaml` at ca14d6046.
const DEFAULT_REMOTES: &str = "# ctxloom Remote Sources\n# Pre-configured remotes for common ctxloom bundles and profiles.\n#\n# Registering a remote is the trust act: its content reaches the assistant\n# because the project added it.\n\nschema_version: 1\n\nremotes:\n  ctxloom-default:\n    url: https://github.com/ctxloom/ctxloom-default\n";

/// `resources/profiles/default.yaml` at ca14d6046.
const SEED_PROFILE: &str = "description: \"Local default coding profile scaffolded by `ctxloom init`: the project's primary implementer context, inheriting ctxloom-default's developer profile. Edit it freely; `ctxloom init prompt` re-enters the setup interview any time.\"\ntags:\n  - default\n# A LOCAL profile (owned and editable in this project's .ctxloom), not a bare\n# remote reference.\n#\n# The baseline-conduct parent was REMOVED with the `default` bundle it pointed\n# at. Conduct is no longer SEEDED into every project as a fixed parent; it is\n# offered by init's phase-3 survey as content carrying the `default` tag, which\n# the user includes or declines like anything else. A tag is a recommendation\n# the user answers; a seeded parent is a decision made on their behalf before\n# they arrive.\nparents:\n  - ctxloom+git://github.com/ctxloom/ctxloom-default//bundles/ai-developer@branch:main#profiles/developer\n";

const CLAUDE_CONFIG: &str = "agents:\n  default:\n    engine_home: session\n    llm: claude-code\n    profiles:\n      - default\n    runtime: host\nauth: token\nconfig:\n  use_distilled: true\ndefault_agent: default\nllm:\n  configs:\n    claude-code:\n      model: opus\n      type: claude-code\n    claude-fast:\n      model: haiku\n      type: claude-code\n  defaults:\n    fast: claude-fast\n    primary: claude-code\nschema_version: 8\n";

// config.feature: Scenario: Create scaffolds the config, the remotes registry
// and the seeded profile
#[test]
fn test_create_scaffolds_the_config_the_remotes_registry_and_the_seeded_profile() {
    let project = Project::bare();
    let ran = project.run(&["config", "create"]);
    ran.succeeds();
    assert_eq!(
        ran.stdout,
        format!("Wrote {}\n", project.dir().join(CONFIG).display())
    );
    project.file_contains(CONFIG, "llm: claude-code");
    // Go's bytes.
    assert_eq!(project.read(CONFIG).as_deref(), Some(CLAUDE_CONFIG));
    assert_eq!(
        project.read(".ctxloom/remotes.yaml").as_deref(),
        Some(DEFAULT_REMOTES)
    );
    assert_eq!(
        project
            .read(".ctxloom/content/bundles/v2/project/bundle.yaml")
            .as_deref(),
        Some("schema_version: 4\nversion: 1.0.0\n")
    );
    assert_eq!(
        project
            .read(".ctxloom/content/bundles/v2/project/profiles/default.yaml")
            .as_deref(),
        Some(SEED_PROFILE)
    );
    project
        .run(&["config", "show"])
        .succeeds()
        .contains("claude-code");
    // What it wrote reads: the seeded profile and the default agent resolve.
    project
        .run(&["profile", "list"])
        .succeeds()
        .contains("default");
    project
        .run(&["agent", "list"])
        .succeeds()
        .contains("default");
}

// cli/config.go create (documented condition): the engine the scaffold
// records; one nobody knows is refused and nothing is written.
#[test]
fn test_create_records_the_engine_it_is_given_and_refuses_one_nobody_knows() {
    for engine in ["mock", "opencode", "mock-launch"] {
        let project = Project::bare();
        project
            .run(&["config", "create", "--engine", engine])
            .succeeds();
        let written = project.read(CONFIG).expect("the config");
        let expected = format!(
            "agents:\n  default:\n    engine_home: session\n    llm: {engine}\n    profiles:\n      - default\n    runtime: host\nauth: token\nconfig:\n  use_distilled: true\ndefault_agent: default\nllm:\n  configs:\n    {engine}:\n      type: {engine}\n  defaults:\n    fast: {engine}\n    primary: {engine}\nschema_version: 8\n"
        );
        assert_eq!(written, expected);
    }
    let project = Project::bare();
    project
        .run(&["config", "create", "--engine", "nosuch"])
        .fails()
        .contains("unknown engine \"nosuch\"; valid engines: claude-code, mock, mock-launch, mock-lossy, mock-noskills, opencode");
    project.file_is_absent(".ctxloom");
    project
        .run(&["config", "create", "--format", "json"])
        .fails()
        .contains("does not support it yet");
    project.file_is_absent(".ctxloom");
}

// config.feature: Scenario: Create refuses an existing config and leaves it
// byte-for-byte alone
#[test]
fn test_create_refuses_an_existing_config_and_leaves_it_byte_for_byte_alone() {
    let project = Project::initialized();
    project.file_contains(CONFIG, "schema_version: 8");
    let before = project.tree(".ctxloom");
    project
        .run(&["config", "create"])
        .fails()
        .contains("config already exists");
    project
        .file_contains(CONFIG, "schema_version: 8")
        .file_lacks(CONFIG, "engine:");
    assert_eq!(project.read(CONFIG).as_deref(), Some(MINIMAL_CONFIG));
    assert_eq!(project.tree(".ctxloom"), before);
}

// config.feature: Scenario: Editing a project with no config says so and names
// the fix
#[test]
fn test_editing_a_project_with_no_config_says_so_and_names_the_fix() {
    let project = Project::bare();
    project
        .run(&["config", "edit"])
        .fails()
        .contains("no config at")
        .contains("ctxloom config create");
    project.file_is_absent(CONFIG);
}

// cli/config.go edit (documented condition): the config file is handed to
// $VISUAL, else $EDITOR (resolved from the environment alone, so a config
// that does not load can still be mended), and what the editor leaves is the
// file; an editor that fails is an error.
#[test]
fn test_edit_opens_the_config_in_the_editor_of_the_environment() {
    let project = Project::initialized();
    let editor = project.dir().join("editor.sh");
    files::put_program(&editor, "#!/bin/sh\nprintf '# edited\\n' >> \"$1\"\n");
    let editor = editor.to_str().expect("utf-8");
    project
        .run_with(&[("EDITOR", editor)], &["config", "edit"])
        .succeeds();
    assert_eq!(
        project.read(CONFIG).as_deref(),
        Some("schema_version: 8\n# edited\n")
    );
    // VISUAL wins over EDITOR.
    let visual = project.dir().join("visual.sh");
    files::put_program(&visual, "#!/bin/sh\nprintf '# visual\\n' >> \"$1\"\n");
    project
        .run_with(
            &[
                ("EDITOR", editor),
                ("VISUAL", visual.to_str().expect("utf-8")),
            ],
            &["config", "edit"],
        )
        .succeeds();
    project
        .file_contains(CONFIG, "# visual")
        .file_lacks(CONFIG, "# edited\n# edited");
    project
        .run_with(&[("EDITOR", "false")], &["config", "edit"])
        .fails();
    project
        .run_with(
            &[("EDITOR", editor)],
            &["config", "edit", "--format", "json"],
        )
        .fails()
        .contains("does not support it yet");
}

// operations.Update (documented condition, plan §10 S2): a write replaces the
// section it owns whole, with its keys sorted, and moves nothing else: not
// the comments, not the order of the sections, not a key ctxloom does not
// model.
#[test]
fn test_a_config_write_is_a_section_merge_with_sorted_keys() {
    let project = Project::initialized();
    project.write(
        CONFIG,
        "# my config\nschema_version: 8\noutput_dir: out # my own\ncompanions:\n  - acme\nllm:\n  configs:\n    zeta:\n      type: mock\n      model: z1\n    alpha:\n      type: mock\n",
    );
    project
        .run(&["llm", "create", "mid", "--type", "mock", "--model", "m"])
        .succeeds();
    assert_eq!(
        project.read(CONFIG).as_deref(),
        Some("# my config\nschema_version: 8\noutput_dir: out # my own\ncompanions:\n  - acme\nllm:\n  configs:\n    alpha:\n      type: mock\n    mid:\n      model: m\n      type: mock\n    zeta:\n      model: z1\n      type: mock\n")
    );
    // A key the write does not own is as the author left it.
    project.run(&["agent", "default", "ghost"]).succeeds();
    let after = project.read(CONFIG).expect("the config");
    assert!(
        after.starts_with("# my config\nschema_version: 8\noutput_dir: out # my own\n"),
        "{after}"
    );
    assert!(after.ends_with("default_agent: ghost\n"), "{after}");
}

// operations.Update / readExistingConfig (documented condition, plan §10 S2):
// a config that does not parse is not healed: the command stops and the file
// is as the user has it.
#[test]
fn test_a_config_write_never_heals_a_corrupt_file() {
    for corrupt in [
        "schema_version: 8\nllm: [unclosed\n",
        "::: not: [valid yaml\n\tbroken",
        "schema_version: 8\n\tagents: tab-indented\n",
    ] {
        let project = Project::initialized();
        project.write(CONFIG, corrupt);
        for args in [
            vec!["llm", "default", "claude-fast"],
            vec!["llm", "create", "big", "--type", "mock"],
            vec!["agent", "default", "x"],
            vec!["agent", "remove", "x", "--yes"],
        ] {
            project.run(&args).fails();
            assert_eq!(
                project.read(CONFIG).as_deref(),
                Some(corrupt),
                "{args:?} rewrote a corrupt config"
            );
        }
    }
    // A home config the same.
    let project = Project::initialized();
    project.write_home(CONFIG, "editor: [unclosed\n");
    project
        .run(&["companion", "remove", "acme", "--yes"])
        .fails();
    assert_eq!(
        project.read_home(CONFIG).as_deref(),
        Some("editor: [unclosed\n")
    );
}

// D15 (plan): a config of another generation than the binary reads is not
// rewritten either; the refusal says what to do.
#[test]
fn test_a_config_write_refuses_a_config_of_another_generation() {
    for body in ["schema_version: 9\n", "schema_version: 1\n"] {
        let project = Project::initialized();
        project.write(CONFIG, body);
        project.run(&["llm", "default", "claude-fast"]).fails();
        assert_eq!(project.read(CONFIG).as_deref(), Some(body));
    }
}
