//! The diagnostics: each says its code and args, and its message uses exactly them.

use angzarr_diagnostic::{Arg, Diagnostic, Report};

use super::{diagnostic_samples, TasksError, EN_US};

// Every sample says its code (Display is the code then the args, never
// English), its catalog message uses exactly its args, and the samples are
// not empty.
#[test]
fn every_sample_displays_its_code_and_its_message_uses_exactly_its_args() {
    let catalogs: Vec<&str> = vec![EN_US.1];
    let samples = diagnostic_samples();
    assert!(!samples.is_empty());
    for sample in samples {
        let shown = format!("{sample}");
        assert!(shown.starts_with(sample.code()), "{shown}");
        let names: Vec<&str> = sample.args().into_iter().map(|(n, _)| n).collect();
        assert_eq!(shown.contains('='), !names.is_empty(), "{shown}");
        let line = catalogs
            .iter()
            .flat_map(|c| c.lines())
            .find(|l| l.starts_with(&format!("{} =", sample.code())))
            .unwrap_or_else(|| panic!("{} has no message", sample.code()));
        let mut used: Vec<&str> = line
            .split('$')
            .skip(1)
            .map(|r| {
                r.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                    .next()
                    .unwrap()
            })
            .collect();
        used.sort_unstable();
        used.dedup();
        let mut names = names;
        names.sort_unstable();
        assert_eq!(names, used, "{}", sample.code());
    }
}

fn shown(e: &TasksError) -> (String, Vec<(String, String)>) {
    let args = e
        .args()
        .into_iter()
        .map(|(name, arg)| {
            let value = match arg {
                Arg::Str(s) => s,
                Arg::Nested(cause) => format!("nested {cause}"),
                other => panic!("unexpected {other:?}"),
            };
            (name.to_string(), value)
        })
        .collect();
    (e.code().to_string(), args)
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect()
}

#[test]
fn each_error_has_its_own_code_and_args() {
    let p = || "taskloom".to_string();
    let s = |t: &str| t.to_string();
    let cases = [
        (
            TasksError::NotFound { program: p() },
            "tasks-ports-not-found",
            pairs(&[("program", "taskloom")]),
        ),
        (
            TasksError::Unrunnable {
                program: p(),
                why: s("denied"),
            },
            "tasks-ports-unrunnable",
            pairs(&[("program", "taskloom"), ("why", "denied")]),
        ),
        (
            TasksError::Refused {
                program: p(),
                exit: s("exit status: 2"),
                cause: Report::new("tag-rules").arg("findings", "x"),
            },
            "tasks-ports-refused",
            pairs(&[
                ("program", "taskloom"),
                ("exit", "exit status: 2"),
                ("cause", "nested tag-rules findings=x"),
            ]),
        ),
        (
            TasksError::Failed {
                program: p(),
                exit: s("exit status: 1"),
                stderr: s("boom"),
            },
            "tasks-ports-failed",
            pairs(&[
                ("program", "taskloom"),
                ("exit", "exit status: 1"),
                ("stderr", "boom"),
            ]),
        ),
        (
            TasksError::Unparsable {
                program: p(),
                why: s("eof"),
            },
            "tasks-ports-unparsable",
            pairs(&[("program", "taskloom"), ("why", "eof")]),
        ),
        (
            TasksError::TimedOut {
                program: p(),
                timeout: s("30s"),
            },
            "tasks-ports-timed-out",
            pairs(&[("program", "taskloom"), ("timeout", "30s")]),
        ),
    ];
    for (error, code, args) in cases {
        assert_eq!(shown(&error), (code.to_string(), args), "{error}");
    }
}

#[test]
fn the_catalog_is_this_crates() {
    assert_eq!(EN_US.0, "tasks-ports");
}
