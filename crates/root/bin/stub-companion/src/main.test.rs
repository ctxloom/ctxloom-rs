use super::*;

/// Files by the name beside the program.
fn beside<'a>(files: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |suffix| {
        files
            .iter()
            .find(|(name, _)| *name == suffix)
            .map(|(_, text)| (*text).to_string())
    }
}

fn args(of: &[&str]) -> Vec<String> {
    of.iter().map(ToString::to_string).collect()
}

#[test]
fn version_answers_json_named_for_the_program() {
    let reply = answer(
        "/bin/ctxloom-companion-stub",
        &args(&["version", "--format", "json"]),
        &beside(&[]),
    );
    assert_eq!(reply.code, 0);
    assert_eq!(
        reply.stdout,
        "{\"name\":\"ctxloom-companion-stub\",\"version\":\"0.0.0-stub\"}\n"
    );
}

#[test]
fn loadout_prints_the_loadout_file_verbatim() {
    let files = [("loadout", "run:\n  version: 1\n")];
    let reply = answer(
        "/x/reprise",
        &args(&["loadout", "--format", "yaml"]),
        &beside(&files),
    );
    assert_eq!(
        (reply.code, reply.stdout.as_str()),
        (0, "run:\n  version: 1\n")
    );
}

#[test]
fn loadout_without_a_file_is_a_clean_nothing() {
    let reply = answer(
        "/x/reprise",
        &args(&["loadout", "--format", "yaml"]),
        &beside(&[]),
    );
    assert_eq!((reply.code, reply.stdout.as_str()), (0, ""));
}

#[test]
fn an_exit_file_makes_the_loadout_fail_with_that_code() {
    let files = [("loadout", "run:\n"), ("exit", "7\n")];
    let reply = answer(
        "/x/reprise",
        &args(&["loadout", "--format", "yaml"]),
        &beside(&files),
    );
    assert_eq!((reply.code, reply.stdout.as_str()), (7, ""));
}

#[test]
fn the_probe_must_be_asked_for_yaml() {
    let files = [("loadout", "run:\n")];
    for asked in [
        &["loadout"][..],
        &["loadout", "--format", "json"],
        &["loadout", "yaml"],
    ] {
        let reply = answer("/x/reprise", &args(asked), &beside(&files));
        assert_eq!(reply.code, 2, "{asked:?}");
    }
}

#[test]
fn anything_else_is_a_usage_error() {
    assert_eq!(answer("/x/p", &args(&[]), &beside(&[])).code, 2);
    assert_eq!(answer("/x/p", &args(&["serve"]), &beside(&[])).code, 2);
}

#[test]
fn a_hang_file_never_answers() {
    let files = [("hang", "")];
    let reply = answer(
        "/x/p",
        &args(&["loadout", "--format", "yaml"]),
        &beside(&files),
    );
    assert!(reply.hang);
}

#[test]
fn the_program_name_is_the_last_segment() {
    assert_eq!(
        program_name("/a/b/ctxloom-companion-stub"),
        "ctxloom-companion-stub"
    );
    assert_eq!(program_name("reprise"), "reprise");
}
