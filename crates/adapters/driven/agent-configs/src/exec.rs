//! Which program a command line runs, as Go's `exectoken` reads it: the
//! base name of its first token, quotes and a `.exe` taken off. An entry
//! is a writer's own when it runs the writer's program, wherever the
//! program was installed.

/// The base name of the program `command` runs.
pub fn program(command: &str) -> &str {
    let exe = first_token(command);
    let exe = exe.rsplit(['/', '\\']).next().unwrap_or(exe);
    let exe = exe.strip_suffix(".exe").unwrap_or(exe);
    exe.trim_matches(['"', '\''])
}

/// Whether `command` runs `owner`: never, for no owner.
pub fn runs(command: &str, owner: &str) -> bool {
    !owner.is_empty() && program(command) == owner
}

fn first_token(command: &str) -> &str {
    let command = command.trim_start_matches([' ', '\t']);
    let Some(quote) = command.chars().next().filter(|c| *c == '"' || *c == '\'') else {
        return command.split([' ', '\t']).next().unwrap_or_default();
    };
    let rest = &command[1..];
    match rest.find(quote) {
        Some(end) => &rest[..end],
        // An unterminated quote: the rest.
        None => rest,
    }
}

#[cfg(test)]
#[path = "exec.test.rs"]
mod tests;
