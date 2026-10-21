//! The use cases, over fake ports and annals' in-memory store, run through
//! annals' kernel as the binary runs them over SQLite.

use annals::next::v1::event_meta::Causation;
use annals::next::AsyncEventStore;
use annals_diagnostic::{Diagnostic, Report};

use crate::fakes::{
    project, ready, FakeClock, FakeDelay, FakeIdentity, FakeNames, FakeStores, Fault, Flaky,
};
use crate::{
    cover, AppError, ListQuery, NewTask, Service, StatusChange, TagChange, TaskService, TextEdit,
    Written, HARP_ATTEMPTS,
};

type Fake = Service<FakeIdentity, FakeStores, FakeNames, FakeClock, FakeDelay>;

fn service(names: &[&str]) -> Fake {
    Service::new(
        FakeIdentity::default(),
        FakeStores::default(),
        FakeNames::of(names),
        FakeClock::default(),
        FakeDelay::default(),
    )
}

fn new_task(text: &str) -> NewTask {
    NewTask {
        text: text.to_string(),
        ..NewTask::default()
    }
}

fn add(service: &mut Fake, task: NewTask) -> Result<Written, AppError> {
    ready(service.add(task))
}

fn listed(service: &mut Fake, query: ListQuery) -> Vec<String> {
    ready(service.list(query))
        .unwrap()
        .tasks()
        .map(|t| t.harp_id.clone())
        .collect()
}

/// The command ids recorded on `harp`'s events.
fn command_ids(service: &mut Fake, harp: &str) -> Vec<annals::next::v1::Uuid> {
    let slice = ready(service.stores.store.read_from(&cover(harp), 0))
        .unwrap()
        .expect("the stream");
    slice
        .pages
        .iter()
        .filter_map(|p| match p.meta.as_ref()?.causation.as_ref()? {
            Causation::CommandId(id) => Some(id.clone()),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------ Send

/// Compiles only for a `Send` value.
fn is_send<T: Send>(_: &T) {}

/// What a server generic over the ports and the service relies on: it
/// compiles only if the traits themselves promise `Send` futures, whatever
/// implements them.
fn spawnable<S, T, D>(service: &mut S, stores: &mut T, delay: &mut D)
where
    S: TaskService + Send,
    T: ports::TaskStores + Send,
    D: ports::Delay + Send,
{
    is_send(&service.add(NewTask::default()));
    is_send(&service.tag(TagChange::default()));
    is_send(&service.list(ListQuery::default()));
    is_send(&service.set_status(StatusChange::default()));
    is_send(&service.edit(TextEdit::default()));
    is_send(&service.show(crate::ShowQuery::default()));
    is_send(&service.summary(crate::Scope::default()));
    is_send(&service.statuses(crate::Scope::default()));
    is_send(&service.tags(ListQuery::default()));
    is_send(&service.lint(crate::Scope::default()));
    is_send(&stores.open(&project()));
    is_send(&stores.open_existing(&project()));
    is_send(&delay.sleep(std::time::Duration::ZERO));
}

#[test]
fn test_every_use_case_and_port_future_is_send_so_a_server_can_spawn_it() {
    let mut service = service(&["swift-amber"]);
    is_send(&service.add(new_task("Ship it")));
    is_send(&service.tag(change("swift-amber", &["x"], &[])));
    is_send(&service.list(ListQuery::default()));
    let (mut stores, mut delay) = (FakeStores::default(), FakeDelay::default());
    spawnable(&mut service, &mut stores, &mut delay);
}

// ------------------------------------------------------------ add

#[test]
fn test_an_add_mints_a_harp_and_answers_with_the_task_and_its_project() {
    let mut service = service(&["swift-amber"]);
    let task = NewTask {
        project: Some("pinned-id".to_string()),
        text: "Ship it".to_string(),
        status: "In Progress".to_string(),
        trigger: String::new(),
        tags: vec!["b".to_string(), "a".to_string()],
        origin_session: "calm-otter-fox".to_string(),
        config_set: Vec::new(),
    };
    let written = add(&mut service, task).unwrap();
    assert_eq!(written.project, project());
    assert_eq!(written.task.harp_id, "swift-amber");
    assert_eq!(written.task.text, "Ship it");
    assert_eq!(written.task.status, "In Progress");
    assert_eq!(spelled(&written.task.tags), ["a", "b"]);
    assert_eq!(written.task.origin_session, "calm-otter-fox");
    assert_eq!(
        written.task.created_at.seconds, 1_700_000_001,
        "recorded at the clock's instant"
    );
    assert_eq!(service.identity.pins, [Some("pinned-id".to_string())]);
    assert_eq!(service.stores.opened, ["jolly-mute-magma"]);
}

#[test]
fn test_an_add_whose_harp_is_taken_mints_another_with_a_fresh_command_id() {
    let mut service = service(&["swift-amber", "swift-amber", "calm-otter"]);
    add(&mut service, new_task("First")).unwrap();
    let second = add(&mut service, new_task("Second")).unwrap();
    assert_eq!(second.task.harp_id, "calm-otter");
    assert_eq!(second.task.text, "Second");
    assert_eq!(service.names.minted, 3);
    assert_eq!(service.clock.ids.len(), 3, "one command id per harp tried");
    let landed = command_ids(&mut service, "calm-otter");
    assert_eq!(landed, [service.clock.ids[2].clone()]);
    assert_eq!(command_ids(&mut service, "swift-amber").len(), 1);
}

#[test]
fn test_an_add_gives_up_once_every_harp_it_mints_is_taken() {
    let mut service = service(&["swift-amber"]);
    service.names.then = Some("swift-amber".to_string());
    add(&mut service, new_task("First")).unwrap();
    service.names.minted = 0;
    let error = add(&mut service, new_task("Second")).unwrap_err();
    assert_eq!(error, AppError::HarpsExhausted { attempts: 100 });
    assert_eq!(service.names.minted, HARP_ATTEMPTS);
}

#[test]
fn test_an_add_the_domain_refuses_is_answered_with_its_rejection() {
    let mut service = service(&["swift-amber"]);
    let error = add(&mut service, new_task("   ")).unwrap_err();
    let AppError::Rejected(rejection) = &error else {
        panic!("rejected: {error:?}");
    };
    assert_eq!(rejection.code(), "tasks-text-required");
    assert_eq!(service.names.minted, 1, "not re-minted");
}

#[test]
fn test_an_add_retries_its_one_command_id_through_store_failures() {
    let mut service = service(&["swift-amber"]);
    service.stores.store = Flaky::failing(&[Fault::Unavailable, Fault::LandsThenFails]);
    let written = add(&mut service, new_task("Ship it")).unwrap();
    assert_eq!(written.task.harp_id, "swift-amber");
    assert_eq!(service.stores.store.begun, 3);
    assert_eq!(service.delay.slept.len(), 2);
    assert_eq!(service.clock.ids.len(), 1, "the same command id each time");
    assert_eq!(command_ids(&mut service, "swift-amber").len(), 1);
}

#[test]
fn test_an_add_that_cannot_resolve_its_project_fails_with_why() {
    let mut service = service(&["swift-amber"]);
    service.identity.fails = Some(Report::new("project-registry-x", "no home"));
    let error = add(&mut service, new_task("Ship it")).unwrap_err();
    assert_eq!(
        error,
        AppError::Identity(Report::new("project-registry-x", "no home"))
    );
    assert!(service.stores.opened.is_empty());
}

#[test]
fn test_an_add_whose_store_does_not_open_fails() {
    let mut service = service(&["swift-amber"]);
    service.stores.store.open_fails = true;
    let error = add(&mut service, new_task("Ship it")).unwrap_err();
    assert_eq!(error.code(), "annals-next-store-unavailable");
}

// ------------------------------------------------------------ tag

fn change(harp: &str, add: &[&str], remove: &[&str]) -> TagChange {
    TagChange {
        project: Some("pinned-id".to_string()),
        harp_id: harp.to_string(),
        add: add.iter().map(|s| s.to_string()).collect(),
        remove: remove.iter().map(|s| s.to_string()).collect(),
        config_set: Vec::new(),
    }
}

// A tag outside tagma's grammar could never be queried: it is refused
// before anything is resolved or written.
#[test]
fn test_an_add_with_a_tag_outside_the_grammar_is_refused_and_writes_nothing() {
    let mut service = service(&["swift-amber"]);
    let task = NewTask {
        tags: vec!["ok".to_string(), "not ok".to_string()],
        ..new_task("Ship it")
    };
    let error = add(&mut service, task).unwrap_err();
    assert_eq!(error.code(), "tasks-application-tag-unparseable");
    assert!(error.to_string().contains("not ok"), "{error}");
    assert!(service.identity.pins.is_empty(), "no project resolved");
    assert!(service.stores.opened.is_empty());
}

#[test]
fn test_a_tag_naming_a_tag_outside_the_grammar_is_refused_either_way() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, new_task("Ship it")).unwrap();
    for change in [
        change("swift-amber", &["a/b"], &[]),
        change("swift-amber", &[], &["a/b"]),
    ] {
        let error = ready(service.tag(change)).unwrap_err();
        assert_eq!(error.code(), "tasks-application-tag-unparseable");
    }
    assert_eq!(command_ids(&mut service, "swift-amber").len(), 1, "the add");
}

// A tag's spelling is not its identity: `k="v"` is `k=v`.
#[test]
fn test_a_tag_spelled_another_way_is_the_same_tag() {
    let mut service = service(&["swift-amber"]);
    let task = NewTask {
        tags: vec!["k=v".to_string()],
        ..new_task("Ship it")
    };
    add(&mut service, task).unwrap();
    let written = ready(service.tag(change("swift-amber", &[], &["k=\"v\""]))).unwrap();
    assert!(written.task.tags.is_empty());
}

// ------------------------------------------------------------ schema

/// What the configuration answers, and the overrides it was asked with.
#[derive(Default)]
struct FakeConfiguration {
    declarations: Vec<String>,
    asked: Vec<Vec<String>>,
}

impl ports::Configuration for FakeConfiguration {
    fn tag_schema(&mut self, overrides: &[String]) -> Result<Vec<String>, Report> {
        self.asked.push(overrides.to_vec());
        if self.declarations.iter().any(|d| d == "unreadable") {
            return Err(Report::new("x-config", "the config cannot be read"));
        }
        Ok(self.declarations.clone())
    }
}

type Configured = Service<
    FakeIdentity,
    FakeStores,
    FakeNames,
    FakeClock,
    FakeDelay,
    crate::NoGoLogs,
    FakeConfiguration,
>;

fn configured(declarations: &[&str]) -> Configured {
    service(&["swift-amber"]).with_configuration(FakeConfiguration {
        declarations: declarations.iter().map(|d| d.to_string()).collect(),
        asked: Vec::new(),
    })
}

fn tagged(add: &[&str]) -> NewTask {
    NewTask {
        tags: add.iter().map(|s| s.to_string()).collect(),
        ..new_task("Ship it")
    }
}

// The default schema holds with nothing configured: triage:level is 1-5.
#[test]
fn test_a_write_is_held_to_the_default_schema() {
    let mut service = service(&["swift-amber"]);
    let error = add(&mut service, tagged(&["triage:level=9"])).unwrap_err();
    assert_eq!(error.code(), "tasks-application-tag-out-of-range");
    assert!(service.identity.pins.is_empty(), "refused before resolving");
    for refused in [
        &["and"][..],
        &["tagma.hide:x=true"],
        &["triage:kind=sparkles"],
    ] {
        assert!(add(&mut service, tagged(refused)).is_err(), "{refused:?}");
    }
}

#[test]
fn test_a_configured_declaration_extends_the_default_schema() {
    let mut service = configured(&[
        r#"tagma.range:"triage:level"="1,9""#,
        r#"tagma.enum:"area:x"="a""#,
    ]);
    let task = NewTask {
        config_set: vec!["tag_schema=x".to_string()],
        ..tagged(&["triage:level=9"])
    };
    let written = ready(service.add(task)).unwrap();
    assert_eq!(spelled(&written.task.tags), ["triage:level=9"]);
    assert_eq!(
        service.configuration.asked,
        [vec!["tag_schema=x".to_string()]]
    );
    let error = ready(service.add(tagged(&["area:x=b"]))).unwrap_err();
    assert_eq!(error.code(), "tasks-application-tag-not-in-enum");
    let error = ready(service.add(tagged(&["triage:kind=sparkles"]))).unwrap_err();
    assert_eq!(
        error.code(),
        "tasks-application-tag-not-in-enum",
        "the default still holds"
    );
}

#[test]
fn test_a_malformed_or_unreadable_configuration_refuses_every_write() {
    let mut service = configured(&["tagma.arty:x=1"]);
    let error = ready(service.add(new_task("Ship it"))).unwrap_err();
    assert_eq!(error.code(), "tasks-application-schema-unknown-facet");
    assert!(matches!(error, AppError::Configuration(_)));
    let error = ready(service.tag(change("swift-amber", &["x"], &[]))).unwrap_err();
    assert_eq!(error.code(), "tasks-application-schema-unknown-facet");
    let mut service = configured(&["unreadable"]);
    let error = ready(service.add(new_task("Ship it"))).unwrap_err();
    assert_eq!(error.code(), "x-config");
    assert!(service.stores.opened.is_empty());
}

// The tag schema a write is held to, as a reader asks for it: the default
// extended by what is configured, the overrides passed on; and refused as
// a write is, where the configuration is.
#[test]
fn test_the_tag_schema_is_the_one_a_write_is_held_to() {
    let mut service = configured(&[r#"tagma.range:"triage:level"="1,9""#]);
    let overrides = vec!["tag_schema=x".to_string()];
    let schema = ready(service.tag_schema(overrides.clone())).unwrap();
    assert_eq!(
        schema.range_of("triage:level"),
        Some(crate::Range { min: 1.0, max: 9.0 })
    );
    assert!(schema.is_scalar("triage:kind"), "the default still holds");
    assert_eq!(service.configuration.asked, [overrides]);
    assert!(service.identity.pins.is_empty(), "no project is resolved");
    let mut service = configured(&["tagma.arty:x=1"]);
    let error = ready(service.tag_schema(Vec::new())).unwrap_err();
    assert_eq!(error.code(), "tasks-application-schema-unknown-facet");
}

// Scalar collapse, resolved before the retag is sent: Go's
// TestScalarCollapse_* cases, over the default schema's scalars.
#[test]
fn test_a_new_scalar_value_displaces_the_one_the_task_holds() {
    let mut service = service(&["swift-amber"]);
    add(
        &mut service,
        tagged(&["triage:level=3", "triage:kind=chore", "x"]),
    )
    .unwrap();
    let written = ready(service.tag(change("swift-amber", &["triage:level=1"], &[]))).unwrap();
    assert_eq!(
        spelled(&written.task.tags),
        ["triage:kind=chore", "triage:level=1", "x"]
    );
    let written = ready(service.tag(change("swift-amber", &["triage:level=\"1\""], &[]))).unwrap();
    assert_eq!(
        written.task.tags.len(),
        3,
        "the same value, spelled otherwise"
    );
    assert_eq!(
        command_ids(&mut service, "swift-amber").len(),
        2,
        "nothing recorded"
    );
}

#[test]
fn test_an_add_keeps_the_last_of_several_scalar_values() {
    let mut service = service(&["swift-amber"]);
    let written = add(
        &mut service,
        tagged(&["triage:level=3", "area:x=1", "area:x=2", "triage:level=2"]),
    )
    .unwrap();
    assert_eq!(
        spelled(&written.task.tags),
        ["area:x=1", "area:x=2", "triage:level=2"]
    );
}

#[test]
fn test_a_removed_tag_is_never_checked_against_the_schema() {
    let mut service = configured(&[]);
    add_unchecked(&mut service, "triage:level=9");
    let written = ready(service.tag(change("swift-amber", &[], &["triage:level=9"]))).unwrap();
    assert!(written.task.tags.is_empty());
}

/// Add a task holding `tag` as one written before the schema forbade it:
/// its add goes past the use case, straight to the domain.
fn add_unchecked(service: &mut Configured, tag: &str) {
    use crate::tags::parse;
    let add = tasks::AddTask {
        harp_id: "swift-amber".to_string(),
        text: "Ship it".to_string(),
        status: String::new(),
        trigger: String::new(),
        tags: vec![parse(tag).unwrap().unwrap()],
        origin_session: String::new(),
    };
    let mut runner = crate::retry::Runner {
        registry: &service.registry,
        clock: &mut service.clock,
        delay: &mut service.delay,
        backoff: service.backoff,
    };
    let book = crate::book("swift-amber", add.into());
    let id = annals::next::v1::Uuid::from_bytes([7; 16]);
    ready(runner.run(&mut service.stores.store, &book, &id)).unwrap();
}

#[test]
fn test_a_tag_adds_then_removes_and_answers_with_the_task() {
    let mut service = service(&["swift-amber"]);
    let task = NewTask {
        tags: vec!["old".to_string()],
        ..new_task("Ship it")
    };
    add(&mut service, task).unwrap();
    let written =
        ready(service.tag(change("swift-amber", &["new", "both"], &["old", "both"]))).unwrap();
    assert_eq!(spelled(&written.task.tags), ["new"]);
    assert_eq!(written.project, project());
    assert_eq!(
        service.identity.pins.last(),
        Some(&Some("pinned-id".to_string()))
    );
}

#[test]
fn test_a_tag_changing_nothing_answers_with_the_task_as_it_is() {
    let mut service = service(&["swift-amber"]);
    let task = NewTask {
        tags: vec!["kept".to_string()],
        ..new_task("Ship it")
    };
    add(&mut service, task).unwrap();
    let written = ready(service.tag(change("swift-amber", &["kept"], &[]))).unwrap();
    assert_eq!(spelled(&written.task.tags), ["kept"]);
}

#[test]
fn test_a_tag_with_nothing_to_add_or_remove_is_refused_before_anything_is_read() {
    let mut service = configured(&["unreadable"]);
    for (add, remove) in [(&[][..], &[][..]), (&[" "], &[""])] {
        let error = ready(service.tag(change("swift-amber", add, remove))).unwrap_err();
        assert_eq!(error.code(), "tasks-tag-required");
    }
    assert!(
        service.configuration.asked.is_empty(),
        "no configuration read"
    );
    assert!(service.identity.pins.is_empty(), "no project resolved");
    let error = ready(service.tag(change("swift-amber", &[], &[" x "]))).unwrap_err();
    assert_eq!(error.code(), "x-config", "one tag is something to do");
}

#[test]
fn test_a_tag_of_no_task_is_refused() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, new_task("Ship it")).unwrap();
    let error = ready(service.tag(change("calm-otter", &["x"], &[]))).unwrap_err();
    assert_eq!(error.code(), "tasks-no-task");
}

#[test]
fn test_a_tag_where_there_is_no_project_fails_and_binds_none() {
    let mut service = service(&[]);
    let unpinned = TagChange {
        project: None,
        ..change("swift-amber", &["x"], &[])
    };
    let error = ready(service.tag(unpinned)).unwrap_err();
    assert_eq!(error, AppError::NoProject);
    assert_eq!(
        service.identity.bound, None,
        "resolved for a read: none minted"
    );
    assert!(service.stores.opened.is_empty() && service.stores.looked.is_empty());
}

#[test]
fn test_a_tag_in_a_project_with_no_store_yet_is_refused_and_creates_none() {
    let mut service = service(&[]);
    service.stores.absent = true;
    let error = ready(service.tag(change("swift-amber", &["x"], &[]))).unwrap_err();
    assert_eq!(error.code(), "tasks-no-task");
    assert_eq!(service.stores.looked, ["jolly-mute-magma"]);
    assert!(service.stores.opened.is_empty(), "no store opened to write");
}

#[test]
fn test_a_tag_retries_through_store_failures() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, new_task("Ship it")).unwrap();
    service.stores.store.faults = [Fault::Conflict, Fault::LandsThenFails].into();
    let written = ready(service.tag(change("swift-amber", &["x"], &[]))).unwrap();
    assert_eq!(spelled(&written.task.tags), ["x"]);
    assert_eq!(service.delay.slept.len(), 2);
    assert_eq!(
        command_ids(&mut service, "swift-amber").len(),
        2,
        "add, retag"
    );
}

#[test]
fn test_a_tag_that_gives_up_says_so() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, new_task("Ship it")).unwrap();
    service.backoff.attempts = 2;
    service.stores.store.faults = [Fault::Unavailable, Fault::Unavailable].into();
    let error = ready(service.tag(change("swift-amber", &["x"], &[]))).unwrap_err();
    assert!(
        matches!(error, AppError::GaveUp { attempts: 2, .. }),
        "{error:?}"
    );
}

#[test]
fn test_a_tag_whose_project_does_not_resolve_fails_with_why() {
    let mut service = service(&[]);
    service.identity.fails = Some(Report::new("project-registry-x", "no home"));
    let error = ready(service.tag(change("swift-amber", &["x"], &[]))).unwrap_err();
    assert_eq!(error.code(), "project-registry-x");
}

#[test]
fn test_a_tag_whose_store_does_not_open_fails() {
    let mut service = service(&[]);
    service.identity.bound = Some(project());
    service.stores.store.open_fails = true;
    let error = ready(service.tag(change("swift-amber", &["x"], &[]))).unwrap_err();
    assert_eq!(error.code(), "annals-next-store-unavailable");
}

// ------------------------------------------------------------ status

fn moved(harp: &str, status: &str, trigger: &str) -> StatusChange {
    StatusChange {
        project: Some("pinned-id".to_string()),
        harp_id: harp.to_string(),
        status: status.to_string(),
        trigger: trigger.to_string(),
        config_set: Vec::new(),
    }
}

#[test]
fn test_a_status_change_moves_the_task_and_answers_with_it_and_its_project() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, new_task("Ship it")).unwrap();
    let written = ready(service.set_status(moved("swift-amber", "Done", ""))).unwrap();
    assert_eq!(written.task.status, "Done");
    assert!(written.task.checked);
    assert_eq!(written.project, project());
    assert_eq!(
        service.identity.pins.last(),
        Some(&Some("pinned-id".to_string()))
    );
    assert_eq!(command_ids(&mut service, "swift-amber").len(), 2);
}

#[test]
fn test_deferred_needs_a_trigger_given_or_held() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, new_task("Ship it")).unwrap();
    let error = ready(service.set_status(moved("swift-amber", "Deferred", " "))).unwrap_err();
    assert_eq!(error.code(), "tasks-trigger-required");
    let written =
        ready(service.set_status(moved("swift-amber", "Deferred", "the release"))).unwrap();
    assert_eq!(written.task.trigger, "the release");
    ready(service.set_status(moved("swift-amber", "To Do", ""))).unwrap();
    let again = ready(service.set_status(moved("swift-amber", "Deferred", ""))).unwrap();
    assert_eq!(again.task.trigger, "the release", "kept from before");
}

#[test]
fn test_a_status_class_is_no_status_to_move_to() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, new_task("Ship it")).unwrap();
    let error = ready(service.set_status(moved("swift-amber", "@open", ""))).unwrap_err();
    assert_eq!(error.code(), "tasks-status-reserved");
}

#[test]
fn test_a_status_change_of_no_task_or_where_there_is_none_is_refused() {
    let mut held = service(&["swift-amber"]);
    add(&mut held, new_task("Ship it")).unwrap();
    let error = ready(held.set_status(moved("calm-otter", "Done", ""))).unwrap_err();
    assert_eq!(error.code(), "tasks-no-task");
    let mut nowhere = service(&[]);
    let unpinned = StatusChange {
        project: None,
        ..moved("swift-amber", "Done", "")
    };
    let error = ready(nowhere.set_status(unpinned)).unwrap_err();
    assert_eq!(error, AppError::NoProject);
    assert!(nowhere.stores.opened.is_empty() && nowhere.stores.looked.is_empty());
    let mut storeless = service(&[]);
    storeless.stores.absent = true;
    let error = ready(storeless.set_status(moved("swift-amber", "Done", ""))).unwrap_err();
    assert_eq!(error.code(), "tasks-no-task");
    assert!(storeless.stores.opened.is_empty(), "no store created");
}

#[test]
fn test_a_status_change_retries_through_store_failures() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, new_task("Ship it")).unwrap();
    service.stores.store.faults = [Fault::Conflict].into();
    let written = ready(service.set_status(moved("swift-amber", "Done", ""))).unwrap();
    assert_eq!(written.task.status, "Done");
    assert_eq!(service.delay.slept.len(), 1);
}

#[test]
fn test_a_status_change_under_a_malformed_configuration_is_refused() {
    let mut service = configured(&["tagma.arty:x=1"]);
    let error = ready(service.set_status(moved("swift-amber", "Done", ""))).unwrap_err();
    assert_eq!(error.code(), "tasks-application-schema-unknown-facet");
    assert!(service.identity.pins.is_empty());
}

// ------------------------------------------------------------ edit

fn edited(harp: &str, text: &str) -> TextEdit {
    TextEdit {
        project: Some("pinned-id".to_string()),
        harp_id: harp.to_string(),
        text: text.to_string(),
        config_set: Vec::new(),
    }
}

#[test]
fn test_an_edit_replaces_the_text_whole_and_keeps_the_status_and_trigger() {
    let mut service = service(&["swift-amber"]);
    let deferred = NewTask {
        status: "Deferred".to_string(),
        trigger: "the release".to_string(),
        ..new_task("Ship it")
    };
    let before = add(&mut service, deferred).unwrap();
    let written = ready(service.edit(edited("swift-amber", "  Ship it, then tell\n"))).unwrap();
    assert_eq!(written.task.text, "Ship it, then tell", "trimmed");
    assert_ne!(written.task.text_hash, before.task.text_hash);
    assert_eq!(written.task.status, "Deferred");
    assert_eq!(written.task.trigger, "the release");
    assert_eq!(written.project, project());
    assert_eq!(
        service.identity.pins.last(),
        Some(&Some("pinned-id".to_string()))
    );
}

#[test]
fn test_an_edit_to_no_text_or_no_task_is_refused() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, new_task("Ship it")).unwrap();
    let error = ready(service.edit(edited("swift-amber", " "))).unwrap_err();
    assert_eq!(error.code(), "tasks-text-required");
    let error = ready(service.edit(edited("calm-otter", "x"))).unwrap_err();
    assert_eq!(error.code(), "tasks-no-task");
    let mut nowhere = self::service(&[]);
    let unpinned = TextEdit {
        project: None,
        ..edited("swift-amber", "x")
    };
    assert_eq!(
        ready(nowhere.edit(unpinned)).unwrap_err(),
        AppError::NoProject
    );
    let mut configured = configured(&["tagma.arty:x=1"]);
    let error = ready(configured.edit(edited("swift-amber", "x"))).unwrap_err();
    assert_eq!(error.code(), "tasks-application-schema-unknown-facet");
}

// ------------------------------------------------------------ list

/// A service holding tasks in each default status, added in this order.
fn with_every_status() -> Fake {
    let mut service = service(&["to-do", "doing", "deferred", "done", "archived", "other"]);
    for (status, trigger) in [
        ("To Do", ""),
        ("In Progress", ""),
        ("Deferred", "the release"),
        ("Done", ""),
        ("Archived", ""),
        ("Blocked", ""),
    ] {
        let task = NewTask {
            status: status.to_string(),
            trigger: trigger.to_string(),
            ..new_task(status)
        };
        add(&mut service, task).unwrap();
    }
    service
}

#[test]
fn test_a_list_by_default_holds_the_open_tasks_in_the_order_added() {
    let mut service = with_every_status();
    assert_eq!(
        listed(&mut service, ListQuery::default()),
        ["to-do", "doing", "other"]
    );
}

#[test]
fn test_a_list_of_all_holds_every_task() {
    let mut service = with_every_status();
    let all = ListQuery {
        all: true,
        ..ListQuery::default()
    };
    assert_eq!(
        listed(&mut service, all),
        ["to-do", "doing", "deferred", "done", "archived", "other"]
    );
}

#[test]
fn test_a_list_of_named_statuses_holds_exactly_their_tasks() {
    let mut service = with_every_status();
    let named = ListQuery {
        statuses: vec!["Done".to_string(), "Deferred".to_string()],
        ..ListQuery::default()
    };
    assert_eq!(listed(&mut service, named), ["deferred", "done"]);
}

// ------------------------------------------------------------ tag queries

fn queried(query: &str) -> ListQuery {
    ListQuery {
        tag_query: query.to_string(),
        ..ListQuery::default()
    }
}

// Go's TestFilterTasksTagQueryPreservesInputOrder: the tasks a query
// selects keep the listing's order.
#[test]
fn test_a_tag_query_keeps_the_tasks_it_selects_in_the_listings_order() {
    let mut service = service(&["zebra-a", "mango-b", "apple-c", "kiwi-d"]);
    for tags in [&["urgent"][..], &["urgent"], &["urgent", "x"], &[]] {
        add(&mut service, tagged(tags)).unwrap();
    }
    assert_eq!(
        listed(&mut service, queried("urgent")),
        ["zebra-a", "mango-b", "apple-c"]
    );
    assert_eq!(
        listed(&mut service, queried("urgent and not x")),
        ["zebra-a", "mango-b"]
    );
    assert_eq!(listed(&mut service, queried("urgent/not")), ["kiwi-d"]);
}

// Go's TestFilterTasksTagQueryNotScopedToStatusCandidates: `not`
// complements within the tasks the listing admits, never past them.
#[test]
fn test_not_complements_within_the_listed_tasks() {
    let mut service = service(&["keep-me", "done-urgent"]);
    add(&mut service, new_task("Keep")).unwrap();
    let done = NewTask {
        status: "Done".to_string(),
        ..tagged(&["urgent"])
    };
    add(&mut service, done).unwrap();
    assert_eq!(listed(&mut service, queried("urgent/not")), ["keep-me"]);
    let all = ListQuery {
        all: true,
        ..queried("urgent")
    };
    assert_eq!(listed(&mut service, all), ["done-urgent"]);
}

#[test]
fn test_a_malformed_tag_query_is_refused_even_with_nothing_to_list() {
    let mut service = service(&[]);
    let error = ready(service.list(queried("urgent/and"))).unwrap_err();
    assert_eq!(error.code(), "tasks-application-tag-query");
    assert!(matches!(error, AppError::TagQuery(_)));
    assert!(service.identity.pins.is_empty(), "refused before resolving");
}

// The default schema declares triage:blocks-release a SemVer version.
#[test]
fn test_a_tag_query_orders_a_semver_target_by_precedence() {
    let mut service = service(&["below-a", "pre-b", "above-c"]);
    for version in ["0.6.9", "0.7.0-pre001", "0.8.0"] {
        let tag = format!("triage:blocks-release={version}");
        add(&mut service, tagged(&[tag.as_str()])).unwrap();
    }
    assert_eq!(
        listed(&mut service, queried("triage:blocks-release<=0.7.0")),
        ["below-a", "pre-b"]
    );
}

#[test]
fn test_tasks_added_at_one_instant_list_by_harp_id() {
    let mut service = service(&["zebra-b", "alpha-a"]);
    add(&mut service, new_task("First")).unwrap();
    service.clock.seconds = 0;
    add(&mut service, new_task("Second")).unwrap();
    assert_eq!(
        listed(&mut service, ListQuery::default()),
        ["alpha-a", "zebra-b"]
    );
}

#[test]
fn test_a_list_answers_with_each_tasks_values_and_its_project() {
    let mut service = service(&["swift-amber"]);
    let added = add(&mut service, new_task("Ship it")).unwrap();
    let listing = ready(service.list(ListQuery::default())).unwrap();
    assert_eq!(listing.project(), Some(&project()));
    assert_eq!(listing.tasks().collect::<Vec<_>>(), [&added.task]);
}

#[test]
fn test_a_list_where_there_is_no_project_in_a_work_root_is_empty_and_opens_no_store() {
    let mut service = service(&[]);
    let listing = ready(service.list(ListQuery::default())).unwrap();
    assert_eq!(listing.reach, crate::Reach::NoProject);
    assert!(listing.rows.is_empty());
    assert!(service.stores.opened.is_empty() && service.stores.looked.is_empty());
    assert_eq!(
        service.identity.bound, None,
        "resolved for a read: none minted"
    );
}

#[test]
fn test_a_list_of_a_project_with_no_store_yet_is_empty_and_creates_none() {
    let mut service = service(&[]);
    service.stores.absent = true;
    let query = ListQuery {
        project: Some("pinned-id".to_string()),
        ..ListQuery::default()
    };
    let listing = ready(service.list(query)).unwrap();
    assert_eq!(listing.project(), Some(&project()));
    assert!(listing.rows.is_empty());
    assert_eq!(service.stores.looked, ["jolly-mute-magma"]);
    assert!(service.stores.opened.is_empty(), "no store opened to write");
    assert_eq!(service.identity.pins, [Some("pinned-id".to_string())]);
}

#[test]
fn test_a_list_opens_its_store_to_read() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, new_task("Ship it")).unwrap();
    listed(&mut service, ListQuery::default());
    assert_eq!(service.stores.opened, ["jolly-mute-magma"], "the add's");
    assert_eq!(service.stores.looked, ["jolly-mute-magma"], "the list's");
}

#[test]
fn test_a_list_whose_project_does_not_resolve_fails_with_why() {
    let mut service = service(&[]);
    service.identity.fails = Some(Report::new("project-registry-x", "no home"));
    let error = ready(service.list(ListQuery::default())).unwrap_err();
    assert_eq!(error.code(), "project-registry-x");
}

#[test]
fn test_a_list_whose_store_does_not_open_fails() {
    let mut service = service(&[]);
    service.identity.bound = Some(project());
    service.stores.store.open_fails = true;
    let error = ready(service.list(ListQuery::default())).unwrap_err();
    assert_eq!(error.code(), "annals-next-store-unavailable");
}

#[test]
fn test_an_add_in_a_status_class_is_refused() {
    let mut service = service(&["swift-amber"]);
    let task = NewTask {
        status: "@terminal".to_string(),
        ..new_task("Ship it")
    };
    let error = add(&mut service, task).unwrap_err();
    assert_eq!(error.code(), "tasks-status-reserved");
}

/// Each tag in its canonical spelling.
fn spelled(tags: &[tasks::Tag]) -> Vec<String> {
    tags.iter().map(ToString::to_string).collect()
}

// ------------------------------------------------------------ reading back

#[test]
fn test_a_task_read_back_as_no_events_is_never_a_blank_task() {
    // A stream purged mid-read can read back as a stream with no events.
    let mut service = service(&["swift-amber"]);
    service.stores.store.hollow = true;
    let error = add(&mut service, new_task("Ship it")).unwrap_err();
    assert_eq!(
        error,
        AppError::Vanished {
            harp_id: "swift-amber".to_string()
        }
    );
    assert!(listed(&mut service, ListQuery::default()).is_empty());
}
