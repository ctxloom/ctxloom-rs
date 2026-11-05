//! Which purge age a sweep uses: the flag, else the configuration.

use ctxloom_ports::Failure;
use sessions_application::Timestamp;

use super::*;
use crate::ReclaimError;

const NOW: Timestamp = Timestamp {
    seconds: 1_800_000_000,
    nanos: 7,
};
const DAY: i64 = 86_400;

/// A configuration that says what it is given, and counts being asked.
struct Config {
    age: Result<Option<&'static str>, ()>,
    asked: usize,
}

impl Config {
    fn says(age: &'static str) -> Config {
        Config {
            age: Ok(Some(age)),
            asked: 0,
        }
    }
}

impl SweepConfig for Config {
    fn purge_age(&mut self) -> Result<Option<String>, Failure> {
        self.asked += 1;
        match self.age {
            Ok(age) => Ok(age.map(str::to_string)),
            Err(()) => Err(Box::new(ReclaimError::BadAge {
                flag: "config".into(),
                value: "unreadable".into(),
            })),
        }
    }
}

// The sweep's purge-age rule: a purge age is `--purge-older-than`, else
// `session_purge_age`; the flag beats the configuration.
#[test]
fn the_flag_beats_the_configured_age_and_the_configuration_is_not_asked() {
    let mut config = Config::says("90d");
    let cutoff = purge_cutoff(Some("30d"), &mut config, NOW)
        .unwrap()
        .unwrap();
    assert_eq!(cutoff.seconds, NOW.seconds - 30 * DAY);
    assert_eq!(config.asked, 0);
}

#[test]
fn without_the_flag_the_configured_age_is_the_purge_age() {
    let mut config = Config::says("90d");
    let cutoff = purge_cutoff(None, &mut config, NOW).unwrap().unwrap();
    assert_eq!(cutoff.seconds, NOW.seconds - 90 * DAY);
    assert_eq!(config.asked, 1);
}

#[test]
fn with_neither_there_is_no_purge_age() {
    let mut config = Config {
        age: Ok(None),
        asked: 0,
    };
    assert_eq!(purge_cutoff(None, &mut config, NOW).unwrap(), None);
}

#[test]
fn a_blank_configured_age_is_no_purge_age() {
    for blank in ["", "  "] {
        let mut config = Config::says(blank);
        assert_eq!(
            purge_cutoff(None, &mut config, NOW).unwrap(),
            None,
            "{blank:?}"
        );
    }
}

#[test]
fn a_configured_age_that_is_none_is_refused_naming_the_key() {
    let mut config = Config::says("soon");
    match purge_cutoff(None, &mut config, NOW) {
        Err(ReclaimError::BadAge { flag, value }) => {
            assert_eq!(
                (flag.as_str(), value.as_str()),
                ("session_purge_age", "soon")
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_flag_that_is_none_is_refused_naming_the_flag_whatever_the_configuration_says() {
    let mut config = Config::says("90d");
    match purge_cutoff(Some("soon"), &mut config, NOW) {
        Err(ReclaimError::BadAge { flag, .. }) => assert_eq!(flag, "--purge-older-than"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_configuration_that_cannot_be_read_fails_the_sweep() {
    let mut config = Config {
        age: Err(()),
        asked: 0,
    };
    let error = purge_cutoff(None, &mut config, NOW).unwrap_err();
    assert!(matches!(error, ReclaimError::Config(_)), "{error:?}");
    assert_eq!(error.exit_code(), 1);
}

#[test]
fn a_configuration_that_cannot_be_read_does_not_matter_when_the_flag_is_given() {
    let mut config = Config {
        age: Err(()),
        asked: 0,
    };
    assert!(purge_cutoff(Some("30d"), &mut config, NOW).is_ok());
}
