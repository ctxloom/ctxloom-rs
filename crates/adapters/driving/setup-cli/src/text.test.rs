//! The text renderings. Go: `renderCachePlan`, `renderDoctorSummary` (release/0.7).

use doctor_checks::{Check, Report, Status};
use reclaim_application::{CleanReport, CleanTarget};

use crate::text::{clean, doctor, human};

fn render(f: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>) -> String {
    let mut out = Vec::new();
    f(&mut out).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn sizes_read_as_a_person_does() {
    assert_eq!(human(512), "512 B");
    assert_eq!(human(2048), "2.0 KB");
    assert_eq!(human(5 * 1024 * 1024), "5.0 MB");
}

fn target(present: bool, bytes: u64) -> CleanTarget {
    CleanTarget {
        rel: ".ctxloom/cache/context".to_string(),
        path: String::new(),
        present,
        bytes,
        rebuild: "ctxloom run".to_string(),
        removed: false,
    }
}

// clean.feature: the report names the path, and what rebuilds it.
#[test]
fn the_plan_names_each_present_path_and_its_rebuild() {
    let report = CleanReport {
        applied: false,
        targets: vec![target(true, 2048), target(false, 0)],
        bytes: 2048,
    };
    let text = render(|o| clean(&report, o));
    assert!(text.contains("would remove 2.0 KB"));
    assert!(text.contains(".ctxloom/cache/context") && text.contains("rebuild: ctxloom run"));
    let applied = CleanReport {
        applied: true,
        ..report
    };
    assert!(render(|o| clean(&applied, o)).contains("ctxloom removed"));
}

#[test]
fn nothing_present_says_so() {
    let report = CleanReport {
        applied: false,
        targets: vec![target(false, 0)],
        bytes: 0,
    };
    assert!(render(|o| clean(&report, o)).starts_with("Nothing to clean"));
}

fn row(status: Status, remedy: &str) -> Check {
    Check {
        marker: "DOCTOR-CHECK-X-a1",
        name: "x",
        status,
        detail: "detail".to_string(),
        remedy: remedy.to_string(),
    }
}

#[test]
fn the_report_lists_warnings_and_counts_them_with_the_first_fix() {
    let report = Report {
        checks: vec![row(Status::Ok, ""), row(Status::Warn, "do this")],
    };
    let text = render(|o| doctor(&report, false, o));
    assert!(text.contains("[warn]") && !text.contains("[ok]"));
    assert!(text.contains("1 warning(s); first fix: do this"));
    assert!(render(|o| doctor(&report, true, o)).contains("[ok]"));
}

#[test]
fn a_clean_report_says_so_and_a_failure_is_counted_first() {
    let clean_report = Report {
        checks: vec![row(Status::Ok, "")],
    };
    assert!(render(|o| doctor(&clean_report, false, o)).contains("No warnings."));
    let failing = Report {
        checks: vec![row(Status::Fail, "fix it"), row(Status::Warn, "")],
    };
    assert!(render(|o| doctor(&failing, false, o))
        .contains("1 failure(s), 1 warning(s); first fix: fix it"));
}
