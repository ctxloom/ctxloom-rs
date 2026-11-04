//! `bundle push` and `bundle move` over the fakes.

use crate::fakes::Fixture;
use crate::{AppError, MoveRequest, Outcome, PushRequest};

const URL: &str = "file:///srv/team.git";

fn ok<T>(outcome: Outcome<T>) -> T {
    outcome.result.unwrap_or_else(|e| panic!("{e:?}"))
}

fn authored() -> Fixture {
    let fx = Fixture::new();
    fx.files.author(
        "authored",
        &[
            ("bundle.yaml", "description: acceptance fixture bundle\n"),
            ("fragments/example.md", "# Example Fragment\n"),
        ],
    );
    fx
}

fn push(bundle: &str, remote: &str) -> PushRequest {
    PushRequest {
        bundle: bundle.to_string(),
        remote: remote.to_string(),
        ..PushRequest::default()
    }
}

// bundle.feature: Pushing a bundle to a plain git remote lands its tree there
#[test]
fn test_a_push_lands_the_whole_tree_as_one_commit_beside_what_was_there() {
    let fx = authored();
    let before = fx.origin("team", URL);
    let pushed = ok(fx.deps.push(&push("authored", "team")));
    assert_eq!(pushed.status, "pushed");
    assert_eq!(pushed.remote, "team");
    assert_eq!(pushed.target_path, ".ctxloom/content/bundles/v2/authored");
    assert_eq!(pushed.title, "Update bundle: authored");
    assert!(!pushed.create_pr);
    assert_eq!(pushed.size_bytes, 58);
    assert_eq!(
        pushed.path,
        "/p/.ctxloom/content/bundles/v2/authored/bundle.yaml"
    );
    assert_ne!(pushed.commit_sha, before);
    assert_eq!(pushed.commit_sha, fx.remotes.head(URL));
    let held = |path: &str| {
        fx.remotes
            .file(URL, &format!(".ctxloom/content/bundles/v2/{path}"))
    };
    assert_eq!(
        held("authored/bundle.yaml").as_deref(),
        Some("description: acceptance fixture bundle\n")
    );
    assert_eq!(
        held("authored/fragments/example.md").as_deref(),
        Some("# Example Fragment\n")
    );
    assert!(
        held("demo/fragments/demo-frag.md").is_some(),
        "a publish adds to the history"
    );
    assert!(fx.files.has_authored("authored"), "a push keeps the source");
}

#[test]
fn test_a_push_with_a_message_and_to_the_default_remote() {
    let fx = authored();
    fx.origin("team", URL);
    ok(fx.deps.remote_default(Some("team")));
    let pushed = ok(fx.deps.push(&PushRequest {
        message: "ship it".to_string(),
        ..push("authored", "")
    }));
    assert_eq!(
        (pushed.title.as_str(), pushed.remote.as_str()),
        ("ship it", "team")
    );
}

#[test]
fn test_a_push_that_changes_nothing_reports_the_commit_already_at_the_tip() {
    let fx = authored();
    fx.origin("team", URL);
    let first = ok(fx.deps.push(&push("authored", "team")));
    let second = ok(fx.deps.push(&push("authored", "team")));
    assert_eq!(first.commit_sha, second.commit_sha);
}

#[test]
fn test_a_push_that_cannot_go_is_refused_naming_why() {
    let fx = authored();
    fx.origin("team", URL);
    let refuses = |request: PushRequest| fx.deps.push(&request).result.expect_err("refused");
    assert!(matches!(
        refuses(PushRequest {
            pr: true,
            ..push("authored", "team")
        }),
        AppError::PullRequestNeedsForge
    ));
    assert!(matches!(
        refuses(push("authored", "")),
        AppError::Required { .. }
    ));
    assert!(matches!(
        refuses(push("authored", "nope")),
        AppError::Cause(_)
    ));
    assert!(matches!(
        refuses(push("missing", "team")),
        AppError::BundleNotFound { .. }
    ));
    fx.remotes.set_unreachable(URL, true);
    match refuses(push("authored", "team")) {
        AppError::Publish { remote, detail } => {
            assert_eq!(remote, "team");
            assert!(detail.contains("could not read"));
        }
        other => panic!("{other:?}"),
    }
}

// bundle.feature: Moving a bundle into another project lands it in that
// project's content tree
#[test]
fn test_a_move_to_a_directory_lands_the_bundle_there_and_removes_the_source() {
    let fx = authored();
    fx.files.directory("other-project");
    let moved = ok(fx.deps.move_bundle(&MoveRequest {
        name: "authored".to_string(),
        to: "other-project".to_string(),
        ..MoveRequest::default()
    }));
    assert_eq!((moved.status, moved.dest_kind), ("moved", "path"));
    assert_eq!(
        moved.dest,
        "other-project/.ctxloom/content/bundles/v2/authored"
    );
    assert_eq!(
        moved.source,
        "/p/.ctxloom/content/bundles/v2/authored/bundle.yaml"
    );
    assert_eq!((moved.remote.as_str(), moved.commit_sha.as_str()), ("", ""));
    let placed = fx.files.placed(&moved.dest).expect("placed");
    assert_eq!(placed["fragments/example.md"], b"# Example Fragment\n");
    assert!(!fx.files.has_authored("authored"));
}

#[test]
fn test_a_move_to_a_remote_publishes_first_then_removes_the_source() {
    let fx = authored();
    fx.origin("team", URL);
    let moved = ok(fx.deps.move_bundle(&MoveRequest {
        name: "authored".to_string(),
        to: "team".to_string(),
        message: "relocate".to_string(),
        ..MoveRequest::default()
    }));
    assert_eq!(moved.dest_kind, "remote");
    assert_eq!(moved.remote, "team");
    assert_eq!(moved.dest, ".ctxloom/content/bundles/v2/authored");
    assert_eq!(moved.commit_sha, fx.remotes.head(URL));
    assert!(!fx.files.has_authored("authored"));
}

// bundle.feature: A destination that is neither a remote nor a directory is
// refused, and the source stays
#[test]
fn test_a_move_that_cannot_land_leaves_the_source_alone() {
    let fx = authored();
    let moves = |to: &str, force: bool| {
        fx.deps
            .move_bundle(&MoveRequest {
                name: "authored".to_string(),
                to: to.to_string(),
                force,
                ..MoveRequest::default()
            })
            .result
            .expect_err("refused")
    };
    match moves("nowhere", false) {
        AppError::BadDestination { to, known } => {
            assert_eq!(
                (to.as_str(), known.as_str()),
                ("nowhere", " (no remotes configured)")
            );
        }
        other => panic!("{other:?}"),
    }
    fx.origin("team", URL);
    fx.origin("other", "file:///srv/other.git");
    match moves("nowhere", false) {
        AppError::BadDestination { known, .. } => {
            assert_eq!(known, " (configured remotes: other, team)")
        }
        other => panic!("{other:?}"),
    }
    fx.files.directory("elsewhere");
    ok(fx.deps.move_bundle(&MoveRequest {
        name: "authored".to_string(),
        to: "elsewhere".to_string(),
        ..MoveRequest::default()
    }));
    // The bundle is gone from here; a second one of the same name is not.
    fx.files.author("authored", &[("bundle.yaml", "v: 2\n")]);
    assert!(matches!(
        moves("elsewhere", false),
        AppError::DestinationExists { .. }
    ));
    ok(fx.deps.move_bundle(&MoveRequest {
        name: "authored".to_string(),
        to: "elsewhere".to_string(),
        force: true,
        ..MoveRequest::default()
    }));
    fx.files.author("authored", &[("bundle.yaml", "v: 3\n")]);
    fx.files.directory(".");
    assert!(matches!(
        moves(".", false),
        AppError::DestinationItself { .. }
    ));
    assert!(fx.files.has_authored("authored"));
    assert!(matches!(
        fx.deps
            .move_bundle(&MoveRequest {
                name: "missing".to_string(),
                to: "elsewhere".to_string(),
                ..MoveRequest::default()
            })
            .result
            .expect_err("refused"),
        AppError::BundleNotFound { .. }
    ));
    fx.remotes.set_unreachable(URL, true);
    assert!(matches!(moves("team", false), AppError::Publish { .. }));
    assert!(
        fx.files.has_authored("authored"),
        "a publish that failed took nothing"
    );
}
