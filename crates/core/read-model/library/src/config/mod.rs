//! The layered configuration (Go's `internal/adapters/configload`,
//! `internal/shared/confload` and `internal/core/config`), as pure values:
//! the adapter reads the files and the process hands the environment and the
//! flags in.
//!
//! Precedence, lowest first: the home's `config.yaml`, the project's,
//! `CTXLOOM_CONFIG_<PATH>` variables, `--config-set path=value`. Every file is
//! a mapping stamped `schema_version` (this build reads [`OLDEST`] to
//! [`CURRENT`]; an older generation is migrated in memory, nothing is written
//! back). Every value is its text: the reader keeps no types, and the
//! [`schema`] says what each key holds.

pub mod layer;
pub mod overrides;
pub mod schema;
pub mod typed;

/// The generation this build reads and writes.
pub const CURRENT: u32 = 8;

/// The oldest generation it migrates.
pub const OLDEST: u32 = 7;

use angzarr_yaml::Value;

/// `layer` laid over `into`: mappings merge key by key, anything else the
/// layer has replaces (a list replaces a list; koanf's default).
pub(crate) fn deep_merge(into: &mut Value, layer: &Value) {
    match (into, layer) {
        (Value::Map(mine), Value::Map(theirs)) => {
            for (key, value) in theirs {
                match mine.iter_mut().find(|(k, _)| k == key) {
                    Some((_, existing)) => deep_merge(existing, value),
                    None => mine.push((key.clone(), value.clone())),
                }
            }
        }
        (me, other) => *me = other.clone(),
    }
}
