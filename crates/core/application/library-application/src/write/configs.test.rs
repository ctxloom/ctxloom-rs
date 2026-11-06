//! `config.feature`'s write scenarios at the application port.

use crate::fakes::{Mem, Setup};
use crate::Library;
use std::sync::Arc;

fn world() -> (Library, Arc<Mem>) {
    Setup::default().fixture()
}

fn err<T>(out: crate::Outcome<T>) -> String {
    format!("{:?}", out.result.err().expect("an error"))
}

// config.feature: Create scaffolds the config, the remotes registry and the seeded profile
#[test]
fn test_create_writes_the_four_files_go_writes() {
    let (library, mem) = world();
    let done = library.config_create("").result.unwrap();
    assert_eq!(done.path, "/work/.ctxloom/config.yaml");
    assert_eq!(
        mem.scaffold_paths(),
        [
            "config.yaml",
            "content/bundles/v2/project/bundle.yaml",
            "content/bundles/v2/project/profiles/default.yaml",
            "remotes.yaml"
        ]
    );
    assert_eq!(
        mem.scaffold_text("config.yaml").unwrap(),
        library::write::scaffold::initial_config("claude-code")
    );
    assert_eq!(
        mem.scaffold_text("remotes.yaml").unwrap(),
        library::write::scaffold::DEFAULT_REMOTES
    );
    assert_eq!(
        mem.scaffold_text("content/bundles/v2/project/bundle.yaml")
            .unwrap(),
        "schema_version: 4\nversion: 1.0.0\n"
    );
    assert_eq!(
        mem.scaffold_text("content/bundles/v2/project/profiles/default.yaml")
            .unwrap(),
        library::write::scaffold::SEED_PROFILE
    );
    assert_eq!(mem.log(), ["scaffold"], "one write");
}

#[test]
fn test_create_records_the_engine_it_is_given() {
    let (library, mem) = world();
    library.config_create("mock").result.unwrap();
    assert_eq!(
        mem.scaffold_text("config.yaml").unwrap(),
        library::write::scaffold::initial_config("mock")
    );
}

#[test]
fn test_create_refuses_an_engine_nobody_knows_and_writes_nothing() {
    let (library, mem) = world();
    let said = err(library.config_create("nosuch"));
    assert!(
        said.contains("ConfigEngine") && said.contains("nosuch"),
        "{said}"
    );
    assert_eq!(mem.writes(), 0);
}

// config.feature: Create refuses an existing config and leaves it byte-for-byte alone
#[test]
fn test_create_refuses_an_existing_config() {
    let (library, mem) = world();
    library.config_create("").result.unwrap();
    let before = mem.scaffold_paths();
    let text = mem.scaffold_text("config.yaml");
    assert!(err(library.config_create("")).contains("ConfigExists"));
    assert_eq!(mem.scaffold_paths(), before);
    assert_eq!(mem.scaffold_text("config.yaml"), text);
    assert_eq!(mem.log(), ["scaffold"]);
}

// operations.scaffoldSeedProfile: the project bundle and its seed profile are kept if there
#[test]
fn test_create_does_not_replace_a_project_bundle_or_seed_profile_that_is_there() {
    let (library, mem) = world();
    mem.set_scaffold_for_test("content/bundles/v2/project/bundle.yaml", "MINE");
    mem.set_scaffold_for_test(
        "content/bundles/v2/project/profiles/default.yaml",
        "MY SEED",
    );
    library.config_create("").result.unwrap();
    assert_eq!(
        mem.scaffold_text("content/bundles/v2/project/bundle.yaml")
            .unwrap(),
        "MINE"
    );
    assert_eq!(
        mem.scaffold_text("content/bundles/v2/project/profiles/default.yaml")
            .unwrap(),
        "MY SEED"
    );
    assert!(mem.scaffold_text("config.yaml").is_some());
}

// config.feature: Editing a project with no config says so and names the fix
#[test]
fn test_edit_without_a_config_says_so_and_runs_no_editor() {
    let (library, mem) = world();
    let said = err(library.config_edit());
    assert!(
        said.contains("ConfigMissing") && said.contains("/work/.ctxloom/config.yaml"),
        "{said}"
    );
    assert!(mem.opened().is_empty());
}

#[test]
fn test_edit_opens_the_config_in_the_editor_of_the_environment() {
    let (library, mem) = Setup {
        environ: &[("EDITOR", "ed"), ("VISUAL", "vis")],
        ..Setup::default()
    }
    .fixture();
    mem.set_scaffold_for_test("config.yaml", "schema_version: 9\n");
    library.config_edit().result.unwrap();
    assert_eq!(mem.opened(), ["/work/.ctxloom/config.yaml"]);
}

// A config that does not load is what `config edit` is for.
#[test]
fn test_edit_works_on_a_config_that_does_not_load() {
    let (library, mem) = Setup {
        layers: &["schema_version: 9\nllm: [unclosed\n"],
        ..Setup::default()
    }
    .fixture();
    mem.set_scaffold_for_test("config.yaml", "llm: [unclosed\n");
    assert!(library.config_edit().result.is_ok());
    assert_eq!(mem.opened().len(), 1);
}
