//! The hook verbs, whose stdout is the engine's input.

use angzarr_presentation::Format;

use super::*;
use crate::tests::{Fake, NoSessions, Wire};

// A hook's stdout is the engine's input: the statusline hook answers with the
// bare name when the payload is not a status document.
#[test]
fn test_the_hud_hook_writes_its_statusline_to_stdout() {
    let hook = hook_cli::Hook {
        engine: None,
        verb: hook_cli::HookVerb::Hud,
    };
    let catalog = Catalog::en_us(&[crate::EN_US]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut services = Wire(Fake { answer: Ok(0) }, NoSessions);
    hook_verb(
        &hook,
        "not json",
        Format::Text,
        &mut services,
        &catalog,
        &mut out,
        &mut err,
    )
    .expect("answers");
    assert_eq!(String::from_utf8(out).unwrap(), "ctxloom");
    assert!(err.is_empty());
}
