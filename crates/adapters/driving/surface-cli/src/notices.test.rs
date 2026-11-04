//! What a command noticed, said as a json line or as words with a fix.

use angzarr_presentation::Catalog;

use super::*;

fn catalog() -> Catalog {
    Catalog::en_us(&[library_application::EN_US]).expect("a catalog")
}

fn said(format: Format, notices: &[Notice]) -> String {
    let mut err = Vec::new();
    say(notices, "ctxloom", format, &catalog(), &mut err);
    String::from_utf8(err).expect("utf-8")
}

fn broken() -> Notice {
    Notice::Finding {
        text: "it broke".to_string(),
        remedy: "mend it".to_string(),
    }
}

#[test]
fn the_remedy_of_a_diagnostic_is_its_code_with_fix_and_its_args() {
    let notice = broken();
    let fix = FixOf(
        &notice,
        "library-application-notice-finding-fix".to_string(),
    );
    assert_eq!(fix.code(), "library-application-notice-finding-fix");
    assert_eq!(fix.args().len(), 2);
    assert_eq!(
        format!("{fix:?}"),
        "FixOf(library-application-notice-finding-fix)"
    );
    assert!(
        fix.to_string()
            .starts_with("library-application-notice-finding-fix"),
        "{fix}"
    );
}

#[test]
fn a_warning_is_a_line_of_words_with_its_fix() {
    let notices = [broken(), Notice::text("only words")];
    assert_eq!(
        said(Format::Text, &notices),
        "ctxloom: warning: it broke\n  fix: mend it\nctxloom: warning: only words\n"
    );
}

#[test]
fn a_warning_is_a_json_line_with_its_remedy_when_it_has_one() {
    let notices = [broken(), Notice::text("only words")];
    assert_eq!(
        said(Format::Json, &notices),
        "{\"prog\":\"ctxloom\",\"warning\":\"it broke\",\"remedy\":\"mend it\"}\n{\"prog\":\"ctxloom\",\"warning\":\"only words\"}\n"
    );
}
