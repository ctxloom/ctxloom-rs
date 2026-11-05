//! `session_purge_age` read from the layered configuration the sweep is run
//! in.

use ctxloom_ports::{ConfigFiles, Failure, LayerText, Scope, Source};
use reclaim_application::SweepConfig;

use super::*;

/// Layers fixed in memory, lowest first.
struct Layers(Vec<&'static str>);

impl ConfigFiles for Layers {
    fn source(&self) -> Source {
        Source::Project
    }
    fn app_dir(&self) -> String {
        "/p/.ctxloom".to_string()
    }
    fn layers(&self) -> Result<Vec<LayerText>, Failure> {
        Ok(self
            .0
            .iter()
            .enumerate()
            .map(|(i, text)| LayerText {
                path: format!("/layer{i}/config.yaml"),
                text: format!("schema_version: 8\n{text}"),
            })
            .collect())
    }
    fn layer(&self, _: Scope) -> Result<Option<LayerText>, Failure> {
        unreachable!("only the merged layers are read")
    }
    fn layer_path(&self, _: Scope) -> String {
        unreachable!("only the merged layers are read")
    }
    fn write_layer(&self, _: Scope, _: &str) -> Result<(), Failure> {
        unreachable!("the sweep writes no config")
    }
}

fn age(layers: Vec<&'static str>, environ: &[(&str, &str)]) -> Result<Option<String>, Failure> {
    let environ = environ
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    SweepSettings::new(Arc::new(Layers(layers)), environ).purge_age()
}

#[test]
fn the_configured_purge_age_is_read() {
    assert_eq!(
        age(vec!["session_purge_age: 90d\n"], &[]).unwrap(),
        Some("90d".to_string())
    );
}

#[test]
fn a_config_without_the_key_has_no_purge_age() {
    assert_eq!(age(vec!["workspace: worktree\n"], &[]).unwrap(), None);
    assert_eq!(age(vec![], &[]).unwrap(), None);
}

#[test]
fn the_project_layer_beats_the_home_layer() {
    let layers = vec!["session_purge_age: 90d\n", "session_purge_age: 30d\n"];
    assert_eq!(age(layers, &[]).unwrap(), Some("30d".to_string()));
}

#[test]
fn an_environment_override_beats_the_files() {
    let environ = [("CTXLOOM_CONFIG_session_purge_age", "7d")];
    assert_eq!(
        age(vec!["session_purge_age: 90d\n"], &environ).unwrap(),
        Some("7d".to_string())
    );
}

#[test]
fn a_config_that_will_not_parse_is_an_error_not_no_age() {
    assert!(age(vec!["session_purge_age: [unclosed\n"], &[]).is_err());
}
