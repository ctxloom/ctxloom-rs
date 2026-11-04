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
