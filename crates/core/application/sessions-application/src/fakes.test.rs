//! Fakes for the ports, for the use cases' tests. Every future they return is
//! ready at once, so [`ready`] runs a use case to its end without a runtime.

use std::collections::BTreeMap;
use std::path::PathBuf;

use angzarr_diagnostic::Report;
use angzarr_embedded::angzarr_ports::memory::MemoryStore;
use angzarr_embedded::v1::{command_page, Command, CommandBook, CommandPage, Cover, Uuid};
use angzarr_embedded::{Kernel, OnRetryable, Payload, Sampled, StoreError, Timestamp};
use ports::{Project, ProjectIdentity};
use session_ports::{
    FileItem, GoSession, GoSessions, Liveness, LockState, ProjectStores, SessionFiles,
    TranscriptFile,
};
use sessions::{
    BindEngineSession, MintSession, Origin, RecordEnd, SessionAggregate, SessionCommand,
    SESSION_DOMAIN,
};

/// Run a future whose every await is ready at once: the test support's.
pub use angzarr_embedded::testing::block_on_ready as ready;
/// The clock, ids and delay: the ports' stand-ins.
pub use ports::testing::{FakeClock, FakeDelay};

pub fn project(id: &str) -> Project {
    Project {
        id: id.to_string(),
        root: Some(format!("/work/{id}")),
        store: PathBuf::from(format!("/home/.ctxloom/projects/{id}/state.db")),
    }
}

/// A work root with a project (`bound`), or none; a pin resolves, as an
/// explicit id always does; every project is `every`.
#[derive(Default)]
pub struct FakeIdentity {
    pub bound: Option<Project>,
    pub every: Vec<Project>,
    pub fails: Option<Report>,
}

impl ProjectIdentity for FakeIdentity {
    fn for_write(&mut self, pinned: Option<&str>) -> Result<Project, Report> {
        if let Some(report) = &self.fails {
            return Err(report.clone());
        }
        Ok(match pinned {
            Some(id) => project(id),
            None => self
                .bound
                .get_or_insert_with(|| project("jolly-mute-magma"))
                .clone(),
        })
    }

    fn for_read(&mut self, pinned: Option<&str>) -> Result<Option<Project>, Report> {
        if let Some(report) = &self.fails {
            return Err(report.clone());
        }
        Ok(match pinned {
            Some(id) => Some(project(id)),
            None => self.bound.clone(),
        })
    }

    fn every_project(&mut self) -> Result<Vec<Project>, Report> {
        if let Some(report) = &self.fails {
            return Err(report.clone());
        }
        Ok(self.every.clone())
    }

    fn rootless_directory(&mut self) -> Result<Option<String>, Report> {
        Ok(None)
    }
}

/// One in-memory store per project, absent until a write opens it. The
/// projects opened to write (`opened`) are recorded, so a test can say a read
/// created nothing.
#[derive(Default)]
pub struct FakeStores {
    pub stores: BTreeMap<String, MemoryStore>,
    pub opened: Vec<String>,
}

impl ProjectStores for FakeStores {
    type Store = MemoryStore;

    async fn open(&mut self, project: &Project) -> Result<&MemoryStore, StoreError> {
        self.opened.push(project.id.clone());
        Ok(self.stores.entry(project.id.clone()).or_default())
    }

    async fn open_existing(
        &mut self,
        project: &Project,
    ) -> Result<Option<&MemoryStore>, StoreError> {
        Ok(self.stores.get(&project.id))
    }
}

/// The lock of each harp; one not named is unproven.
#[derive(Default)]
pub struct FakeLiveness(pub BTreeMap<String, LockState>);

impl Liveness for FakeLiveness {
    fn lock_state(&mut self, harp: &str) -> LockState {
        self.0.get(harp).copied().unwrap_or(LockState::Unproven)
    }
}

/// A session's files, in memory. `fail_destroy` makes a removal fail.
#[derive(Default)]
pub struct FakeFiles {
    pub transcripts: BTreeMap<String, String>,
    pub last_active: BTreeMap<String, Timestamp>,
    /// Other files per harp: (rel, bytes).
    pub others: BTreeMap<String, Vec<(String, u64)>>,
    pub destroyed: Vec<(String, Vec<String>)>,
    pub fail_destroy: bool,
    /// Output folders that exist, by path; a move renames one.
    pub outputs: std::collections::BTreeSet<String>,
    /// Every move asked for: (from, to).
    pub moves: Vec<(String, String)>,
    pub fail_move: bool,
}

const DIR: &str = "/home/.ctxloom/sessions";

impl FakeFiles {
    fn moving(&mut self, from: &str, to: &str) -> Result<bool, Report> {
        self.moves.push((from.to_string(), to.to_string()));
        if self.fail_move || self.outputs.contains(to) {
            return Err(Report::new("session-files-output-exists"));
        }
        let moved = self.outputs.remove(from);
        if moved {
            self.outputs.insert(to.to_string());
        }
        Ok(moved)
    }
}

impl SessionFiles for FakeFiles {
    fn move_output(&mut self, from: &str, to: &str) -> Result<bool, Report> {
        self.moving(from, to)
    }

    fn transcript(&mut self, harp: &str) -> Result<Option<TranscriptFile>, Report> {
        Ok(self.transcripts.get(harp).map(|text| TranscriptFile {
            path: format!("{DIR}/{harp}/transcripts/transcript.jsonl"),
            bytes: text.len() as u64,
        }))
    }

    fn read_transcript(&mut self, harp: &str) -> Result<Option<String>, Report> {
        Ok(self.transcripts.get(harp).cloned())
    }

    fn last_active(&mut self, harp: &str) -> Result<Option<Timestamp>, Report> {
        Ok(self.last_active.get(harp).copied())
    }

    fn survey(&mut self, harp: &str) -> Result<Vec<FileItem>, Report> {
        let mut found: Vec<(String, u64)> = self.others.get(harp).cloned().unwrap_or_default();
        if let Some(text) = self.transcripts.get(harp) {
            found.push((
                "transcripts/transcript.jsonl".to_string(),
                text.len() as u64,
            ));
        }
        found.sort();
        Ok(found
            .into_iter()
            .map(|(rel, bytes)| FileItem {
                path: format!("{DIR}/{harp}/{rel}"),
                rel,
                bytes,
            })
            .collect())
    }

    fn destroy(&mut self, harp: &str, rels: &[String]) -> Result<u64, Report> {
        if self.fail_destroy {
            return Err(Report::new("fake-files-busy"));
        }
        let mut freed = 0;
        for rel in rels {
            if rel == "transcripts/transcript.jsonl" {
                freed += self.transcripts.remove(harp).map_or(0, |t| t.len() as u64);
            } else if let Some(files) = self.others.get_mut(harp) {
                files.retain(|(r, bytes)| {
                    let hit = r == rel;
                    if hit {
                        freed += bytes;
                    }
                    !hit
                });
            }
        }
        self.destroyed.push((harp.to_string(), rels.to_vec()));
        Ok(freed)
    }
}

/// The stream of the session with stable id `id`.
pub fn cover(id: u8) -> Cover {
    Cover {
        domain: SESSION_DOMAIN.to_string(),
        root: Some(Uuid::from_bytes([id; 16])),
        edition: None,
    }
}

/// Run `command` on session `id`'s stream in `store` directly, as another
/// process would, sampled at `seconds`.
pub fn run(store: &MemoryStore, id: u8, seconds: i64, command: impl Into<SessionCommand>) {
    let registry = crate::registry();
    let sampled = Sampled {
        now: Timestamp { seconds, nanos: 0 },
        command_id: Uuid::from_bytes([(seconds % 251) as u8; 16]),
    };
    let book = CommandBook {
        cover: Some(cover(id)),
        page: Some(CommandPage {
            body: Some(command_page::Body::Command(Command {
                payload: Some(command.into().to_any()),
            })),
            ..CommandPage::default()
        }),
        handling: None,
    };
    ready(
        Kernel::new(&registry)
            .on_retryable(OnRetryable {
                max_retries: 0,
                ..OnRetryable::default()
            })
            .handle::<SessionAggregate, _>(store, book, &sampled),
    )
    .unwrap_or_else(|failure| panic!("{failure:?}"));
}

/// A session minted in `store` at `started`, with an engine and (when
/// `native_id` is not empty) bound.
pub fn minted(store: &MemoryStore, id: u8, harp: &str, project_dir: &str, started: i64) {
    run(
        store,
        id,
        started,
        MintSession {
            harp: harp.to_string(),
            project_id: "jolly-mute-magma".to_string(),
            project_dir: project_dir.to_string(),
            output_dir: format!("/home/Documents/ctxloom/proj/{harp}"),
            engine: "claude-code".to_string(),
            origin: Origin::Session,
            started_at: None,
            depth: 0,
        },
    );
}

pub fn bound(store: &MemoryStore, id: u8, at: i64, native_id: &str, transcript: &str) {
    run(
        store,
        id,
        at,
        BindEngineSession {
            native_id: native_id.to_string(),
            transcript_path: transcript.to_string(),
            engine_version: None,
        },
    );
}

pub fn ended(store: &MemoryStore, id: u8, at: i64, ended_at: i64) {
    run(
        store,
        id,
        at,
        RecordEnd {
            at: Timestamp {
                seconds: ended_at,
                nanos: 0,
            },
        },
    );
}

/// Go's session directories, as a test lays them out.
#[derive(Default)]
pub struct FakeGo(pub Vec<GoSession>);

impl GoSessions for FakeGo {
    fn go_sessions(&mut self) -> Result<Vec<GoSession>, Report> {
        Ok(self.0.clone())
    }
}

/// A project whose work root is `root`.
pub fn project_at(id: &str, root: &str) -> Project {
    Project {
        root: Some(root.to_string()),
        ..project(id)
    }
}

/// The claim stream of `harp` in project `project`.
pub fn claim_cover(project: &str, harp: &str) -> Cover {
    Cover {
        domain: sessions::NAMES_DOMAIN.to_string(),
        root: Some(sessions::claim_root(project, harp)),
        edition: None,
    }
}

/// `session` (a UUID's text) takes the name `harp` of `project` in `store`
/// directly, as a mint does; the kernel's answer.
pub fn claim(
    store: &MemoryStore,
    project: &str,
    harp: &str,
    session: &str,
) -> Result<(), Box<angzarr_embedded::Failure<angzarr_embedded::Rejected>>> {
    let registry = crate::registry();
    let command = sessions::NameClaimCommand::from(sessions::ClaimName {
        project_id: project.to_string(),
        harp: harp.to_string(),
        session: session.to_string(),
    });
    let book = CommandBook {
        cover: Some(claim_cover(project, harp)),
        page: Some(CommandPage {
            body: Some(command_page::Body::Command(Command {
                payload: Some(command.to_any()),
            })),
            ..CommandPage::default()
        }),
        handling: None,
    };
    let sampled = Sampled {
        now: Timestamp {
            seconds: 1_700_000_000,
            nanos: 0,
        },
        command_id: Uuid::from_bytes(
            *uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                format!("{project}{harp}{session}").as_bytes(),
            )
            .as_bytes(),
        ),
    };
    ready(
        Kernel::new(&registry)
            .on_retryable(OnRetryable {
                max_retries: 0,
                ..OnRetryable::default()
            })
            .handle::<sessions::NameClaimAggregate, _>(store, book, &sampled),
    )
    .map(drop)
    .map_err(Box::new)
}

/// Who holds the name `harp` of `project` in `store`; empty while free.
pub fn holder(store: &MemoryStore, project: &str, harp: &str) -> String {
    let registry = crate::registry();
    let (claim, _) = ready(
        Kernel::new(&registry)
            .load::<sessions::NameClaimAggregate, _>(store, &claim_cover(project, harp)),
    )
    .unwrap();
    claim.state().state().session.clone()
}

/// The stable id text of the session on stream `id`.
pub fn id_text(id: u8) -> String {
    uuid::Uuid::from_bytes([id; 16]).to_string()
}
