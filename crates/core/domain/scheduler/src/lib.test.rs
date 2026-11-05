//! The rules that need no stream: the stream's root and text, the clock
//! fact's external id, and the catalog.

use angzarr_embedded::Timestamp;

use crate::{
    alarm_external_id, diagnostic_samples, rfc3339_nanos, stream_for, stream_text, Stream, EN_US,
    SCHEDULER_DOMAIN,
};

fn stream(domain: &str, edition: Option<&str>) -> Stream {
    Stream {
        domain: domain.to_string(),
        root: (1..=16).collect(),
        edition: edition.map(str::to_string),
    }
}

#[test]
fn the_domain_name_is_the_one_the_spec_gives() {
    assert_eq!(SCHEDULER_DOMAIN, "scheduler");
}

#[test]
fn the_stream_text_is_domain_slash_root_and_an_edition_after_an_at() {
    assert_eq!(
        stream_text(&stream("hold", None)),
        "hold/01020304-0506-0708-090a-0b0c0d0e0f10"
    );
    assert_eq!(
        stream_text(&stream("hold", Some("what-if"))),
        "hold/01020304-0506-0708-090a-0b0c0d0e0f10@what-if"
    );
    // An empty edition is the main timeline.
    assert_eq!(
        stream_text(&stream("hold", Some(""))),
        stream_text(&stream("hold", None))
    );
}

#[test]
fn a_root_that_is_not_sixteen_bytes_is_named_in_hex() {
    let short = Stream {
        domain: "hold".to_string(),
        root: vec![0xab, 0x01],
        edition: None,
    };
    assert_eq!(stream_text(&short), "hold/ab01");
}

#[test]
fn stream_for_is_compute_root_scheduler_of_the_stream_text() {
    let target = stream("hold", None);
    let expected = angzarr_embedded::ids::compute_root(
        "scheduler",
        "hold/01020304-0506-0708-090a-0b0c0d0e0f10",
    )
    .unwrap();
    assert_eq!(stream_for(&target), expected);
    // Golden: the stored scheduler streams are rooted by it, so it never changes.
    assert_eq!(
        stream_for(&target).to_string(),
        "c11699cf-e799-5ce9-966b-b2dab3913915"
    );
    assert_ne!(stream_for(&target), stream_for(&stream("hold", Some("e"))));
    assert_ne!(stream_for(&target), stream_for(&stream("other", None)));
}

#[test]
fn the_instant_is_rfc3339_utc_with_nine_digits_and_a_z() {
    assert_eq!(
        rfc3339_nanos(&Timestamp {
            seconds: 0,
            nanos: 0
        }),
        "1970-01-01T00:00:00.000000000Z"
    );
    assert_eq!(
        rfc3339_nanos(&Timestamp {
            seconds: 1_793_664_000 + 3_661,
            nanos: 5_000_000,
        }),
        "2026-11-03T01:01:01.005000000Z"
    );
}

#[test]
fn alarm_due_external_id_is_alarm_stream_slash_id_at_rfc3339_nanos_z() {
    let at = Timestamp {
        seconds: 1_793_664_000,
        nanos: 123,
    };
    assert_eq!(
        alarm_external_id(&stream("hold", None), "deadline", &at),
        "alarm:hold/01020304-0506-0708-090a-0b0c0d0e0f10/deadline@2026-11-03T00:00:00.000000123Z"
    );
    assert_eq!(
        alarm_external_id(&stream("hold", Some("what-if")), "deadline", &at),
        "alarm:hold/01020304-0506-0708-090a-0b0c0d0e0f10@what-if/deadline@2026-11-03T00:00:00.000000123Z"
    );
    // An alarm id holding '@' and '/' is still unambiguous: the instant is fixed width.
    assert_eq!(
        alarm_external_id(&stream("hold", None), "a@b/c", &at),
        "alarm:hold/01020304-0506-0708-090a-0b0c0d0e0f10/a@b/c@2026-11-03T00:00:00.000000123Z"
    );
}

// The crate's catalog test (the binary's holds the same for the whole graph).
#[test]
fn every_diagnostic_has_a_message_that_uses_exactly_its_args() {
    let (name, catalog) = EN_US;
    assert_eq!(name, "scheduler");
    for sample in diagnostic_samples() {
        let code = sample.code();
        assert!(code.starts_with("scheduler-"), "{code} is prefixed");
        let line = catalog
            .lines()
            .find(|l| l.starts_with(&format!("{code} =")))
            .unwrap_or_else(|| panic!("{code} has no message"));
        let mut args: Vec<&str> = sample.args().into_iter().map(|(n, _)| n).collect();
        args.sort_unstable();
        let mut used: Vec<&str> = line
            .split('$')
            .skip(1)
            .map(|rest| {
                rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                    .next()
                    .unwrap()
            })
            .collect();
        used.sort_unstable();
        used.dedup();
        assert_eq!(args, used, "{code} uses exactly its args");
    }
    // And the other way: every message in the catalog is a sample's.
    let codes: Vec<String> = diagnostic_samples()
        .iter()
        .map(|s| s.code().to_string())
        .collect();
    for line in catalog
        .lines()
        .filter(|l| l.contains(" = ") && !l.starts_with('#'))
    {
        let id = line.split(" = ").next().unwrap();
        assert!(codes.iter().any(|c| c == id), "{id} has no sample");
    }
}

#[test]
fn a_cover_is_named_as_its_stream_is() {
    use angzarr_embedded::v1::{Cover, Uuid};
    let cover = Cover {
        domain: "hold".to_string(),
        root: Some(Uuid::from_bytes([1; 16])),
        edition: Some("e".to_string()),
    };
    assert_eq!(
        crate::cover_text(&cover),
        "hold/01010101-0101-0101-0101-010101010101@e"
    );
    let no_root = Cover {
        root: None,
        edition: None,
        ..cover
    };
    assert_eq!(crate::cover_text(&no_root), "hold/");
}

#[test]
fn rejections_display_their_code_and_args() {
    use crate::SchedulerRejection;
    assert_eq!(
        SchedulerRejection::AlarmIdEmpty.to_string(),
        "scheduler-alarm-id-empty"
    );
    assert_eq!(
        SchedulerRejection::TargetMismatch {
            stream: "a".into(),
            target: "b".into()
        }
        .to_string(),
        "scheduler-target-mismatch stream=a target=b"
    );
}
