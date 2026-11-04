//! Commands as claude's slash-command files.

use surface_ports::{Claim, Command, Planned};

/// Where claude loads commands from, below a directory's root.
const DIR: &str = ".claude/commands";

/// A command's file: its name with the path separators made dashes.
fn file_name(name: &str) -> String {
    format!("{}.md", name.replace('/', "-"))
}

/// Whether a name is safe to make a file of: nothing that climbs or is
/// absolute.
fn safe(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}

/// A YAML scalar for a frontmatter value: quoted where plain would not read
/// back as the same text.
fn scalar(text: &str) -> String {
    let plain = !text.is_empty()
        && text.trim() == text
        && !text.contains([':', '#', '\n', '"', '\'', '\\'])
        && !text.starts_with([
            '-', '?', '[', ']', '{', '}', ',', '&', '*', '!', '|', '>', '%', '@', '`',
        ]);
    if plain {
        return text.to_string();
    }
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A command as claude reads it: a description in the frontmatter (when it
/// has one), then the body.
pub(crate) fn render(command: &Command) -> String {
    let mut out = String::new();
    if !command.description.is_empty() {
        out.push_str("---\ndescription: ");
        out.push_str(&scalar(&command.description));
        out.push_str("\n---\n\n");
    }
    out.push_str(&command.body);
    out
}

pub(crate) fn plan(commands: &[Command], planned: &mut Planned) {
    for command in commands {
        if !safe(&command.name) {
            planned.skipped.push(format!(
                "skipping command {:?}: its name is not a safe file name",
                command.name
            ));
            continue;
        }
        planned.claims.push(Claim::Whole {
            path: format!("{DIR}/{}", file_name(&command.name)),
            bytes: render(command).into_bytes(),
            executable: false,
        });
    }
}

#[cfg(test)]
#[path = "commands.test.rs"]
mod tests;
