//! Messages from catalogs, and the English when there is none.

use annals_diagnostic::Report;

use super::Catalog;
use crate::Unrendered;

const DEMO: crate::Source = (
    "demo",
    "demo-gave-up = gave up after { $attempts } attempts: { $error }\n\
     demo-busy = the store { $path } is busy\n\
     demo-events = { $n ->\n    [one] one event\n   *[other] { $n } events\n}\n",
);

fn catalog() -> Catalog {
    Catalog::en_us(&[DEMO]).expect("the demo catalog loads")
}

fn busy() -> Report {
    Report::new("demo-busy", "English for busy").arg("path", "/db")
}

#[test]
fn test_a_message_is_the_catalogs_filled_from_the_args_without_isolation_marks() {
    assert_eq!(
        catalog().render(&busy()),
        Ok("the store /db is busy".to_string())
    );
}

#[test]
fn test_a_nested_cause_is_rendered_from_the_catalog_too() {
    let gave_up = Report::new("demo-gave-up", "English")
        .arg("attempts", 10u32)
        .arg("error", annals_diagnostic::Arg::nested(&busy()));
    assert_eq!(
        catalog().message(&gave_up),
        "gave up after 10 attempts: the store /db is busy"
    );
}

#[test]
fn test_a_nested_cause_no_catalog_has_falls_back_to_its_english() {
    let gave_up = Report::new("demo-gave-up", "English")
        .arg("attempts", 10u32)
        .arg(
            "error",
            annals_diagnostic::Arg::nested(&Report::new("annals-store-sql-unavailable", "down")),
        );
    assert_eq!(
        catalog().render(&gave_up),
        Ok("gave up after 10 attempts: down".to_string())
    );
}

#[test]
fn test_an_unknown_code_is_unrendered_and_its_message_is_its_english() {
    let unknown = Report::new("demo-nothing", "the English");
    assert_eq!(
        catalog().render(&unknown),
        Err(Unrendered::Unknown {
            code: "demo-nothing".to_string()
        })
    );
    assert_eq!(catalog().message(&unknown), "the English");
}

#[test]
fn test_a_missing_arg_is_unformattable_and_its_message_is_its_english() {
    let bare = Report::new("demo-busy", "the English");
    match catalog().render(&bare) {
        Err(Unrendered::Unformattable { code, detail }) => {
            assert_eq!(code, "demo-busy");
            assert!(detail.contains("path"), "{detail}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(catalog().message(&bare), "the English");
}

fn events(n: impl Into<annals_diagnostic::Arg>) -> String {
    let report = Report::new("demo-events", "English").arg("n", n);
    catalog().render(&report).unwrap()
}

#[test]
fn test_numbers_select_plurals() {
    assert_eq!(events(1i64), "one event");
    assert_eq!(events(1u64), "one event");
    assert_eq!(events(3u64), "3 events");
    assert_eq!(events(-3i64), "-3 events");
}

#[test]
fn test_a_number_too_large_for_a_float_is_shown_exactly() {
    let big = (1u64 << 60) + 1;
    assert_eq!(events(big), "1152921504606846977 events");
    assert_eq!(events(-(big as i64)), "-1152921504606846977 events");
}

#[test]
fn test_a_catalog_that_does_not_parse_is_refused_by_name() {
    let error = Catalog::en_us(&[DEMO, ("broken", "broken-x = { $")])
        .err()
        .expect("refused");
    assert_eq!(error.source, "broken");
    assert!(!error.detail.is_empty());
}

#[test]
fn test_a_message_two_catalogs_hold_is_refused() {
    let error = Catalog::en_us(&[DEMO, ("again", "demo-busy = again\n")])
        .err()
        .expect("refused");
    assert_eq!(error.source, "again");
    assert!(error.detail.contains("demo-busy"), "{}", error.detail);
}

#[test]
fn test_a_catalog_can_be_shared_between_threads() {
    fn shared<T: Send + Sync>(_: &T) {}
    shared(&catalog());
}
