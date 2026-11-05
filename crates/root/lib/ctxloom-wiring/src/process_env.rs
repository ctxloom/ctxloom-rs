//! The environment ctxloom was launched in, behind `ExportedVars`.

use std::collections::BTreeSet;
use std::ffi::OsString;

use ctxloom_ports::ExportedVars;

/// The names exported with a value. The values are not kept: a credential
/// read from here cannot be printed or stored, only found present.
pub struct ProcessEnv {
    exported: BTreeSet<String>,
}

impl ProcessEnv {
    /// The names among `vars` that have a value.
    pub fn of(vars: impl IntoIterator<Item = (OsString, OsString)>) -> ProcessEnv {
        ProcessEnv {
            exported: vars
                .into_iter()
                .filter(|(_, value)| !value.is_empty())
                .filter_map(|(name, _)| name.into_string().ok())
                .collect(),
        }
    }

    /// This process's environment.
    pub fn from_process() -> ProcessEnv {
        ProcessEnv::of(std::env::vars_os())
    }
}

impl ExportedVars for ProcessEnv {
    fn is_exported(&self, name: &str) -> bool {
        self.exported.contains(name)
    }
}

#[cfg(test)]
#[path = "process_env.test.rs"]
mod tests;
