//! The delivery policy: `AlarmFired` on a scheduler stream becomes
//! `AlarmReached` on the target's. One policy per consuming domain, declared
//! with [`delivery!`](crate::delivery), because a policy has a single target
//! type and a const NAME that is its identity.

use angzarr_diagnostic::Report;
use angzarr_embedded::v1::Uuid;
use angzarr_embedded::{Any, Payload};

use crate::{AlarmReached, SchedulerEvent};

/// The fact owed `domain` for `event`, a scheduler event, and the root of the
/// stream it is for: `AlarmReached` for an `AlarmFired` whose target is in
/// `domain`; `None` for any other event, or another domain's firing.
pub fn delivered(event: &Any, domain: &str) -> Result<Option<(Uuid, AlarmReached)>, Report> {
    let SchedulerEvent::AlarmFired(fired) =
        SchedulerEvent::from_any(event).map_err(|e| Report::of(&e))?
    else {
        return Ok(None);
    };
    let Ok(root) = <[u8; 16]>::try_from(fired.target.root.as_slice()) else {
        return Ok(None);
    };
    let reached = AlarmReached {
        alarm_id: fired.alarm_id,
        at: fired.at,
    };
    Ok((fired.target.domain == domain).then(|| (Uuid::from_bytes(root), reached)))
}

/// Declare the delivery policy for `$target`, a domain that ingests
/// [`AlarmReached`](crate::AlarmReached) (`#[handles_fact(AlarmReached)]`):
/// a unit struct named `$policy`, to register with `Registry::policy`. Its
/// NAME is `<the invoking crate's package>::<$target>::alarms`, so it is the
/// consumer's and does not change when the scheduler is lifted.
#[macro_export]
macro_rules! delivery {
    ($policy:ident, $target:ty) => {
        pub struct $policy;

        impl $crate::__private::Policy for $policy {
            const NAME: &'static str = concat!(
                env!("CARGO_PKG_NAME"),
                "::",
                stringify!($target),
                "::alarms"
            );
            type Source = $crate::SchedulerAggregate;
            type Target = $target;
            const EVENTS: &'static [$crate::__private::TypeName] =
                &[$crate::__private::TypeName::of::<
                    $crate::__private::AlarmFired,
                >()];

            fn on_event(
                &self,
                event: &$crate::__private::Any,
                _ctx: &$crate::__private::PolicyContext,
            ) -> Result<Option<$crate::__private::Commands<$target>>, $crate::__private::Report>
            {
                Ok(
                    $crate::delivered(
                        event,
                        <$target as $crate::__private::CommandTarget>::DOMAIN,
                    )?
                    .map(|(root, reached)| $crate::__private::Commands::none().fact(root, reached)),
                )
            }
        }
    };
}

#[cfg(test)]
#[path = "delivery.test.rs"]
mod tests;
