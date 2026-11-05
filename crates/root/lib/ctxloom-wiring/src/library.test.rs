//! The library's nouns as the command line mounts them.

use std::sync::Arc;

use ctxloom_cli::Extension;
use process_env::EnvSnapshot;

use super::LibraryCommands;

#[test]
fn the_nouns_are_the_library_clis() {
    let commands = LibraryCommands::new(Arc::new(EnvSnapshot::from_process()));
    let ours: Vec<String> = commands
        .nouns()
        .iter()
        .map(|n| n.get_name().to_string())
        .collect();
    let theirs: Vec<String> = library_cli::nouns()
        .iter()
        .map(|n| n.get_name().to_string())
        .collect();
    assert!(!theirs.is_empty());
    assert_eq!(ours, theirs);
}
