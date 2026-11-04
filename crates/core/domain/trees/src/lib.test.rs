use crate::{diagnostic_samples, EN_US, TREE_DOMAIN};

#[test]
fn the_domain_name_is_the_one_the_plan_gives() {
    assert_eq!(TREE_DOMAIN, "trees");
}

#[test]
fn every_diagnostic_has_a_message_and_its_code_is_prefixed() {
    let (name, catalog) = EN_US;
    assert_eq!(name, "trees");
    for sample in diagnostic_samples() {
        let code = sample.code();
        assert!(code.starts_with("trees-"), "{code} is prefixed");
        assert!(
            catalog.contains(&format!("{code} =")),
            "{code} has no message"
        );
    }
}
