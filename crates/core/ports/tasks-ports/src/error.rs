//! Why taskloom could not be asked.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic, Report};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TasksError {
    /// There is no `taskloom` to run: not at the path given, not on `PATH`.
    NotFound { program: String },
    /// The program exists but could not be started (not executable, say).
    Unrunnable { program: String, why: String },
    /// taskloom ran and refused, and said why in its own error envelope
    /// (`{code, args, message}`), kept as the cause.
    Refused {
        program: String,
        exit: String,
        cause: Report,
    },
    /// taskloom ran and failed without an envelope ctxloom can read: its
    /// standard error, as it wrote it.
    Failed {
        program: String,
        exit: String,
        stderr: String,
    },
    /// taskloom succeeded but its output is not the JSON a list is.
    Unparsable { program: String, why: String },
    /// taskloom did not finish within its limit; it was killed and reaped.
    TimedOut { program: String, timeout: String },
}

impl fmt::Display for TasksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for TasksError {
    fn code(&self) -> &str {
        match self {
            TasksError::NotFound { .. } => code!("not-found"),
            TasksError::Unrunnable { .. } => code!("unrunnable"),
            TasksError::Refused { .. } => code!("refused"),
            TasksError::Failed { .. } => code!("failed"),
            TasksError::Unparsable { .. } => code!("unparsable"),
            TasksError::TimedOut { .. } => code!("timed-out"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            TasksError::NotFound { program } => vec![("program", program.as_str().into())],
            TasksError::Unrunnable { program, why } | TasksError::Unparsable { program, why } => {
                vec![
                    ("program", program.as_str().into()),
                    ("why", why.as_str().into()),
                ]
            }
            TasksError::Refused {
                program,
                exit,
                cause,
            } => vec![
                ("program", program.as_str().into()),
                ("exit", exit.as_str().into()),
                ("cause", Arg::nested(cause)),
            ],
            TasksError::Failed {
                program,
                exit,
                stderr,
            } => vec![
                ("program", program.as_str().into()),
                ("exit", exit.as_str().into()),
                ("stderr", stderr.as_str().into()),
            ],
            TasksError::TimedOut { program, timeout } => vec![
                ("program", program.as_str().into()),
                ("timeout", timeout.as_str().into()),
            ],
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every error this crate defines, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(TasksError::NotFound {
            program: "taskloom".to_string(),
        }),
        Box::new(TasksError::Unrunnable {
            program: "/opt/taskloom".to_string(),
            why: "Permission denied (os error 13)".to_string(),
        }),
        Box::new(TasksError::Refused {
            program: "taskloom".to_string(),
            exit: "exit status: 2".to_string(),
            cause: Report::new("tag-rules-tag-rules").arg("findings", "not-permitted area:cli"),
        }),
        Box::new(TasksError::Failed {
            program: "taskloom".to_string(),
            exit: "exit status: 1".to_string(),
            stderr: "taskloom: no such project".to_string(),
        }),
        Box::new(TasksError::Unparsable {
            program: "taskloom".to_string(),
            why: "expected value at line 1 column 1".to_string(),
        }),
        Box::new(TasksError::TimedOut {
            program: "taskloom".to_string(),
            timeout: "30s".to_string(),
        }),
    ]
}
