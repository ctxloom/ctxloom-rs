//! `mcp server set|edit` (Go's `operations.SetBundleMCP` and
//! `cli/bundle_items.go`): the servers a bundle ships live in the bundle, as
//! `mcp/<n>.yaml` and `mcp/.<n>.meta.yaml`; the project registers none of its
//! own.

use library::model::McpServer;
use library::write::mcp::{self as mcpfile, McpPatch};

use crate::error::AppError;
use crate::project::{Library, Outcome, Project};
use crate::write::editor::editor_command;
use crate::write::plan::Plan;
use crate::write::views::{McpEdited, McpSet};
use crate::write::WriteError;
use crate::Flags;

impl Library {
    /// `mcp server set <bundle>#mcp/<name> ...`: the fields passed change, the
    /// rest keep their value, a name the bundle lacks is created. A server with
    /// not exactly one target is refused and nothing is saved.
    pub fn mcp_set(&self, flags: &Flags, reference: &str, patch: &McpPatch) -> Outcome<McpSet> {
        self.run(flags, |p| p.mcp_set(reference, patch))
    }

    /// `mcp server edit <bundle>#mcp/<name>`: the server through the editor.
    pub fn mcp_edit(&self, flags: &Flags, reference: &str) -> Outcome<McpEdited> {
        self.run(flags, |p| p.mcp_edit(reference))
    }
}

/// The bundle and server a `<bundle>#mcp/<name>` reference names.
fn split_reference(reference: &str) -> Result<(String, String), AppError> {
    let bad = || WriteError::McpReference {
        got: reference.to_string(),
    };
    let (bundle, selector) = reference.split_once('#').ok_or_else(bad)?;
    let name = selector.strip_prefix("mcp/").ok_or_else(bad)?;
    if bundle.is_empty() || !library::write::item::valid_name(name) {
        return Err(bad().into());
    }
    Ok((bundle.to_string(), name.to_string()))
}

/// The server written into the plan: its content, and its sidecar where it
/// has anything to say.
fn write_server(plan: &mut Plan, name: &str, server: &McpServer) {
    plan.write(&mcpfile::content_path(name), mcpfile::content_file(server));
    match mcpfile::meta_file(server) {
        Some(meta) => plan.write(&mcpfile::meta_path(name), meta),
        None => {
            plan.remove(&mcpfile::meta_path(name));
        }
    }
}

fn refuse_fault(name: &str, server: &McpServer) -> Result<(), AppError> {
    match mcpfile::fault(server) {
        Some(detail) => Err(WriteError::McpInvalid {
            name: name.to_string(),
            detail,
        }
        .into()),
        None => Ok(()),
    }
}

impl Project {
    fn mcp_set(&self, reference: &str, patch: &McpPatch) -> Result<McpSet, AppError> {
        self.require_project()?;
        let (bundle, name) = split_reference(reference)?;
        let entry = self.bundle(&bundle)?;
        let held = entry.bundle.mcp.get(&name);
        let server = mcpfile::patched(&held.cloned().unwrap_or_default(), patch);
        refuse_fault(&name, &server)?;
        let mut plan = self.plan_of(&entry.name)?;
        write_server(&mut plan, &name, &server);
        self.commit(&entry.name, plan)?;
        Ok(McpSet {
            status: if held.is_some() { "updated" } else { "created" }.to_string(),
            bundle: bundle.clone(),
            name,
            path: entry.manifest_path.clone(),
        })
    }

    fn mcp_edit(&self, reference: &str) -> Result<McpEdited, AppError> {
        self.require_project()?;
        let (bundle, name) = split_reference(reference)?;
        let entry = self.bundle(&bundle)?;
        let Some(held) = entry.bundle.mcp.get(&name) else {
            return Err(WriteError::McpServerNotFound { name }.into());
        };
        let ports = self.write_ports()?;
        let text = mcpfile::editor_text(held);
        let command = editor_command(&self.config, &self.environ);
        let edited = ports
            .editor
            .edit(&command, &format!("{name}.yaml"), &text)
            .map_err(AppError::Cause)?;
        if edited == text {
            return Ok(McpEdited::Unchanged);
        }
        let server = mcpfile::parse_edited(&edited).map_err(|detail| {
            if edited.trim().is_empty() {
                WriteError::McpEditEmpty {
                    bundle: bundle.clone(),
                }
            } else {
                WriteError::McpEditInvalid { detail }
            }
        })?;
        refuse_fault(&name, &server)?;
        let mut plan = self.plan_of(&entry.name)?;
        write_server(&mut plan, &name, &server);
        self.commit(&entry.name, plan)?;
        Ok(McpEdited::Updated { name, bundle })
    }
}

#[cfg(test)]
#[path = "mcp.test.rs"]
mod tests;
