//! Which editor to run (Go's `GetEditorCommand` and `EditorFromEnv`): the
//! config's `editor.command` with `editor.args` after it, else `$VISUAL`, else
//! `$EDITOR`, else nano. A value with several words is split on whitespace
//! (quoting is not supported: a program whose path has a space is configured
//! with `editor.command` and `editor.args`).

use angzarr_yaml::Value;
use ctxloom_ports::EditorCommand;

use crate::config::Config;

/// `value` split on whitespace into a program and the arguments that follow
/// it; none for a blank value.
fn words(value: &str) -> Option<(String, Vec<String>)> {
    let mut parts = value.split_whitespace().map(ToString::to_string);
    let program = parts.next()?;
    Some((program, parts.collect()))
}

fn from_env(environ: &[(String, String)]) -> EditorCommand {
    for name in ["VISUAL", "EDITOR"] {
        let value = environ
            .iter()
            .find(|(n, _)| n == name)
            .map_or("", |(_, v)| v.as_str());
        if let Some((program, args)) = words(value) {
            return EditorCommand { program, args };
        }
    }
    EditorCommand {
        program: "nano".to_string(),
        args: Vec::new(),
    }
}

/// The editor for a command that has the configuration.
pub(crate) fn editor_command(config: &Config, environ: &[(String, String)]) -> EditorCommand {
    if let Some((program, mut args)) = words(&config.text("editor.command")) {
        if let Some(Value::Seq(more)) = config.value.get("editor").and_then(|e| e.get("args")) {
            args.extend(
                more.iter()
                    .filter_map(|a| a.as_str().map(ToString::to_string)),
            );
        }
        return EditorCommand { program, args };
    }
    from_env(environ)
}

/// The editor of the environment alone, for `config edit`, which has to work
/// on a config that does not load.
pub(crate) fn editor_from_env(environ: &[(String, String)]) -> EditorCommand {
    from_env(environ)
}

#[cfg(test)]
#[path = "editor.test.rs"]
mod tests;
