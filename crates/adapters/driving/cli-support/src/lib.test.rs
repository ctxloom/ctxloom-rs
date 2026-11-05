//! The shared glue: notices said in either format, the warning, the argument
//! readers and `emit`.

use std::fmt;

use angzarr_diagnostic::{Arg, Diagnostic};
use angzarr_presentation::{Catalog, Format};
use clap::{Arg as ClapArg, ArgAction, Command};

use super::*;

/// A notice with a message and perhaps a remedy, as the application's are.
#[derive(Debug)]
struct Note(&'static str, &'static str);

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for Note {
    fn code(&self) -> &str {
        "note"
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        vec![("text", self.0.into()), ("remedy", self.1.into())]
    }
}

const FTL: &str =
    "note = { $text }{ $remedy ->\n   *[other] { \"\" }\n }\nnote-fix = { $remedy }\n";

fn catalog() -> Catalog {
    Catalog::en_us(&[("cli-support-test", FTL)]).expect("a catalog")
}

fn notices() -> [Note; 2] {
    [Note("it broke", "mend it"), Note("only words", "")]
}

fn notices_said(format: Format) -> String {
    let mut err = Vec::new();
    say_notices(&notices(), "ctxloom", format, &catalog(), &mut err);
    String::from_utf8(err).expect("utf-8")
}

#[test]
fn test_a_remedy_is_the_code_with_fix_and_the_args() {
    let note = Note("a", "b");
    let fix = FixOf(&note, "note-fix".to_string());
    assert_eq!(fix.code(), "note-fix");
    assert_eq!(fix.args().len(), 2);
    assert_eq!(format!("{fix:?}"), "FixOf(note-fix)");
    assert!(fix.to_string().starts_with("note-fix"), "{fix}");
}

#[test]
fn test_notices_under_text_are_words_with_their_fix() {
    assert_eq!(
        notices_said(Format::Text),
        "ctxloom: warning: it broke\n  fix: mend it\nctxloom: warning: only words\n"
    );
}

#[test]
fn test_notices_under_json_are_lines_with_a_remedy_when_there_is_one() {
    assert_eq!(
        notices_said(Format::Json),
        "{\"prog\":\"ctxloom\",\"warning\":\"it broke\",\"remedy\":\"mend it\"}\n{\"prog\":\"ctxloom\",\"warning\":\"only words\"}\n"
    );
}

#[test]
fn test_a_warning_under_text_is_the_program_and_the_message() {
    let mut err = Vec::new();
    say_warning(
        &Note("it broke", ""),
        "ctxloom",
        Format::Text,
        &catalog(),
        &mut err,
    );
    assert_eq!(
        String::from_utf8(err).expect("utf-8"),
        "ctxloom: it broke\n"
    );
}

#[test]
fn test_a_warning_under_json_is_the_envelope() {
    let mut err = Vec::new();
    say_warning(
        &Note("it broke", ""),
        "ctxloom",
        Format::Json,
        &catalog(),
        &mut err,
    );
    let json = String::from_utf8(err).expect("utf-8");
    assert!(json.contains("\"message\": \"it broke\""), "{json}");
    assert!(json.contains("\"code\": \"note\""), "{json}");
    assert!(!json.contains("ctxloom:"), "{json}");
}

fn matches(line: &[&str]) -> clap::ArgMatches {
    Command::new("t")
        .arg(ClapArg::new("on").long("on").action(ArgAction::SetTrue))
        .arg(ClapArg::new("name").long("name"))
        .arg(
            ClapArg::new("tags")
                .long("tags")
                .value_delimiter(',')
                .action(ArgAction::Append),
        )
        .get_matches_from(line)
}

#[test]
fn test_arguments_are_read_by_name() {
    let given = matches(&["t", "--on", "--name", "x", "--tags", "a,b"]);
    assert!(flag(&given, "on"));
    assert_eq!(one(&given, "name"), Some("x".to_string()));
    assert_eq!(many(&given, "tags"), vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn test_arguments_not_given_or_not_declared_read_as_empty() {
    let given = matches(&["t"]);
    assert!(!flag(&given, "on"));
    assert!(!flag(&given, "nope"));
    assert_eq!(one(&given, "name"), None);
    assert_eq!(one(&given, "nope"), None);
    assert!(many(&given, "tags").is_empty());
    assert!(many(&given, "nope").is_empty());
}

#[test]
fn test_emit_writes_json_or_the_text_by_format() {
    let mut json = Vec::new();
    emit(Format::Json, &mut json, &vec![1], |_| panic!("text")).expect("written");
    let json = String::from_utf8(json).expect("utf-8");
    assert_eq!(json.replace(char::is_whitespace, ""), "[1]");
    let mut text = Vec::new();
    emit(Format::Text, &mut text, &vec![1], |w| w.write_all(b"words")).expect("written");
    assert_eq!(text, b"words");
}

#[test]
fn test_a_call_answers_in_the_format_asked_for_else_the_default() {
    let given = matches(&["t"]);
    let asked = Call::new(&given, Some(Format::Json), Format::Text);
    assert_eq!(asked.format(), Format::Json);
    let unasked = Call::new(&given, None, Format::Text);
    assert_eq!(unasked.format(), Format::Text);
}

#[test]
fn test_a_failure_carries_its_cause_and_exit() {
    let failure = Failure::new(Note("x", ""), 2);
    assert_eq!(failure.exit, 2);
    assert_eq!(failure.cause.code(), "note");
}

// ---------------------------------------------------------------- characterization
//
// Pinned before the crate moves to the shared repository (A0): the exact
// bytes written on the error stream and the exit codes carried, so the move
// is a rename of code plus these tests.

/// A notice whose code the catalog does not know a `-fix` of.
#[derive(Debug)]
struct Plain;

impl fmt::Display for Plain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for Plain {
    fn code(&self) -> &str {
        "plain"
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        vec![("text", "no \"remedy\" here".into())]
    }
}

fn plain_catalog() -> Catalog {
    Catalog::en_us(&[("cli-support-plain", "plain = { $text }\n")]).expect("a catalog")
}

#[test]
fn test_a_notice_without_a_fix_message_says_only_its_warning() {
    let mut text = Vec::new();
    say_notices(&[Plain], "prog", Format::Text, &plain_catalog(), &mut text);
    assert_eq!(
        String::from_utf8(text).expect("utf-8"),
        "prog: warning: no \"remedy\" here\n"
    );
    let mut json = Vec::new();
    say_notices(&[Plain], "prog", Format::Json, &plain_catalog(), &mut json);
    assert_eq!(
        String::from_utf8(json).expect("utf-8"),
        "{\"prog\":\"prog\",\"warning\":\"no \\\"remedy\\\" here\"}\n"
    );
}

#[test]
fn test_no_notices_say_nothing() {
    let none: [Note; 0] = [];
    for format in [Format::Text, Format::Json] {
        let mut err = Vec::new();
        say_notices(&none, "prog", format, &catalog(), &mut err);
        assert!(err.is_empty());
    }
}

#[test]
fn test_the_program_name_is_whatever_the_caller_gives() {
    let mut err = Vec::new();
    say_notices(
        &[Note("w", "r")],
        "taskloom",
        Format::Text,
        &catalog(),
        &mut err,
    );
    assert_eq!(
        String::from_utf8(err).expect("utf-8"),
        "taskloom: warning: w\n  fix: r\n"
    );
    let mut err = Vec::new();
    say_warning(
        &Note("w", ""),
        "taskloom",
        Format::Text,
        &catalog(),
        &mut err,
    );
    assert_eq!(String::from_utf8(err).expect("utf-8"), "taskloom: w\n");
}

#[test]
fn test_a_warning_under_json_is_exactly_the_envelope_the_catalog_writes() {
    let mut err = Vec::new();
    say_warning(
        &Note("it broke", ""),
        "ctxloom",
        Format::Json,
        &catalog(),
        &mut err,
    );
    let said = String::from_utf8(err).expect("utf-8");
    let envelope = catalog().envelope(&Note("it broke", ""));
    let mut expected = Vec::new();
    angzarr_presentation::write_json(&mut expected, &envelope).expect("written");
    assert_eq!(said.as_bytes(), expected.as_slice());
    assert!(said.ends_with('\n'), "{said:?}");
    assert!(said.matches('\n').count() > 1, "pretty printed: {said:?}");
}

/// A writer that refuses everything: a closed stderr.
struct Closed;

impl std::io::Write for Closed {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("closed"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_notices_and_warnings_never_fail_on_a_closed_stream() {
    for format in [Format::Text, Format::Json] {
        say_notices(&notices(), "p", format, &catalog(), &mut Closed);
        say_warning(&Note("x", ""), "p", format, &catalog(), &mut Closed);
    }
}

#[test]
fn test_the_remedy_of_a_notice_spells_the_args_of_the_notice() {
    let note = Note("a", "b");
    let fix = FixOf(&note, "note-fix".to_string());
    let args: Vec<String> = fix.args().iter().map(|(k, _)| (*k).to_string()).collect();
    assert_eq!(args, ["text", "remedy"]);
    assert_eq!(
        catalog().render(&fix).expect("a remedy"),
        "b",
        "the catalog's `note-fix` over the notice's args"
    );
}

#[test]
fn test_a_new_call_has_every_global_at_its_default() {
    let given = matches(&["t"]);
    let call = Call::new(&given, None, Format::Json);
    assert!(!call.stdout_is_terminal);
    assert!(!call.degraded);
    assert!(!call.no_companions);
    assert!(call.config_set.is_empty());
    assert_eq!(call.explicit_format, None);
    assert_eq!(call.default_format, Format::Json);
    assert!(std::ptr::eq(call.matches, &given));
}

#[test]
fn test_the_explicit_format_wins_whatever_the_default() {
    let given = matches(&["t"]);
    for (explicit, default) in [
        (Format::Text, Format::Json),
        (Format::Json, Format::Text),
        (Format::Text, Format::Text),
    ] {
        assert_eq!(
            Call::new(&given, Some(explicit), default).format(),
            explicit
        );
    }
}

#[test]
fn test_a_failure_keeps_any_exit_code_it_is_given() {
    // 1 failed, 2 refused before any state was read, 3 fatal findings
    for exit in [0, 1, 2, 3, 255] {
        assert_eq!(Failure::new(Note("x", ""), exit).exit, exit);
    }
}

#[test]
fn test_a_flag_is_false_for_a_value_argument_and_a_value_is_none_for_a_flag() {
    let given = matches(&["t", "--on", "--name", "x"]);
    assert!(!flag(&given, "name"), "a text argument is no flag");
    assert_eq!(one(&given, "on"), None, "a flag is no text");
    assert!(many(&given, "on").is_empty());
}

#[test]
fn test_one_of_a_repeated_argument_is_its_first_and_many_keeps_the_order() {
    let given = Command::new("t")
        .arg(ClapArg::new("tag").long("tag").action(ArgAction::Append))
        .get_matches_from(["t", "--tag", "b", "--tag", "a"]);
    assert_eq!(one(&given, "tag"), Some("b".to_string()));
    assert_eq!(many(&given, "tag"), ["b", "a"]);
}

#[derive(serde::Serialize)]
struct Doc {
    a: u8,
}

#[test]
fn test_emit_json_is_pretty_with_a_newline_and_never_runs_the_text() {
    let mut out = Vec::new();
    emit(Format::Json, &mut out, &Doc { a: 1 }, |_| panic!("text")).expect("written");
    assert_eq!(String::from_utf8(out).expect("utf-8"), "{\n  \"a\": 1\n}\n");
}

#[test]
fn test_emit_text_is_whatever_the_text_wrote_and_its_error_passes_through() {
    let mut out = Vec::new();
    let err = emit(Format::Text, &mut out, &Doc { a: 1 }, |_| {
        Err(std::io::Error::other("no"))
    })
    .expect_err("passed through");
    assert_eq!(err.to_string(), "no");
    assert!(out.is_empty(), "the value is not written in text");
}
