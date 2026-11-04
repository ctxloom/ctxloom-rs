//! What a command noticed, said as a json line or as words with a fix.

use angzarr_presentation::Catalog;

use super::*;

fn catalog() -> Catalog {
    Catalog::en_us(&[deps_application::EN_US]).expect("a catalog")
}

fn said(format: Format, notices: &[Notice]) -> String {
    let mut err = Vec::new();
    say(notices, "ctxloom", format, &catalog(), &mut err);
    String::from_utf8(err).expect("utf-8")
}

#[test]
fn test_the_remedy_of_a_diagnostic_is_its_code_with_fix_and_its_args() {
    let notice = Notice::with_fix("words", "do this");
    let fix = FixOf(&notice, "deps-application-notice-fix".to_string());
    assert_eq!(fix.code(), "deps-application-notice-fix");
    assert_eq!(fix.args().len(), 2);
    assert_eq!(format!("{fix:?}"), "FixOf(deps-application-notice-fix)");
    assert!(
        fix.to_string().starts_with("deps-application-notice-fix"),
        "{fix}"
    );
}

#[test]
fn test_a_warning_is_a_line_of_words_with_its_fix() {
    let notices = [
        Notice::with_fix("it broke", "mend it"),
        Notice::text("only words"),
    ];
    assert_eq!(
        said(Format::Text, &notices),
        "ctxloom: warning: it broke\n  fix: mend it\nctxloom: warning: only words\n"
    );
}

#[test]
fn test_a_warning_is_a_json_line_with_its_remedy_when_it_has_one() {
    let notices = [
        Notice::with_fix("it broke", "mend it"),
        Notice::text("only words"),
    ];
    assert_eq!(
        said(Format::Json, &notices),
        "{\"prog\":\"ctxloom\",\"warning\":\"it broke\",\"remedy\":\"mend it\"}\n{\"prog\":\"ctxloom\",\"warning\":\"only words\"}\n"
    );
}
