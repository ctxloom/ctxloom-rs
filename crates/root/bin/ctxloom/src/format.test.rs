//! The `--format` matrix (D28): json and text only. A dropped format is
//! refused like any unknown one: a usage refusal, exit 2, naming the formats
//! there are. An empty list is `[]` in json and `(none)` in text. Off a
//! terminal the format is json (a terminal is not testable here: a process
//! run by these tests has a pipe for stdout).

use super::{run, Ran};
use serde_json::json;

const SAMPLE: [&str; 3] = ["test", "sample", "--format"];

fn sample(format: &str, empty: bool) -> Ran {
    let mut args = SAMPLE.to_vec();
    args.push(format);
    if empty {
        args.push("--empty");
    }
    run(&args)
}

#[test]
fn test_json_is_the_sample_document() {
    let ran = sample("json", false);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert_eq!(
        ran.json(),
        json!([{"name": "alpha", "count": 1}, {"name": "beta", "count": 2}])
    );
    assert_eq!(ran.stderr, "");
}

#[test]
fn test_text_is_for_people_not_json() {
    let ran = sample("text", false);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert!(ran.stdout.contains("alpha"), "{}", ran.said());
    assert!(ran.stdout.contains("beta"), "{}", ran.said());
    assert!(serde_json::from_str::<serde_json::Value>(&ran.stdout).is_err());
    assert_eq!(ran.stderr, "");
}

#[test]
fn test_an_empty_list_is_brackets_in_json_and_none_in_text() {
    let json = sample("json", true);
    assert_eq!(json.code, 0, "{}", json.said());
    assert_eq!(json.stdout.trim_end(), "[]");
    let text = sample("text", true);
    assert_eq!(text.code, 0, "{}", text.said());
    assert_eq!(text.stdout.trim_end(), "(none)");
}

#[test]
fn test_off_a_terminal_the_format_is_json() {
    let ran = run(&["test", "sample"]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert_eq!(ran.json().as_array().map(Vec::len), Some(2));
    let empty = run(&["test", "sample", "--empty"]);
    assert_eq!(empty.stdout.trim_end(), "[]", "{}", empty.said());
}

#[test]
fn test_the_dropped_formats_are_refused_like_any_unknown_one() {
    // A format nobody ever had: the shape every refusal must share.
    let reference = run(&["--format", "xml", "test", "sample"]);
    assert_eq!(reference.code, 2, "{}", reference.said());
    for format in ["yaml", "toml", "markdown"] {
        for args in [
            vec!["--format", format, "test", "sample"],
            vec!["test", "sample", "--format", format],
            vec!["version", "--format", format],
        ] {
            let ran = run(&args);
            assert_eq!(ran.code, 2, "{args:?}: {}", ran.said());
            assert_eq!(ran.stdout, "", "{args:?}");
            for remedy in [format, "json", "text"] {
                assert!(ran.stderr.contains(remedy), "{args:?}: {}", ran.said());
            }
        }
        let ran = run(&["--format", format, "test", "sample"]);
        assert_eq!(
            ran.stderr.replace(format, "FORMAT"),
            reference.stderr.replace("xml", "FORMAT"),
            "{format} is refused as xml is"
        );
    }
}

#[test]
fn test_the_equals_spelling_is_refused_the_same() {
    let ran = run(&["--format=yaml", "test", "sample"]);
    assert_eq!(ran.code, 2, "{}", ran.said());
    assert_eq!(ran.stdout, "");
}
