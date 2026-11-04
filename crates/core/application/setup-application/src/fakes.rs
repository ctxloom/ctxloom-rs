//! In-memory ports for this crate's tests: a store that remembers what it was
//! asked to deliver, and an engine that claims one file per kind.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use library_application::fakes::{Mem, Setup as LibrarySetup};
use surface_ports::{
    Claim, Delivered, EngineSurfaces, HookCodec, HookEvent, HookResponse, Inputs, Inspected, Json,
    Kind, Planned, Registry, Released, SurfaceError, SurfaceStore, Wiring,
};

use crate::Setup;

/// What a store holds: the claims of each (engine, kind), and the files.
#[derive(Default)]
pub(crate) struct State {
    pub claims: BTreeMap<(String, Kind), Vec<Claim>>,
    pub files: BTreeMap<String, Vec<u8>>,
    pub targets: BTreeMap<String, Inspected>,
    pub refuse_path: Option<String>,
    pub log: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Store(pub Mutex<State>);

impl Store {
    pub fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }
}

impl SurfaceStore for Store {
    fn deliver(
        &self,
        target: &str,
        engine: &str,
        kind: Kind,
        claims: &[Claim],
        dry: bool,
    ) -> Result<Delivered, SurfaceError> {
        let mut state = self.state();
        state
            .log
            .push(format!("deliver {engine} {} dry={dry}", kind.name()));
        if let Some(path) = &state.refuse_path {
            if claims.iter().any(|c| c.path() == path) {
                return Err(SurfaceError::Refused {
                    file: path.clone(),
                    reason: "refused".into(),
                });
            }
        }
        let changed = state
            .claims
            .get(&(engine.to_string(), kind))
            .map(Vec::as_slice)
            != Some(claims);
        if !dry {
            if claims.is_empty() {
                state.claims.remove(&(engine.to_string(), kind));
            } else {
                state
                    .claims
                    .insert((engine.to_string(), kind), claims.to_vec());
            }
            for claim in claims {
                state
                    .files
                    .insert(format!("{target}/{}", claim.path()), Vec::new());
            }
        }
        Ok(Delivered {
            files: claims.iter().map(|c| c.path().to_string()).collect(),
            left: Vec::new(),
            changed,
        })
    }

    fn release(
        &self,
        _target: &str,
        engine: &str,
        kind: Kind,
        dry: bool,
    ) -> Result<Released, SurfaceError> {
        let mut state = self.state();
        state
            .log
            .push(format!("release {engine} {} dry={dry}", kind.name()));
        let had = state.claims.contains_key(&(engine.to_string(), kind));
        if !dry {
            state.claims.remove(&(engine.to_string(), kind));
        }
        Ok(Released {
            files: Vec::new(),
            changed: had,
        })
    }

    fn claimed(&self, _target: &str, engine: &str) -> Result<Vec<Kind>, SurfaceError> {
        Ok(self
            .state()
            .claims
            .keys()
            .filter(|(e, _)| e == engine)
            .map(|(_, k)| *k)
            .collect())
    }

    fn read(&self, target: &str, rel: &str) -> Result<Option<Vec<u8>>, SurfaceError> {
        Ok(self.state().files.get(&format!("{target}/{rel}")).cloned())
    }

    fn read_absolute(&self, path: &str) -> Result<Option<Vec<u8>>, SurfaceError> {
        Ok(self.state().files.get(path).cloned())
    }

    fn inspect(&self, target: &str) -> Result<Inspected, SurfaceError> {
        Ok(self
            .state()
            .targets
            .get(target)
            .copied()
            .unwrap_or(Inspected::Directory))
    }
}

/// A hook wire of its own: `{"event": ..., "skill": ...}` in, `{...}` out.
pub(crate) struct Wire;

impl HookCodec for Wire {
    fn decode(&self, event: &str, payload: &str) -> Result<HookEvent, SurfaceError> {
        let doc = Json::parse(payload).map_err(|detail| SurfaceError::Payload {
            engine: "fake".into(),
            detail,
        })?;
        let text = |k: &str| {
            doc.get(k)
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_string()
        };
        Ok(HookEvent {
            event: event.to_string(),
            skill: text("skill"),
            transcript: text("transcript"),
            ..HookEvent::default()
        })
    }

    fn encode(&self, _event: &str, r: &HookResponse) -> Result<String, SurfaceError> {
        Ok(format!("context={};notice={}\n", r.context, r.notice))
    }

    fn invoked_skills(&self, transcript: &str) -> Vec<String> {
        transcript.lines().map(str::to_string).collect()
    }
}

/// An engine that claims `<name>/<kind>` for each kind it carries.
pub(crate) struct Engine {
    pub name: &'static str,
    pub kinds: Vec<Kind>,
    pub wire: Wire,
    pub rides_launch: bool,
}

impl Engine {
    pub fn all(name: &'static str) -> Engine {
        Engine {
            name,
            kinds: Kind::ALL.to_vec(),
            wire: Wire,
            rides_launch: false,
        }
    }
}

impl EngineSurfaces for Engine {
    fn name(&self) -> &str {
        self.name
    }

    fn carries(&self, kind: Kind) -> bool {
        self.kinds.contains(&kind)
    }

    fn context_rides_launch(&self) -> bool {
        self.rides_launch
    }

    fn plan(&self, kind: Kind, inputs: &Inputs) -> Result<Planned, SurfaceError> {
        let path = if kind == Kind::Context {
            inputs
                .context_file
                .clone()
                .unwrap_or_else(|| format!("{}.md", self.name))
        } else {
            format!("{}/{}", self.name, kind.name())
        };
        // Settings and mcp with nothing to say claim nothing, as claude's do.
        let empty = match kind {
            Kind::Mcp => inputs.mcp.is_empty(),
            Kind::Settings => !inputs.statusline && inputs.deny_tools.is_empty(),
            Kind::Commands => inputs.commands.is_empty(),
            Kind::Skills => inputs.skills.is_empty(),
            _ => false,
        };
        let mut planned = Planned::default();
        if !empty {
            let text = match kind {
                Kind::Context => inputs.context.clone(),
                _ => kind.name().to_string(),
            };
            planned.claims.push(Claim::Section { path, text });
        }
        Ok(planned)
    }

    fn wiring(&self, _store: &dyn SurfaceStore, _target: &str) -> Result<Wiring, SurfaceError> {
        Ok(Wiring::default())
    }

    fn codec(&self) -> &dyn HookCodec {
        &self.wire
    }
}

pub(crate) const CONFIG: &str = "schema_version: 8\nagents:\n  default:\n    llm: mock\n    profiles: [dev]\ndefault_agent: default\nllm:\n  configs:\n    beta:\n      type: mock\n";

/// Bundles `demo` (a fragment, a command, a skill pair, an MCP server) and
/// `project` (the profile `dev`).
pub(crate) fn bundles() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            "demo",
            vec![
                ("bundle.yaml", "schema_version: 4\nversion: 1.0.0\n"),
                ("fragments/testing.md", "FRAGMENT-BODY-testing"),
                (
                    "prompts/example.md",
                    "---\ndescription: Example\n---\nEXAMPLE-BODY",
                ),
                ("mcp/tool.yaml", "command: tool-cmd\n"),
                ("mcp/ctxloom.yaml", "served_by: session-endpoint\n"),
                (
                    "skills/admit/SKILL.md",
                    "---\nname: admit\ndescription: Admit.\n---\nA\n",
                ),
                (
                    "skills/.admit.meta.yaml",
                    "tags: [ctxloom:link_id=nightly]\n",
                ),
                (
                    "skills/unattended/SKILL.md",
                    "---\nname: unattended\ndescription: Work.\n---\nB\n",
                ),
                (
                    "skills/.unattended.meta.yaml",
                    "tags: [ctxloom:link_id=nightly]\n",
                ),
            ],
        ),
        (
            "project",
            vec![
                ("bundle.yaml", "schema_version: 4\nversion: 1.0.0\n"),
                ("profiles/dev.yaml", "bundles: [demo]\n"),
                ("profiles/broken.yaml", "bundles: [absent]\n"),
            ],
        ),
    ]
}

pub(crate) struct World {
    pub setup: Setup,
    pub store: Arc<Store>,
    pub mem: Arc<Mem>,
}

/// The use cases over a library of `bundles()` and `config`, with the engines
/// `claude-code` (every kind) and `mock` (context only).
pub(crate) fn world(config: &'static str) -> World {
    world_of(
        config,
        bundles(),
        vec![
            Box::new(Engine::all("claude-code")),
            Box::new(Engine {
                name: "mock",
                kinds: vec![Kind::Context],
                wire: Wire,
                rides_launch: false,
            }),
        ],
    )
}

/// The same over bundles and engines of the test's own.
pub(crate) fn world_of(
    config: &'static str,
    owned: Vec<(&'static str, Vec<(&'static str, &'static str)>)>,
    engines: Vec<Box<dyn EngineSurfaces>>,
) -> World {
    let trees: Vec<(&str, Vec<(&str, &str)>)> = owned;
    let slices: Vec<(&str, &[(&str, &str)])> =
        trees.iter().map(|(n, f)| (*n, f.as_slice())).collect();
    let layers = [config];
    let (library, mem) = LibrarySetup {
        bundles: &slices,
        layers: &layers,
        ..LibrarySetup::default()
    }
    .fixture();
    let store = Arc::new(Store::default());
    let engines = Registry::new(engines);
    let setup = Setup::new(library, engines, store.clone(), mem.clone(), "/p", "/p");
    World { setup, store, mem }
}
