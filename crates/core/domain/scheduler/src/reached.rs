//! `AlarmReached`, the time fact a target ingests. Written by hand: the
//! markup layer generates domain types only for the messages a component
//! declares, and this one belongs to no scheduler component but to every
//! target's (`#[handles_fact(AlarmReached)]`).

use angzarr_embedded::markup::DomainMessage;
use angzarr_embedded::{Timestamp, WireError};
use scheduler_proto::v1 as wire;

/// `at` is reached for the alarm the target armed as `alarm_id`. `at` is the
/// instant armed, not the clock's reading when it fired.
#[derive(Clone, Debug, PartialEq)]
pub struct AlarmReached {
    pub alarm_id: String,
    pub at: Timestamp,
}

impl DomainMessage for AlarmReached {
    type Wire = wire::AlarmReached;

    fn to_wire(&self) -> wire::AlarmReached {
        wire::AlarmReached {
            alarm_id: self.alarm_id.clone(),
            at: Some(self.at),
        }
    }

    fn from_wire(wire: wire::AlarmReached) -> Result<Self, WireError> {
        Ok(AlarmReached {
            alarm_id: wire.alarm_id,
            at: wire.at.ok_or(WireError::Missing {
                message: "io.angzarr.scheduler.v1.AlarmReached",
                field: "at",
            })?,
        })
    }
}

#[cfg(test)]
#[path = "reached.test.rs"]
mod tests;
