//! The sweep's purge age: `--purge-older-than`, else `session_purge_age`.

use ctxloom_ports::Failure;
use sessions_application::Timestamp;

use crate::{parse_age, ReclaimError};

/// What the configuration says about reclaiming.
pub trait SweepConfig {
    /// `session_purge_age` as configured; none where it is unset. A
    /// configuration that cannot be read is an error and stops the sweep
    /// (Go warned and held every purge; nothing is acted on either way).
    fn purge_age(&mut self) -> Result<Option<String>, Failure>;
}

/// The instant sessions last active before are purged: the flag's age or
/// date when given (and then the configuration is not read, so a broken one
/// cannot stop a sweep that does not need it), else the configured one; none
/// where neither says (purges have no default).
pub fn purge_cutoff(
    flag: Option<&str>,
    config: &mut dyn SweepConfig,
    now: Timestamp,
) -> Result<Option<Timestamp>, ReclaimError> {
    if let Some(age) = flag {
        return parse_age("--purge-older-than", age, now).map(Some);
    }
    let configured = config.purge_age().map_err(ReclaimError::Config)?;
    configured
        .as_deref()
        .map(str::trim)
        .filter(|age| !age.is_empty())
        .map(|age| parse_age("session_purge_age", age, now))
        .transpose()
}

#[cfg(test)]
#[path = "purge.test.rs"]
mod tests;
