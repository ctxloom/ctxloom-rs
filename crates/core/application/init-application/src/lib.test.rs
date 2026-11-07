//! `init`. Go: `cli/init.feature` (release/0.7, f36e8dfb21).

use crate::{personal_name, prompt, SETUP_INTERVIEW};

// init.feature: "Init prompt emits the whole setup interview body": the
// headings from both ends of it.
#[test]
fn the_prompt_carries_the_phase_headings_from_both_ends() {
    let text = prompt();
    for heading in [
        "Phase 1 \u{2014} Open, orient, scan",
        "Phase 2 \u{2014} Companions",
        "Phase 4 \u{2014} Agents",
        "Phase 5 \u{2014} Close",
    ] {
        assert!(text.contains(heading), "{heading}");
    }
    assert!(text.ends_with(&format!("{SETUP_INTERVIEW}\n")));
    assert!(!text.starts_with("---"), "the front matter is dropped");
}

// init.feature: the personal remote is "personal"; the next are numbered.
#[test]
fn the_remotes_are_named_personal_then_numbered() {
    assert_eq!(personal_name(0), "personal");
    assert_eq!(personal_name(1), "personal-2");
    assert_eq!(personal_name(2), "personal-3");
}

mod init {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;

    use ctxloom_ports::{Failure, Put, Scaffold, UserFiles};
    use deps_application::fakes::Fixture;

    use crate::{Create, Init, InitError, InitRequest};

    /// The scaffold, counting how often it was asked for.
    #[derive(Default)]
    struct Creator {
        made: Cell<u32>,
        refuses: bool,
    }

    impl Create for Rc<Creator> {
        fn create(&self) -> Result<(), Failure> {
            self.made.set(self.made.get() + 1);
            if self.refuses {
                return Err(Box::new(deps_application::AppError::DefaultNeedsName));
            }
            Ok(())
        }
    }

    struct Dir {
        present: Result<bool, &'static str>,
    }

    impl Scaffold for Dir {
        fn app_dir(&self) -> String {
            "/work/.ctxloom".to_string()
        }
        fn exists(&self, _: &str) -> Result<bool, Failure> {
            unreachable!()
        }
        fn write(&self, _: &[Put]) -> Result<(), Failure> {
            unreachable!()
        }
    }

    impl UserFiles for Dir {
        fn exists(&self, _: &str) -> Result<bool, Failure> {
            unreachable!()
        }
        fn is_dir(&self, path: &str) -> Result<bool, Failure> {
            assert_eq!(path, "/work/.ctxloom");
            self.present
                .map_err(|_| Box::new(deps_application::AppError::DefaultNeedsName) as Failure)
        }
        fn read_file(&self, _: &str) -> Result<Vec<u8>, Failure> {
            unreachable!()
        }
        fn read_tree(&self, _: &str) -> Result<Vec<Put>, Failure> {
            unreachable!()
        }
        fn write_file(&self, _: &str, _: &[u8], _: bool) -> Result<bool, Failure> {
            unreachable!()
        }
        fn write_tree(&self, _: &str, _: &[Put]) -> Result<(), Failure> {
            unreachable!()
        }
    }

    fn init(present: Result<bool, &'static str>) -> Init {
        init_with(present, Rc::new(Creator::default()), Fixture::new())
    }

    fn init_with(
        present: Result<bool, &'static str>,
        creator: Rc<Creator>,
        fixture: Fixture,
    ) -> Init {
        let dir = Arc::new(Dir { present });
        Init::new(dir.clone(), dir, Rc::new(fixture.deps), Box::new(creator))
    }

    // init.feature: a directory with no `.ctxloom` is scaffolded, and the
    // dependencies the seeded profile resolves through are pulled.
    #[test]
    fn a_directory_without_ctxloom_is_scaffolded_and_its_dependencies_pulled() {
        let fixture = Fixture::new();
        let url = "file:///srv/origin.git";
        fixture.origin("origin", url);
        fixture.profile("dev", &[&Fixture::reference(url, "demo")]);
        let creator = Rc::new(Creator::default());
        let done = init_with(Ok(false), creator.clone(), fixture)
            .init(&InitRequest::default())
            .expect("ok");
        assert_eq!(creator.made.get(), 1);
        assert!(!done.existed);
        assert_eq!(done.app_dir, "/work/.ctxloom");
        assert_eq!(done.pulled, 1);
    }

    // The re-run branch scaffolds nothing and pulls nothing.
    #[test]
    fn an_existing_directory_is_not_scaffolded_again() {
        let creator = Rc::new(Creator::default());
        let done = init_with(Ok(true), creator.clone(), Fixture::new())
            .init(&InitRequest::default())
            .expect("ok");
        assert_eq!(creator.made.get(), 0);
        assert_eq!(done.pulled, 0);
    }

    // A pull that fails (the network refused) is said, and the config stays
    // written: init does not fail for it.
    #[test]
    fn a_pull_that_fails_is_a_notice_and_the_project_stays_scaffolded() {
        let fixture = Fixture::new();
        let url = "file:///srv/origin.git";
        fixture.origin("origin", url);
        fixture.profile("dev", &[&Fixture::reference(url, "demo")]);
        fixture.remotes.set_unreachable(url, true);
        let done = init_with(Ok(false), Rc::new(Creator::default()), fixture)
            .init(&InitRequest::default())
            .expect("ok");
        assert_eq!(done.pulled, 0);
        assert!(!done.notices.is_empty(), "the failed pull is said");
    }

    // A scaffold that is refused ends init with the scaffold's own reason.
    #[test]
    fn a_scaffold_that_is_refused_ends_init() {
        let creator = Rc::new(Creator {
            refuses: true,
            ..Creator::default()
        });
        let error = init_with(Ok(false), creator, Fixture::new())
            .init(&InitRequest::default())
            .expect_err("refused");
        assert!(matches!(error, InitError::Create(_)));
    }

    #[test]
    fn an_existing_directory_is_reported_as_existing() {
        let done = init(Ok(true)).init(&InitRequest::default()).expect("ok");
        assert!(done.existed);
        assert_eq!(done.app_dir, "/work/.ctxloom");
        assert!(done.remotes.is_empty());
    }

    #[test]
    fn a_directory_that_cannot_be_inspected_is_the_adapters_failure() {
        let error = init(Err("denied"))
            .init(&InitRequest::default())
            .expect_err("fails");
        assert!(matches!(error, InitError::Files(_)));
    }
}

mod error {
    use angzarr_diagnostic::Diagnostic;

    use crate::InitError;

    #[test]
    fn each_variant_is_its_cause_as_itself() {
        let cause = || Box::new(deps_application::AppError::DefaultNeedsName);
        let want = cause();
        for error in [InitError::Files(cause()), InitError::Create(cause())] {
            assert_eq!(error.code(), want.code());
            assert_eq!(error.args().len(), want.args().len());
            assert!(error.to_string().contains(want.code()));
        }
    }

    // Every variant is its cause's code and its cause's args, names and
    // values, from a cause that has some.
    #[test]
    fn each_variant_carries_its_causes_args() {
        use angzarr_diagnostic::Arg;
        let cause = || {
            Box::new(deps_application::AppError::NoRemoteNamed {
                name: "origin".to_string(),
            })
        };
        let want = cause();
        assert_eq!(want.args().len(), 1, "the cause has an arg to carry");
        for error in [InitError::Files(cause()), InitError::Create(cause())] {
            assert_eq!(error.code(), want.code());
            let args = error.args();
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].0, want.args()[0].0);
            assert!(
                matches!(&args[0].1, Arg::Str(v) if v == "origin"),
                "{:?}",
                args[0].1
            );
        }
    }
}
