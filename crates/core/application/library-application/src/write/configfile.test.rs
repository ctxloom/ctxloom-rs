use angzarr_yaml::Value;
use ctxloom_ports::Scope;

use crate::fakes::{plain, Setup, CONFIG};
use crate::AppError;

fn scalar(s: &str) -> Value {
    Value::Scalar(s.to_string())
}

fn map(entries: &[(&str, Value)]) -> Value {
    Value::Map(
        entries
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect(),
    )
}

fn update(
    layers: &[&str],
    scope: Scope,
    key: &str,
    edit: impl FnOnce(Option<Value>) -> Result<Option<Value>, AppError>,
) -> (Result<String, AppError>, std::sync::Arc<crate::fakes::Mem>) {
    let (library, mem) = Setup {
        layers,
        ..Setup::default()
    }
    .fixture();
    let out = library.run(&plain(), |p| p.update_config(scope, key, edit));
    (out.result, mem)
}

// config_save.go saveLocked
#[test]
fn test_a_section_is_replaced_whole_sorted_and_the_rest_of_the_file_is_kept() {
    let layer = "# mine\nschema_version: 9\nauth: token # kept\nagents:\n  b:\n    llm: x\n";
    let (out, mem) = update(&[layer], Scope::Project, "agents", |current| {
        let mut agents = current.expect("the held section");
        *agents.entry("a") = map(&[("llm", scalar("y"))]);
        Ok(Some(agents))
    });
    assert_eq!(out.unwrap(), "/p/.ctxloom/config.yaml");
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "# mine\nschema_version: 9\nauth: token # kept\nagents:\n  a:\n    llm: y\n  b:\n    llm: x\n"
    );
}

#[test]
fn test_the_edit_sees_none_where_the_section_is_not_there() {
    let (out, mem) = update(&[CONFIG], Scope::Project, "companions", |current| {
        assert_eq!(current, None);
        Ok(Some(Value::Map(vec![(
            "acme".to_string(),
            map(&[("command", Value::Seq(vec![scalar("acme-bin")]))]),
        )])))
    });
    out.unwrap();
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "schema_version: 9\ncompanions:\n  acme:\n    command:\n      - acme-bin\n"
    );
}

#[test]
fn test_a_file_that_is_not_there_is_made_with_the_generation_stamp() {
    let (out, mem) = update(&[], Scope::Home, "companions", |_| {
        Ok(Some(Value::Map(vec![(
            "acme".to_string(),
            map(&[("command", Value::Seq(vec![scalar("acme-bin")]))]),
        )])))
    });
    assert_eq!(out.unwrap(), "/h/.ctxloom/config.yaml");
    assert_eq!(
        mem.config(Scope::Home).unwrap(),
        "schema_version: 9\ncompanions:\n  acme:\n    command:\n      - acme-bin\n"
    );
    assert_eq!(mem.config(Scope::Project), None);
}

#[test]
fn test_a_section_taken_out_is_taken_out_of_the_file() {
    let (out, mem) = update(
        &["schema_version: 9\ncompanions:\n  acme:\n    command:\n      - acme-bin\n"],
        Scope::Project,
        "companions",
        |_| Ok(None),
    );
    out.unwrap();
    assert_eq!(mem.config(Scope::Project).unwrap(), "schema_version: 9\n");
}

#[test]
fn test_a_change_that_changes_nothing_writes_nothing() {
    let (out, mem) = update(
        &["schema_version: 9\ncompanions:\n  acme:\n    command:\n      - acme-bin\n"],
        Scope::Project,
        "companions",
        Ok,
    );
    out.unwrap();
    assert_eq!(mem.writes(), 0);
    let (out, mem) = update(&[CONFIG], Scope::Project, "companions", |_| Ok(None));
    out.unwrap();
    assert_eq!(mem.writes(), 0);
}

// readExistingConfig: a file that does not parse is never replaced
#[test]
fn test_a_file_that_does_not_parse_is_refused_and_not_written() {
    // The library loads first and refuses a corrupt config before any write; a
    // layer that goes bad in between is the writer's own check.
    let (library, mem) = Setup {
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture();
    let out = library.run(&plain(), |p| {
        mem.set_config(Scope::Project, "schema_version: 9\nllm: [unclosed\n");
        p.update_config(Scope::Project, "agents", |_| Ok(Some(Value::Null)))
    });
    let err = out.result.unwrap_err();
    assert!(format!("{err:?}").contains("ConfigCorrupt"), "{err:?}");
    assert_eq!(mem.writes(), 0);
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "schema_version: 9\nllm: [unclosed\n"
    );
}

#[test]
fn test_an_edit_that_fails_writes_nothing() {
    let (out, mem) = update(&[CONFIG], Scope::Project, "agents", |_| {
        Err(crate::WriteError::BadBundleName {}.into())
    });
    assert!(out.is_err());
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_a_result_that_breaks_the_schema_is_refused_and_not_written() {
    let (out, mem) = update(&[CONFIG], Scope::Project, "session_reap_age", |_| {
        Ok(Some(map(&[("not", scalar("a string"))])))
    });
    let err = out.unwrap_err();
    assert!(format!("{err:?}").contains("ConfigBreaks"), "{err:?}");
    assert_eq!(mem.writes(), 0);
    assert_eq!(mem.config(Scope::Project).unwrap(), CONFIG);
}
