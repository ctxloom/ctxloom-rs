//! A warning said beside an answer, in either format.

use deps_application::Notice;

use super::*;

fn said(format: Format, notice: &Notice) -> String {
    let catalog = Catalog::en_us(&[deps_application::EN_US]).expect("a catalog");
    let mut err = Vec::new();
    say_warning(notice, format, &catalog, &mut err);
    String::from_utf8(err).expect("utf-8")
}

#[test]
fn a_text_warning_is_the_programs_name_and_the_message() {
    let text = said(Format::Text, &Notice::text("it broke"));
    assert_eq!(text, "ctxloom: it broke\n");
}

#[test]
fn a_json_warning_is_the_envelope() {
    let json = said(Format::Json, &Notice::text("it broke"));
    assert!(json.contains("\"message\": \"it broke\""), "{json}");
    assert!(
        json.contains("\"code\": \"deps-application-notice\""),
        "{json}"
    );
    assert!(!json.contains("ctxloom:"), "{json}");
}
