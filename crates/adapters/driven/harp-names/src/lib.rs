//! Names from harp-core: adjectives and a noun, joined by '-'. harp-core's
//! one word list is the one Go's harp calls its "long" group (1269
//! adjectives, 4396 nouns), so a task id is Go's `GenerateShortName`.

use harp_core::{generate_name_with_options, NameOptions};
use ports::Names;

/// [`Names`] drawn at random from harp-core's word lists.
#[derive(Clone, Copy, Debug, Default)]
pub struct HarpNames;

/// A random name of `components` words: adjectives, then a noun.
fn name(components: u8) -> String {
    generate_name_with_options(&NameOptions {
        components,
        max_element_length: None,
        separator: "-".to_string(),
    })
}

impl Names for HarpNames {
    /// Adjective-noun ("swift-amber").
    fn task_id(&mut self) -> String {
        name(2)
    }

    /// Adjective-adjective-noun ("jolly-mute-magma").
    fn project_id(&mut self) -> String {
        name(3)
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
