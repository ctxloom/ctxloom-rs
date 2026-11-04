//! The faults of the command line: their codes, args and words.

use super::*;

fn output() -> CliFault {
    CliFault::Output {
        ext_err_txt: "broken pipe".to_string(),
    }
}

fn no_such() -> CliFault {
    CliFault::NoSuchCommand {
        noun: "frob".to_string(),
    }
}

#[test]
fn each_fault_has_its_code_and_args() {
    assert_eq!(output().code(), "setup-cli-output");
    assert_eq!(no_such().code(), "setup-cli-no-such-command");
    assert_eq!(output().args().len(), 1);
    assert_eq!(output().args()[0].0, "ext_err_txt");
    assert_eq!(no_such().args().len(), 1);
    assert_eq!(no_such().args()[0].0, "noun");
}

#[test]
fn each_fault_displays_its_code_and_args_and_has_its_words_in_the_catalog() {
    assert_eq!(
        output().to_string(),
        "setup-cli-output ext_err_txt=\"broken pipe\""
    );
    assert_eq!(no_such().to_string(), "setup-cli-no-such-command noun=frob");
    let catalog = angzarr_presentation::Catalog::en_us(&[EN_US]).expect("a catalog");
    assert_eq!(
        catalog.message(&output()),
        "could not write the answer: broken pipe"
    );
    assert_eq!(catalog.message(&no_such()), "frob is not a setup command");
}

#[test]
fn the_samples_cover_every_fault_with_distinct_codes() {
    let codes: Vec<String> = diagnostic_samples()
        .iter()
        .map(|d| d.code().to_string())
        .collect();
    assert_eq!(codes, vec!["setup-cli-output", "setup-cli-no-such-command"]);
}
