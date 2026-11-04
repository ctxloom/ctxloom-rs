//! The edge adapter's pure conversions. RED (S5): stubs.

use angzarr_embedded::{Failure, KernelError, Malformed};
use run_ports::Arrival;

use crate::{host_arrival, unserved};

// NEW-DESIGN (review question 6: the core's own Arrival, converted here).
#[test]
fn the_cores_arrival_is_the_hosts() {
    assert_eq!(
        host_arrival(Arrival::Loopback),
        angzarr_embedded_host::Arrival::Loopback
    );
    assert_eq!(
        host_arrival(Arrival::Ip),
        angzarr_embedded_host::Arrival::Ip
    );
}

// NEW-DESIGN: a hidden domain keeps its real name for the host's log, and its
// answer to the caller is the kernel's unknown domain.
#[test]
fn a_hidden_domain_is_hidden_and_answered_as_unknown() {
    let hidden = angzarr_embedded_host::Unserved::Hidden {
        answer: Failure::Kernel(KernelError::Malformed(Malformed::UnknownDomain {
            domain: "runs".to_string(),
        })),
        hidden: angzarr_embedded_host::NotExposed {
            domain: "runs".to_string(),
            arrival: angzarr_embedded_host::Arrival::Ip,
            expose: angzarr_embedded_host::config::Expose::None,
        },
    };
    let converted = unserved(hidden);
    assert_eq!(
        converted,
        run_ports::Unserved::Hidden {
            domain: "runs".to_string()
        }
    );
    assert_eq!(
        converted.answer(),
        Failure::Kernel(KernelError::Malformed(Malformed::UnknownDomain {
            domain: "runs".to_string()
        }))
    );
}

// NEW-DESIGN: the kernel's own failure is passed as it is.
#[test]
fn a_kernel_failure_is_passed_as_it_is() {
    let failure = Failure::Kernel(KernelError::ChainTooDeep { limit: 8 });
    let converted = unserved(angzarr_embedded_host::Unserved::Failed(failure.clone()));
    assert_eq!(converted, run_ports::Unserved::Failed(failure));
}
