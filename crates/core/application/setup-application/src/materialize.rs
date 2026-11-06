//! `materialize`: the assembled profiles written as each engine's native
//! files, any subset of them, and released (Go's `operations.Materialize`).
//!
//! Each engine's kinds are owned separately, so re-delivering one kind, or one
//! engine, never takes another's files out, and `--release` takes back exactly
//! what it names.

use std::collections::BTreeMap;

use angzarr_diagnostic::Diagnostic;
use library_application::{EnginePackage, Flags, PackageRequest};
use serde::Serialize;
use surface_ports::{
    Command, EngineSurfaces, Hook, Inputs, Inspected, Kind, McpServer, Skill, SkillFile,
    SurfaceError,
};

use crate::owner_voice::with_own_fragment;
use crate::{Done, Setup, SetupError};

/// One `--surface KIND[=MECHANISM][:DEST]`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SurfaceSpec {
    pub kind: Kind,
    /// The only mechanism at rest is the native file.
    pub mechanism: String,
    /// For context only: a file inside the target the context goes to.
    pub dest: String,
}

impl SurfaceSpec {
    /// The spec a `--surface` spells.
    pub fn parse(text: &str) -> Result<SurfaceSpec, SetupError> {
        let (head, dest) = match text.split_once(':') {
            Some((head, dest)) => (head, dest),
            None => (text, ""),
        };
        let (name, mechanism) = head.split_once('=').unwrap_or((head, ""));
        let kind = Kind::parse(name).ok_or_else(|| SetupError::Request {
            detail: format!("{name} is not a surface kind"),
        })?;
        Ok(SurfaceSpec {
            kind,
            mechanism: mechanism.to_string(),
            dest: dest.to_string(),
        })
    }
}

/// One at-rest delivery: which profiles, into which directory, for which
/// engines, which surfaces.
#[derive(Clone, Debug, Default)]
pub struct MaterializeRequest {
    /// The profile set; none is the default agent's.
    pub profiles: Vec<String>,
    /// Where; none is the project's directory, which needs `yes`.
    pub target: Option<String>,
    pub yes: bool,
    /// The engines; none is the configured ones.
    pub engines: Vec<String>,
    /// The kinds; none is every kind.
    pub surfaces: Vec<SurfaceSpec>,
    /// The empty plan for the selected kinds.
    pub release: bool,
    /// Plan and report; write nothing.
    pub dry_run: bool,
    /// Leave the context alone (neither write nor release it) for an engine
    /// whose context rides the launch: the project refresh sets it, for a
    /// launched session carries that context itself.
    pub leave_launch_context: bool,
    pub flags: Flags,
}

/// A surface an engine does not carry.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct NotCarried {
    pub surface: String,
    pub reason: String,
}

/// A kind an engine's delivery was refused, and why.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KindRefusal {
    pub kind: String,
    pub cause: SurfaceError,
}

/// One engine's part of a materialize.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EngineOutcome {
    pub engine: String,
    /// The kinds delivered (or, in a dry run, that would change).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub wrote: Vec<String>,
    /// The selected kinds this run left with nothing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub released: Vec<String>,
    /// The file the context was written to.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub context_file: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_carried: Vec<NotCarried>,
    /// Each command or skill the engine would not carry, and each value it
    /// left standing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// The kinds refused, with the cause (said in words by whoever renders
    /// them).
    #[serde(skip)]
    pub refused: Vec<KindRefusal>,
}

/// What one materialize did, per engine.
#[derive(Debug, Default, Serialize)]
pub struct MaterializeResult {
    pub target: String,
    /// The target did not exist and was created.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub created: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<String>,
    pub engines: Vec<EngineOutcome>,
    /// `applied`, `partial`, `failed`, `released`, `planned` (a dry run) or
    /// `nothing` (nothing to release).
    pub status: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// The failures, to be said in words by whoever renders them.
    #[serde(skip)]
    pub failures: Vec<SurfaceError>,
}

impl Setup {
    /// Deliver (or release) the selected kinds for each engine at the target.
    pub fn materialize(
        &self,
        request: &MaterializeRequest,
    ) -> Result<Done<MaterializeResult>, SetupError> {
        if request.release && !request.profiles.is_empty() {
            return Err(SetupError::Request {
                detail: "--release takes no profiles".to_string(),
            });
        }
        let (kinds, dest) = kinds_of(request)?;
        let explicit = request.target.is_some();
        let target = match &request.target {
            Some(target) => self.absolute(target),
            None => self.project_dir.clone(),
        };
        let outcome = self.library.package(
            &request.flags,
            &PackageRequest {
                profiles: request.profiles.clone(),
            },
        );
        let notices = outcome.notices;
        let package = outcome.result.map_err(SetupError::cause)?;
        let names = self.engines_of(request, &package.configured_engines)?;
        if !explicit && !request.yes {
            return Err(SetupError::NoTarget {
                engines: names.join(", "),
                dir: target,
            });
        }
        let state = self.store.inspect(&target).map_err(SetupError::cause)?;
        if state == Inspected::Other {
            return Err(SetupError::TargetNotDirectory { path: target });
        }

        let mut result = MaterializeResult {
            target: target.clone(),
            profiles: if request.release {
                Vec::new()
            } else if request.profiles.is_empty() {
                package.profiles.clone()
            } else {
                request.profiles.clone()
            },
            ..MaterializeResult::default()
        };
        if request.release {
            self.release(request, &names, &kinds, state, &mut result)?;
        } else {
            self.apply(request, &names, &kinds, &dest, &package, state, &mut result)?;
        }
        Ok(Done {
            value: result,
            notices,
        })
    }

    /// The engines a request names, each registered; else the configured
    /// ones; else the default.
    pub(crate) fn engines_of(
        &self,
        request: &MaterializeRequest,
        configured: &[String],
    ) -> Result<Vec<String>, SetupError> {
        let known = self.engines.names();
        if !request.engines.is_empty() {
            let mut out: Vec<String> = Vec::new();
            for name in &request.engines {
                if !known.contains(name) {
                    return Err(SetupError::UnknownEngine {
                        engine: name.clone(),
                        known: known.join(", "),
                    });
                }
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
            return Ok(out);
        }
        let configured: Vec<String> = configured
            .iter()
            .filter(|e| known.contains(e))
            .cloned()
            .collect();
        if !configured.is_empty() {
            return Ok(configured);
        }
        Ok(known.into_iter().take(1).collect())
    }

    fn release(
        &self,
        request: &MaterializeRequest,
        names: &[String],
        kinds: &[Kind],
        state: Inspected,
        result: &mut MaterializeResult,
    ) -> Result<(), SetupError> {
        if state == Inspected::Missing {
            result.status = "nothing".to_string();
            result.warnings.push(format!(
                "{} does not exist; there is nothing to release",
                result.target
            ));
            return Ok(());
        }
        for name in names {
            let mut outcome = EngineOutcome {
                engine: name.clone(),
                ..EngineOutcome::default()
            };
            for kind in kinds {
                match self
                    .store
                    .release(&result.target, name, *kind, request.dry_run)
                {
                    Ok(_) => outcome.released.push(kind.name().to_string()),
                    Err(e) => {
                        outcome
                            .warnings
                            .push(format!("{}: {}", kind.name(), e.code()));
                        result.failures.push(e);
                    }
                }
            }
            result.engines.push(outcome);
        }
        result.status = status(request, result, "released");
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn apply(
        &self,
        request: &MaterializeRequest,
        names: &[String],
        kinds: &[Kind],
        dest: &str,
        package: &EnginePackage,
        state: Inspected,
        result: &mut MaterializeResult,
    ) -> Result<(), SetupError> {
        // The gate: a finding a strict run acts on refuses the whole
        // delivery, before anything is written (not even the target).
        if !request.dry_run && !package.fatal.is_empty() {
            let findings: Vec<String> = package.fatal.iter().map(findings_text).collect();
            return Err(SetupError::Refused {
                findings: findings.join("; "),
            });
        }
        let context = with_own_fragment(&package.context, package.use_distilled);
        if kinds.contains(&Kind::Context) && context.trim().is_empty() {
            return Err(SetupError::EmptyContext {
                profiles: result.profiles.join(", "),
                engine: names.first().cloned().unwrap_or_default(),
            });
        }
        let mut inputs = inputs_of(package, context);
        if !dest.is_empty() {
            inputs.context_file = Some(dest.to_string());
        }
        for name in names {
            let Some(engine) = self.engines.get(name) else {
                continue;
            };
            let outcome = if request.leave_launch_context && engine.context_rides_launch() {
                let kept: Vec<Kind> = kinds
                    .iter()
                    .copied()
                    .filter(|k| *k != Kind::Context)
                    .collect();
                self.deliver(engine, &kept, &inputs, package, request, result)
            } else {
                self.deliver(engine, kinds, &inputs, package, request, result)
            };
            result.engines.push(outcome);
        }
        result.created = state == Inspected::Missing && !request.dry_run;
        result.status = status(request, result, "applied");
        Ok(())
    }

    /// One engine's kinds, in order. A kind that is refused fails alone, and
    /// so does any later kind in a file it shares: the file is as it was.
    fn deliver(
        &self,
        engine: &dyn EngineSurfaces,
        kinds: &[Kind],
        inputs: &Inputs,
        package: &EnginePackage,
        request: &MaterializeRequest,
        result: &mut MaterializeResult,
    ) -> EngineOutcome {
        let name = engine.name().to_string();
        let mut outcome = EngineOutcome {
            engine: name.clone(),
            ..EngineOutcome::default()
        };
        let mut refused: Vec<(String, SurfaceError)> = Vec::new();
        for kind in kinds {
            if !engine.carries(*kind) {
                if carries_items(package, *kind) {
                    outcome.not_carried.push(NotCarried {
                        surface: kind.name().to_string(),
                        reason: format!("{name} declares no approach for it"),
                    });
                }
                continue;
            }
            let planned = match engine.plan(*kind, inputs) {
                Ok(planned) => planned,
                Err(e) => {
                    outcome.warnings.push(format!("{}: refused", kind.name()));
                    outcome.refused.push(KindRefusal {
                        kind: kind.name().to_string(),
                        cause: e.clone(),
                    });
                    result.failures.push(e);
                    continue;
                }
            };
            outcome.skipped.extend(planned.skipped.iter().cloned());
            let files: Vec<String> = planned
                .claims
                .iter()
                .map(|c| c.path().to_string())
                .collect();
            if let Some((_, cause)) = refused.iter().find(|(file, _)| files.contains(file)) {
                outcome.refused.push(KindRefusal {
                    kind: kind.name().to_string(),
                    cause: cause.clone(),
                });
                outcome.warnings.push(format!(
                    "{}: not delivered, for it shares a file with a kind that was refused",
                    kind.name()
                ));
                continue;
            }
            match self.store.deliver(
                &result.target,
                &name,
                *kind,
                &planned.claims,
                request.dry_run,
            ) {
                Ok(done) => {
                    outcome.skipped.extend(done.left);
                    if !request.dry_run || done.changed {
                        landed(&mut outcome, *kind, &result.target, &files);
                    }
                }
                Err(e) => {
                    refused.extend(files.into_iter().map(|file| (file, e.clone())));
                    outcome.warnings.push(format!("{}: refused", kind.name()));
                    outcome.refused.push(KindRefusal {
                        kind: kind.name().to_string(),
                        cause: e.clone(),
                    });
                    result.failures.push(e);
                }
            }
        }
        if !request.release && kinds.contains(&Kind::Hooks) && engine.carries(Kind::Hooks) {
            outcome.not_carried.extend(hook_losses(engine, package));
        }
        outcome
    }
}

/// The hook events the profile configures that the engine declares it has no
/// native form for, one entry each, in event order. ctxloom's own callbacks
/// are not the profile's: an engine that cannot fire one is not a loss the
/// team made.
fn hook_losses(engine: &dyn EngineSurfaces, package: &EnginePackage) -> Vec<NotCarried> {
    engine
        .hook_losses()
        .into_iter()
        .filter(|(event, _)| {
            package
                .hooks
                .iter()
                .any(|h| h.event == *event && h.source_kind != "context")
        })
        .map(|(_, reason)| NotCarried {
            surface: Kind::Hooks.name().to_string(),
            reason,
        })
        .collect()
}

/// A kind delivered (its claims in files), or left with nothing (released).
fn landed(outcome: &mut EngineOutcome, kind: Kind, target: &str, files: &[String]) {
    let Some(first) = files.first() else {
        outcome.released.push(kind.name().to_string());
        return;
    };
    outcome.wrote.push(kind.name().to_string());
    if kind == Kind::Context {
        outcome.context_file = format!("{target}/{first}");
    }
}

fn findings_text(notice: &library_application::Notice) -> String {
    match notice {
        library_application::Notice::Finding { text, .. } => text.clone(),
        other => format!("{other:?}"),
    }
}

/// The run's status from its failures and engines.
fn status(request: &MaterializeRequest, result: &MaterializeResult, ok: &str) -> String {
    let failed = result.failures.len();
    let engines = result.engines.len();
    if request.dry_run {
        "planned"
    } else if failed == 0 {
        ok
    } else if failed < engines {
        "partial"
    } else {
        "failed"
    }
    .to_string()
}

/// The kinds a request selects (every kind when none is named), in delivery
/// order, and the context's destination.
fn kinds_of(request: &MaterializeRequest) -> Result<(Vec<Kind>, String), SetupError> {
    if request.surfaces.is_empty() {
        return Ok((Kind::ALL.to_vec(), String::new()));
    }
    let mut seen: BTreeMap<Kind, &SurfaceSpec> = BTreeMap::new();
    let mut dest = String::new();
    for spec in &request.surfaces {
        let bad = |detail: String| SetupError::Request { detail };
        if !spec.mechanism.is_empty() && spec.mechanism != "file" {
            return Err(bad(format!(
                "{}={}: at rest the only mechanism is \"file\"",
                spec.kind.name(),
                spec.mechanism
            )));
        }
        if !spec.dest.is_empty() && spec.kind != Kind::Context {
            return Err(bad(format!(
                "{} takes no destination; only context does",
                spec.kind.name()
            )));
        }
        if !spec.dest.is_empty() && request.release {
            return Err(bad("--release takes no destination".to_string()));
        }
        if let Some(prev) = seen.insert(spec.kind, spec) {
            if prev != spec {
                return Err(bad(format!(
                    "surface {} is named twice, differently",
                    spec.kind.name()
                )));
            }
        }
        if spec.kind == Kind::Context {
            dest.clone_from(&spec.dest);
        }
    }
    Ok((seen.into_keys().collect(), dest))
}

/// Whether the package has anything of `kind` to deliver.
fn carries_items(package: &EnginePackage, kind: Kind) -> bool {
    match kind {
        Kind::Context => true,
        Kind::Mcp => package.mcp.iter().any(|m| m.served_by.is_empty()),
        Kind::Settings => package.statusline || !package.deny_tools.is_empty(),
        Kind::Hooks => !package.hooks.is_empty(),
        Kind::Commands => !package.commands.is_empty(),
        Kind::Skills => !package.skills.is_empty(),
    }
}

/// What the engines are asked to deliver. ctxloom's own MCP server is the
/// session's, injected while it runs: at rest nothing is registered for a
/// server `served_by` the session.
fn inputs_of(package: &EnginePackage, context: String) -> Inputs {
    let mut hooks: BTreeMap<String, Vec<Hook>> = BTreeMap::new();
    for packaged in &package.hooks {
        let h = &packaged.hook;
        hooks.entry(packaged.event.clone()).or_default().push(Hook {
            kind: h.kind.clone(),
            command: h.command.clone(),
            args: h.args.clone(),
            prompt: h.prompt.clone(),
            timeout: h.timeout,
            is_async: h.is_async,
            tool: h.tool.clone(),
            matcher: String::new(),
        });
    }
    Inputs {
        context,
        context_file: None,
        mcp: package
            .mcp
            .iter()
            .filter(|m| m.served_by.is_empty())
            .map(|m| McpServer {
                name: m.name.clone(),
                command: m.command.clone(),
                args: m.args.clone(),
                env: m.env.clone(),
                url: m.url.clone(),
                headers: m.headers.clone(),
            })
            .collect(),
        hooks,
        deny_tools: package.deny_tools.clone(),
        statusline: package.statusline,
        commands: package
            .commands
            .iter()
            .map(|c| Command {
                name: c.name.clone(),
                description: c.description.clone(),
                body: c.body.clone(),
            })
            .collect(),
        skills: package
            .skills
            .iter()
            .map(|s| Skill {
                name: s.name.clone(),
                description: s.description.clone(),
                files: s
                    .files
                    .iter()
                    .map(|f| SkillFile {
                        path: f.path.clone(),
                        bytes: f.bytes.clone(),
                        executable: f.executable,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
#[path = "materialize.test.rs"]
mod tests;
