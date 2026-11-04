use angzarr_yaml::Value;
use ctxloom_ports::{EditorCommand, Source};

use super::*;
use crate::config::Config;

fn config(text: &str) -> Config {
    let value = angzarr_yaml::parse(text).unwrap();
    Config {
        raw: value.clone(),
        value,
        authored_labels: Default::default(),
        source: Source::Project,
    }
}

fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn command(program: &str, args: &[&str]) -> EditorCommand {
    EditorCommand {
        program: program.to_string(),
        args: args.iter().map(ToString::to_string).collect(),
    }
}

// config.go GetEditorCommand: config, then VISUAL, EDITOR, then nano
#[test]
fn test_the_configs_editor_comes_first_with_its_args_after_it() {
    let c = config("editor:\n  command: sh\n  args:\n    - -c\n    - 'true'\n    - editor\n");
    assert_eq!(
        editor_command(&c, &env(&[("VISUAL", "vi")])),
        command("sh", &["-c", "true", "editor"])
    );
}

#[test]
fn test_a_command_of_several_words_is_split_and_its_words_come_before_the_args() {
    let c = config("editor:\n  command: code --wait\n  args:\n    - -n\n");
    assert_eq!(editor_command(&c, &[]), command("code", &["--wait", "-n"]));
}

#[test]
fn test_with_no_editor_in_the_config_the_environment_decides() {
    let c = config("schema_version: 8\n");
    assert_eq!(
        editor_command(&c, &env(&[("VISUAL", "vim -u NONE"), ("EDITOR", "ed")])),
        command("vim", &["-u", "NONE"])
    );
    assert_eq!(
        editor_command(&c, &env(&[("EDITOR", "ed")])),
        command("ed", &[])
    );
    assert_eq!(
        editor_command(&c, &env(&[("VISUAL", "  "), ("EDITOR", "ed")])),
        command("ed", &[])
    );
    assert_eq!(editor_command(&c, &[]), command("nano", &[]));
    let blank = config("editor:\n  command: '   '\n");
    assert_eq!(
        editor_command(&blank, &env(&[("EDITOR", "ed")])),
        command("ed", &[])
    );
}

#[test]
fn test_the_environments_editor_alone() {
    assert_eq!(
        editor_from_env(&env(&[("VISUAL", "v"), ("EDITOR", "e")])),
        command("v", &[])
    );
    assert_eq!(
        editor_from_env(&env(&[("EDITOR", "e -x")])),
        command("e", &["-x"])
    );
    assert_eq!(editor_from_env(&[]), command("nano", &[]));
    let _ = Value::Null;
}
