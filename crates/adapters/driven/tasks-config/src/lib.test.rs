//! The layers, over temporary homes and work roots: never a real one.

use std::fs;
use std::path::Path;

use annals_diagnostic::Diagnostic;
use ports::Configuration;
use tempfile::TempDir;

use super::{split, ConfigError, TaskloomConfig};

/// A home and a work root, each with a config file holding `home` and
/// `root` when given.
struct Dirs {
    home: TempDir,
    root: TempDir,
}

impl Dirs {
    fn new(home: Option<&str>, root: Option<&str>) -> Dirs {
        let dirs = Dirs {
            home: TempDir::new().unwrap(),
            root: TempDir::new().unwrap(),
        };
        if let Some(text) = home {
            write(dirs.home.path(), text);
        }
        if let Some(text) = root {
            write(dirs.root.path(), text);
        }
        dirs
    }

    fn config(&self, env: &[(&str, &str)]) -> TaskloomConfig {
        TaskloomConfig::new(
            Some(self.home.path().to_path_buf()),
            Some(self.root.path().to_path_buf()),
            env.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }
}

fn write(dir: &Path, text: &str) {
    fs::create_dir_all(dir.join(".taskloom")).unwrap();
    fs::write(dir.join(".taskloom/config.yaml"), text).unwrap();
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

fn schema(config: &TaskloomConfig, overrides: &[&str]) -> Result<Vec<String>, ConfigError> {
    config.resolve(&strings(overrides))
}

const HOME: &str = "tag_schema:\n  - 'tagma.arity:\"home:x\"=scalar'\n";
const ROOT: &str = "schema_version: 1\nhoming: home\ntag_schema:\n  - 'tagma.arity:\"root:x\"=scalar'\n  - 'tagma.enum:\"root:y\"=\"a,b\"'\n";

#[test]
fn test_nothing_configured_is_no_declaration() {
    let dirs = Dirs::new(None, None);
    assert_eq!(schema(&dirs.config(&[]), &[]), Ok(Vec::new()));
    let nowhere = TaskloomConfig::new(None, None, Vec::new());
    assert_eq!(schema(&nowhere, &[]), Ok(Vec::new()));
}

#[test]
fn test_the_home_config_is_read() {
    let dirs = Dirs::new(Some(HOME), None);
    assert_eq!(
        schema(&dirs.config(&[]), &[]),
        Ok(strings(&[r#"tagma.arity:"home:x"=scalar"#]))
    );
}

// Each layer that sets tag_schema replaces the list below it whole.
#[test]
fn test_the_work_roots_config_replaces_the_homes_list() {
    let dirs = Dirs::new(Some(HOME), Some(ROOT));
    assert_eq!(
        schema(&dirs.config(&[]), &[]),
        Ok(strings(&[
            r#"tagma.arity:"root:x"=scalar"#,
            r#"tagma.enum:"root:y"="a,b""#
        ]))
    );
    // A root config that does not set it leaves the home's.
    let dirs = Dirs::new(Some(HOME), Some("homing: home\n"));
    assert_eq!(schema(&dirs.config(&[]), &[]).unwrap().len(), 1);
}

#[test]
fn test_the_environment_beats_the_files_and_an_override_beats_it() {
    let dirs = Dirs::new(Some(HOME), Some(ROOT));
    let env = [
        (
            "TASKLOOM_CONFIG_TAG_SCHEMA",
            r#"tagma.arity:"env:x"=scalar, tagma.enum:"env:y"="a,b""#,
        ),
        ("HOME", "/elsewhere"),
        ("CTXLOOM_CONFIG_WHATEVER", "x"),
    ];
    let config = dirs.config(&env);
    assert_eq!(
        schema(&config, &[]),
        Ok(strings(&[
            r#"tagma.arity:"env:x"=scalar"#,
            r#"tagma.enum:"env:y"="a,b""#
        ]))
    );
    assert_eq!(
        schema(
            &config,
            &[r#"Tag_Schema=tagma.arity:"set:x"=scalar"#, "homing=repo"]
        ),
        Ok(strings(&[r#"tagma.arity:"set:x"=scalar"#]))
    );
    assert_eq!(
        schema(&config, &["tag_schema=a", "tag_schema=b"]),
        Ok(strings(&["b"])),
        "the last override wins"
    );
    assert_eq!(
        schema(&config, &["tag_schema="]),
        Ok(Vec::new()),
        "set to none"
    );
}

#[test]
fn test_through_the_port() {
    let dirs = Dirs::new(None, Some(ROOT));
    let mut config = dirs.config(&[]);
    assert_eq!(config.tag_schema(&[]).unwrap().len(), 2);
    let error = config.tag_schema(&strings(&["nonsense"])).unwrap_err();
    assert_eq!(error.code(), "tasks-config-not-an-override");
}

#[test]
fn test_a_file_that_is_not_taskloom_config_is_refused() {
    for (text, code) in [
        ("homeing: home\n", "tasks-config-malformed"),
        ("tag_schema: not-a-list\n", "tasks-config-malformed"),
        ("[unclosed\n", "tasks-config-malformed"),
        ("schema_version: 2\n", "tasks-config-newer"),
    ] {
        let dirs = Dirs::new(None, Some(text));
        let error = schema(&dirs.config(&[]), &[]).unwrap_err();
        assert_eq!(error.code(), code, "{text}");
        assert!(error.to_string().contains("config.yaml"), "{error}");
    }
    for fine in [
        "",
        "# nothing here\n",
        "schema_version: 0\n",
        "schema_version: 1\n",
    ] {
        let dirs = Dirs::new(Some(fine), None);
        assert_eq!(schema(&dirs.config(&[]), &[]), Ok(Vec::new()), "{fine:?}");
    }
}

#[test]
fn test_a_config_file_that_cannot_be_read_is_refused() {
    let dirs = Dirs::new(None, None);
    // A directory where the file should be.
    fs::create_dir_all(dirs.root.path().join(".taskloom/config.yaml")).unwrap();
    let error = schema(&dirs.config(&[]), &[]).unwrap_err();
    assert_eq!(error.code(), "tasks-config-unreadable");
}

#[test]
fn test_an_override_or_variable_naming_no_key_is_refused() {
    let dirs = Dirs::new(None, None);
    let error = schema(&dirs.config(&[("TASKLOOM_CONFIG_TAGS", "x")]), &[]).unwrap_err();
    assert_eq!(
        error,
        ConfigError::UnknownKey {
            source: "TASKLOOM_CONFIG_TAGS".to_string(),
            key: "tags".to_string()
        }
    );
    for bad in ["tagz=x", "=x", " =x", "tag_schema"] {
        assert!(schema(&dirs.config(&[]), &[bad]).is_err(), "{bad}");
    }
    let known = schema(
        &dirs.config(&[
            ("TASKLOOM_CONFIG_HOMING", "home"),
            ("TASKLOOM_CONFIG_SCHEMA_VERSION", "1"),
        ]),
        &["schema_version=1"],
    );
    assert_eq!(known, Ok(Vec::new()));
}

#[test]
fn test_a_list_splits_on_commas_outside_quotes() {
    assert_eq!(split("a, b,,c "), Some(strings(&["a", "b", "c"])));
    assert_eq!(
        split(r#"k="a,b",x:"y,z"=1"#),
        Some(strings(&[r#"k="a,b""#, r#"x:"y,z"=1"#]))
    );
    assert_eq!(
        split(r#"k="a\",b",c"#),
        Some(strings(&[r#"k="a\",b""#, "c"])),
        "an escaped quote does not close"
    );
    assert_eq!(split(""), Some(Vec::new()));
    assert_eq!(split(r#"k="a,b"#), None);
    let dirs = Dirs::new(None, None);
    let error = schema(&dirs.config(&[]), &[r#"tag_schema=k="a"#]).unwrap_err();
    assert_eq!(error.code(), "tasks-config-unterminated");
}
