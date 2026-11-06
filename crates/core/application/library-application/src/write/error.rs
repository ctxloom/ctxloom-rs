//! Why a write did not happen. Each is a refusal that names what was asked
//! and, in the catalog, the command that would do it right; a write that is
//! refused has written nothing.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

macro_rules! write_errors {
    ($($(#[$doc:meta])* $variant:ident { $($field:ident),* } = $code:literal;)*) => {
        /// A write that was refused or failed.
        #[derive(Clone, PartialEq, Eq, Debug)]
        pub enum WriteError {
            $($(#[$doc])* $variant { $($field: String),* },)*
        }

        impl Diagnostic for WriteError {
            fn code(&self) -> &str {
                match self {
                    $(WriteError::$variant { .. } => code!($code),)*
                }
            }

            fn args(&self) -> Vec<(&str, Arg)> {
                match self {
                    $(WriteError::$variant { $($field),* } => {
                        vec![$((stringify!($field), $field.as_str().into())),*]
                    })*
                }
            }
        }

        /// One value of every error here, for the catalog test.
        pub fn samples() -> Vec<Box<dyn Diagnostic>> {
            vec![$(Box::new(WriteError::$variant {
                $($field: stringify!($field).to_string()),*
            })),*]
        }
    };
}

write_errors! {
    /// The write side's ports were not wired.
    NotWired { } = "write-not-wired";
    /// A content write with no project.
    NoProject { } = "write-no-project";
    /// A bundle that is already there.
    BundleExists { path } = "write-bundle-exists";
    /// A bundle name that is no name.
    BadBundleName { } = "write-bad-bundle-name";
    /// An item (fragment, command) that is already there.
    ItemExists { kind, name } = "write-item-exists";
    /// An item name that is no single path segment.
    BadItemName { kind, name } = "write-bad-item-name";
    /// An item nobody has.
    ItemNotFound { kind, name } = "write-item-not-found";
    /// An editor that returned nothing.
    EmptyEdit { kind, name } = "write-empty-edit";
    /// An edit that saved its content raw because nothing can distill yet.
    DistillUnavailable { kind, name, reference } = "write-distill-unavailable";
    /// An export with nowhere to go.
    ExportNeedsDest { } = "write-export-needs-dest";
    /// An export onto a file that is there.
    ExportExists { path } = "write-export-exists";
    /// An import that would replace what is there.
    ImportExists { what, path } = "write-import-exists";
    /// An import of what is no bundle.
    NotABundle { path } = "write-not-a-bundle";
    /// An import of a bundle that does not read.
    ImportUnreadable { path, cause } = "write-import-unreadable";
    /// A profile that is already there.
    ProfileExists { name } = "write-profile-exists";
    /// A profile that composes nothing.
    ProfileEmpty { } = "write-profile-empty";
    /// A parent nobody has.
    ParentNotFound { name } = "write-parent-not-found";
    /// A profile name that is no name.
    BadProfileName { name } = "write-bad-profile-name";
    /// A profile name with a path in it.
    ProfileNameSegment { name } = "write-profile-name-segment";
    /// A profile that does not decode after the change.
    ProfileInvalid { cause } = "write-profile-invalid";
    /// An agent that is not there to edit.
    AgentMissing { name } = "write-agent-missing";
    /// An agent that is already there.
    AgentExists { name } = "write-agent-exists";
    /// An agent removed that was not defined.
    AgentNotDefined { name } = "write-agent-not-defined";
    /// An agent removal of a name config.yaml does not hold.
    AgentNotInConfig { name } = "write-agent-not-in-config";
    /// An engine nobody defines.
    UnknownEngine { name, engine, valid } = "write-unknown-engine";
    /// A runtime that is no axis.
    UnknownRuntime { name, value } = "write-unknown-runtime";
    /// An engine home that is neither host nor session.
    UnknownEngineHome { name, value } = "write-unknown-engine-home";
    /// A permission mode its engine does not take.
    Permissions { subject, detail } = "write-permissions";
    /// A permission mode with no engine to check it against.
    PermissionsNeedEngine { name } = "write-permissions-need-engine";
    /// An agent that binds nothing.
    AgentWithoutEngine { name } = "write-agent-without-engine";
    /// An env declaration that would not do what it says.
    BadEnv { name, detail } = "write-bad-env";
    /// A surface preference the engine does not offer.
    SurfaceRefused { name, detail } = "write-surface-refused";
    /// A surface preference with no engine to check it against.
    SurfaceNeedsEngine { name } = "write-surface-needs-engine";
    /// A root selection the engine does not offer.
    RootRefused { name, detail } = "write-root-refused";
    /// A root selection with no engine to check it against.
    RootNeedsEngine { name } = "write-root-needs-engine";
    /// An llm type nobody knows.
    UnknownLlmType { label, kind, known } = "write-unknown-llm-type";
    /// An llm label that already resolves.
    LlmExists { label } = "write-llm-exists";
    /// An llm label that nothing defines.
    LlmMissing { label } = "write-llm-missing";
    /// An llm removal preview of a label config.yaml does not hold.
    LlmNotDefined { label } = "write-llm-not-defined";
    /// An llm removal of a label config.yaml does not hold.
    LlmNotInConfig { label } = "write-llm-not-in-config";
    /// An llm field the schema refuses.
    LlmField { label, detail } = "write-llm-field";
    /// A name that is no companion's.
    CompanionName { name } = "write-companion-name";
    /// A companion whose program does not exist.
    CompanionProgramMissing { program, name } = "write-companion-program-missing";
    /// A companion registered with no command.
    CompanionNoCommand { name } = "write-companion-no-command";
    /// A command that does not answer the probe.
    NotACompanion { command, detail } = "write-not-a-companion";
    /// A command that prints no loadout.
    CompanionSilent { command } = "write-companion-silent";
    /// A companion that is not registered.
    CompanionNotRegistered { name } = "write-companion-not-registered";
    /// An MCP server nobody ships in the bundle.
    McpServerNotFound { name } = "write-mcp-server-not-found";
    /// An MCP server that would not load.
    McpInvalid { name, detail } = "write-mcp-invalid";
    /// An MCP edit that left nothing.
    McpEditEmpty { bundle } = "write-mcp-edit-empty";
    /// An MCP edit that left what is no server.
    McpEditInvalid { detail } = "write-mcp-edit-invalid";
    /// A reference that is not bundle#mcp/name.
    McpReference { got } = "write-mcp-reference";
    /// A skill that is already there.
    SkillExists { name } = "write-skill-exists";
    /// A skill nobody has.
    SkillNotFound { name } = "write-skill-not-found";
    /// A skill name that is no single path segment.
    BadSkillName { name } = "write-bad-skill-name";
    /// A skill package that is not one.
    SkillInvalid { name, detail } = "write-skill-invalid";
    /// A skill reference that is not bundle#skills/name.
    SkillReference { got } = "write-skill-reference";
    /// A config that is already there.
    ConfigExists { path } = "write-config-exists";
    /// A config edit with no config.
    ConfigMissing { path } = "write-config-missing";
    /// A config scaffold of an engine nobody knows.
    ConfigEngine { engine, valid } = "write-config-engine";
    /// A config file that does not parse, and so is not rewritten.
    ConfigCorrupt { path, cause } = "write-config-corrupt";
    /// A config the write would leave breaking its schema.
    ConfigBreaks { violations } = "write-config-breaks";
    /// A bare `mcp` off a terminal.
    McpBare { } = "write-mcp-bare";
    /// A `loadout` format ctxloom has no document in.
    LoadoutFormat { format } = "write-loadout-format";
    /// A repeatable flag whose value is not `NAME=value`.
    FlagPair { flag, got } = "write-flag-pair";
    /// `mcp` asked for json.
    McpBareJson { } = "write-mcp-bare-json";
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for WriteError {}

impl From<WriteError> for crate::AppError {
    fn from(error: WriteError) -> crate::AppError {
        crate::AppError::cause(error)
    }
}
