//! Why a command did not complete, and its exit code (D28: 1 failed, 2
//! refused before any state was read, 3 fatal findings).

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

/// This crate's en-US messages (`locales/en-US/messages.ftl`, ids under the
/// crate's name).
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

#[derive(Debug)]
pub enum CliError {
    /// The command line could not be parsed: clap's account of why.
    Usage { detail: String },
    /// A verb that is none of the command's: what to try instead.
    UnknownVerb {
        verb: String,
        command: String,
        choices: String,
    },
    /// A `--format` that is neither text nor json.
    Format { value: String },
    /// The answer could not be written out.
    Output { detail: String },
    /// A test command that refuses.
    #[cfg(feature = "test-support")]
    TestRefused,
    /// A test command with fatal findings.
    #[cfg(feature = "test-support")]
    TestFindings { count: u64 },
    /// What a service behind a test command said.
    #[cfg(feature = "test-support")]
    Service(Box<dyn Diagnostic>),
}

impl CliError {
    /// The process exit code.
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Usage { .. } | CliError::UnknownVerb { .. } | CliError::Format { .. } => 2,
            CliError::Output { .. } => 1,
            #[cfg(feature = "test-support")]
            CliError::TestRefused => 2,
            #[cfg(feature = "test-support")]
            CliError::TestFindings { .. } => 3,
            #[cfg(feature = "test-support")]
            CliError::Service(_) => 1,
        }
    }

    /// The remedy's diagnostic: `<code>-fix`, whose catalog message, where
    /// there is one, says what to do. It takes the error's own args.
    pub fn fix(&self) -> Fix<'_> {
        Fix {
            code: format!("{}-fix", self.code()),
            of: self,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for CliError {
    fn code(&self) -> &str {
        match self {
            CliError::Usage { .. } => code!("usage"),
            CliError::UnknownVerb { .. } => code!("unknown-verb"),
            CliError::Format { .. } => code!("unknown-format"),
            CliError::Output { .. } => code!("output"),
            #[cfg(feature = "test-support")]
            CliError::TestRefused => code!("test-refused"),
            #[cfg(feature = "test-support")]
            CliError::TestFindings { .. } => code!("test-findings"),
            #[cfg(feature = "test-support")]
            CliError::Service(cause) => cause.code(),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            CliError::Usage { detail } | CliError::Output { detail } => {
                vec![("ext_err_txt", detail.as_str().into())]
            }
            CliError::UnknownVerb {
                verb,
                command,
                choices,
            } => vec![
                ("verb", verb.as_str().into()),
                ("command", command.as_str().into()),
                ("choices", choices.as_str().into()),
            ],
            CliError::Format { value } => vec![("value", value.as_str().into())],
            #[cfg(feature = "test-support")]
            CliError::TestRefused => Vec::new(),
            #[cfg(feature = "test-support")]
            CliError::TestFindings { count } => vec![("count", (*count).into())],
            #[cfg(feature = "test-support")]
            CliError::Service(cause) => cause.args(),
        }
    }
}

/// The remedy of an error: its code with `-fix`, and its args.
#[derive(Debug)]
pub struct Fix<'a> {
    code: String,
    of: &'a CliError,
}

impl fmt::Display for Fix<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for Fix<'_> {
    fn code(&self) -> &str {
        &self.code
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        self.of.args()
    }
}

/// One value of every error here, for a catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(CliError::Usage {
            detail: "unexpected argument".to_string(),
        }),
        Box::new(CliError::UnknownVerb {
            verb: "frobnicate".to_string(),
            command: "ctxloom".to_string(),
            choices: "version".to_string(),
        }),
        Box::new(CliError::Format {
            value: "yaml".to_string(),
        }),
        Box::new(CliError::Output {
            detail: "broken pipe".to_string(),
        }),
        #[cfg(feature = "test-support")]
        Box::new(CliError::TestRefused),
        #[cfg(feature = "test-support")]
        Box::new(CliError::TestFindings { count: 1 }),
    ]
}
