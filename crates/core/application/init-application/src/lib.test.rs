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

#[test]
fn every_sample_is_in_the_catalog() {
    let catalog = crate::EN_US.1;
    for sample in crate::diagnostic_samples() {
        assert!(catalog.contains(&format!("{} =", sample.code())));
    }
}

mod init {
    use std::rc::Rc;
    use std::sync::Arc;

    use ctxloom_ports::{Failure, Put, Scaffold, UserFiles};
    use deps_application::fakes::Fixture;

    use crate::{Init, InitError, InitRequest};

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
            self.present.map_err(|_| {
                Box::new(InitError::NewProjectNotBuilt {
                    app_dir: "cause".into(),
                }) as Failure
            })
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
        let dir = Arc::new(Dir { present });
        Init::new(dir.clone(), dir, Rc::new(Fixture::new().deps))
    }

    #[test]
    fn a_directory_without_ctxloom_is_refused_as_a_new_project() {
        let error = init(Ok(false))
            .init(&InitRequest::default())
            .expect_err("refused");
        assert!(matches!(
            error,
            InitError::NewProjectNotBuilt { ref app_dir } if app_dir == "/work/.ctxloom"
        ));
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

    angzarr_presentation::english_for!(&[crate::EN_US.1]);

    #[test]
    fn each_variant_has_its_code_args_and_message() {
        let new = InitError::NewProjectNotBuilt {
            app_dir: "/w/.ctxloom".into(),
        };
        assert_eq!(new.code(), "init-application-new-project-not-built");
        let args = new.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].0, "app_dir");
        assert!(new.english().contains("/w/.ctxloom"), "{}", new.english());
        assert!(new.to_string().contains("new-project-not-built"));

        let wrapped = InitError::Files(Box::new(InitError::NewProjectNotBuilt {
            app_dir: "/x".into(),
        }));
        assert_eq!(wrapped.code(), "init-application-new-project-not-built");
        assert_eq!(wrapped.args().len(), 1);
    }

    #[test]
    fn the_samples_are_not_empty() {
        assert!(!crate::diagnostic_samples().is_empty());
    }
}
