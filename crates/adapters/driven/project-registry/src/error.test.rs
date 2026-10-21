//! The errors' English and the catalog.

use annals_diagnostic::{Arg, Diagnostic};

use super::{diagnostic_samples, RegistryError};

fn arg_names(d: &dyn Diagnostic) -> Vec<&'static str> {
    d.args().into_iter().map(|(name, _)| name).collect()
}

#[test]
fn test_each_error_renders_its_english_fallback() {
    let english: Vec<String> = diagnostic_samples().iter().map(|d| d.to_string()).collect();
    assert_eq!(
        english,
        [
            "no home directory: set HOME",
            "/home/u/.ctxloom/projects/index.yaml: permission denied",
            "the project id ../x from --project is not a valid id: it contains '/'",
            "the project registry /home/u/.ctxloom/projects/index.yaml cannot be read: permission denied",
            "no free project id after 100 tries",
            "the working directory cannot be read: No such file or directory (os error 2)",
            "index.yaml is not an absolute path: nothing is written relative to the working directory",
            "the project registry's lock /home/u/.ctxloom/projects/index.yaml.lock is still held after 10000ms: another ctxloom or taskloom process holds it; try again once it exits",
            "the project registry's lock /home/u/.ctxloom/projects/index.yaml.lock was replaced 16 times while being taken: try again",
            "the project registry or a marker changed while the change was planned: nothing was changed; run it again",
            "/home/u/.ctxloom/projects/jolly-mute-magma is not a project's database directory: not removed",
            "/work/linked is a linked worktree whose git directory /work/main/.git/worktrees/linked is gone: run `git worktree repair` from its primary checkout, or remove the worktree",
        ]
    );
}

#[test]
fn test_each_error_gives_its_args_values() {
    let error = RegistryError::InvalidId {
        id: "a".to_string(),
        origin: "b".to_string(),
        reason: "c".to_string(),
    };
    let values: Vec<String> = error
        .args()
        .into_iter()
        .map(|(_, arg)| match arg {
            Arg::Str(s) => s,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(values, ["a", "b", "c"]);
    let io = RegistryError::Io {
        path: "p".to_string(),
        detail: "d".to_string(),
    };
    assert!(
        matches!(&io.args()[..], [("path", Arg::Str(p)), ("detail", Arg::Str(d))] if p == "p" && d == "d")
    );
    let cwd = RegistryError::NoWorkingDirectory {
        detail: "d".to_string(),
    };
    assert!(matches!(&cwd.args()[..], [("detail", Arg::Str(d))] if d == "d"));
    let relative = RegistryError::RelativePath {
        path: "p".to_string(),
    };
    assert!(matches!(&relative.args()[..], [("path", Arg::Str(p))] if p == "p"));
    let not_db = RegistryError::NotADatabase {
        path: "p".to_string(),
    };
    assert!(matches!(&not_db.args()[..], [("path", Arg::Str(p))] if p == "p"));
    let stale = RegistryError::StaleWorktree {
        root: "r".to_string(),
        git_dir: "g".to_string(),
    };
    assert!(matches!(
        &stale.args()[..],
        [("root", Arg::Str(r)), ("git_dir", Arg::Str(g))] if r == "r" && g == "g"
    ));
    let exhausted = RegistryError::Exhausted { attempts: 7 }.args();
    assert!(matches!(exhausted[0], ("attempts", Arg::Uint(7))));
    let timeout = RegistryError::LockTimeout {
        path: "p".to_string(),
        waited_ms: 9,
    };
    assert!(matches!(
        &timeout.args()[..],
        [("path", Arg::Str(p)), ("waited_ms", Arg::Uint(9))] if p == "p"
    ));
    let replaced = RegistryError::LockReplaced {
        path: "p".to_string(),
        attempts: 3,
    };
    assert!(matches!(
        &replaced.args()[..],
        [("path", Arg::Str(p)), ("attempts", Arg::Uint(3))] if p == "p"
    ));
}

#[test]
fn test_every_error_has_a_catalog_message_with_exactly_its_args() {
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, PatternElement};

    let ftl = crate::EN_US.1;
    let resource = fluent_syntax::parser::parse(ftl).expect("the catalog parses");
    let samples = diagnostic_samples();
    let mut catalog: Vec<&str> = resource
        .body
        .iter()
        .filter_map(|e| match e {
            Entry::Message(m) => Some(m.id.name),
            _ => None,
        })
        .collect();
    catalog.sort_unstable();
    let mut sampled: Vec<&str> = samples.iter().map(|s| s.code()).collect();
    sampled.sort_unstable();
    assert_eq!(catalog, sampled, "catalog messages and samples");
    for sample in samples {
        let message = resource
            .body
            .iter()
            .find_map(|e| match e {
                Entry::Message(m) if m.id.name == sample.code() => Some(m),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no message for {}", sample.code()));
        let mut used: Vec<&str> = message
            .value
            .iter()
            .flat_map(|p| &p.elements)
            .filter_map(|el| match el {
                PatternElement::Placeable {
                    expression: Expression::Inline(InlineExpression::VariableReference { id }),
                } => Some(id.name),
                _ => None,
            })
            .collect();
        used.sort_unstable();
        let mut args = arg_names(&*sample);
        args.sort_unstable();
        assert_eq!(used, args, "{}", sample.code());
    }
}
