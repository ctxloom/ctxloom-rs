//! The seed the acceptance tests plant sessions with (`ctxloom test
//! seed-session`, built with `test-support` only). Go's fixtures write a
//! session's sidecar; the Rust store has no sidecar to write, so the seed
//! mints the session in the cwd's project store through the domain and writes
//! beside it what a fixture writes: the session's lock, its canonical
//! transcript, authored files.
//!
//! It mints WITHOUT the session-name claim: the claim is the owner's design
//! (D5 Q1) and its tests await review. When it lands, the real mint (S6)
//! takes the claim first and this seed goes through it.

use std::path::Path;
use std::time::{Duration, SystemTime};

use angzarr_embedded::v1::{command_page, Command, CommandBook, CommandPage, Cover, Uuid};
use angzarr_embedded::{Kernel, Payload, Sampled, Timestamp};
use file_system::{Open, Perms};
use os::Mode;
use ports::{Clock, CommandIds, ProjectIdentity};
use session_ports::ProjectStores;
use sessions::{
    claim_root, BindEngineSession, ClaimName, MintSession, NameClaimAggregate, NameClaimCommand,
    Origin, RecordEnd, SessionAggregate, SessionCommand, SetKept, NAMES_DOMAIN, SESSION_DOMAIN,
};
use system::System;

use crate::{LibError, Wiring};

/// A session to plant.
#[derive(Clone, Debug, Default)]
pub struct Seed {
    pub harp: String,
    /// Another project than the cwd's.
    pub project: Option<String>,
    /// Seconds ago it was minted.
    pub started_ago: u64,
    /// Seconds ago its last run ended; none: it never ended.
    pub ended_ago: Option<u64>,
    /// Make its lock file, nothing holding it.
    pub lock_free: bool,
    /// Record the canonical transcript Go's fixture records.
    pub transcript: bool,
    pub keep: bool,
    pub oneshot: bool,
    /// The engine's session and transcript it is bound to.
    pub native: Option<(String, String)>,
    /// Authored files in the session directory.
    pub other_files: Vec<String>,
}

fn seed_error(detail: impl ToString) -> LibError {
    LibError::Seed {
        detail: detail.to_string(),
    }
}

impl Wiring {
    /// Plant `seed`; its stable id.
    pub fn seed_session(&self, seed: &Seed) -> Result<String, LibError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| LibError::NoRuntime {
                detail: e.to_string(),
            })?;
        runtime.block_on(self.seed(seed))
    }

    async fn seed(&self, seed: &Seed) -> Result<String, LibError> {
        let sessions = self.sessions();
        let mut identity = sessions.use_cases.identity;
        let mut stores = sessions.use_cases.stores;
        let project = identity
            .for_write(seed.project.as_deref())
            .map_err(seed_error_of)?;
        let store = stores.open(&project).await.map_err(seed_error)?;
        let cwd = self.fs.cwd().map_err(seed_error)?.to_path_buf();
        let output_dir = self
            .home
            .join("Documents/ctxloom")
            .join(cwd.file_name().unwrap_or_default())
            .join(&seed.harp);

        let mut system = System;
        let now = system.now();
        let before = |ago: u64| Timestamp {
            seconds: now.seconds - i64::try_from(ago).unwrap_or(0),
            nanos: 0,
        };
        let id = *uuid::Uuid::now_v7().as_bytes();
        let registry = sessions_application::registry();
        let registry = &registry;
        let session_cover = Cover {
            domain: SESSION_DOMAIN.to_string(),
            root: Some(Uuid::from_bytes(id)),
            edition: None,
        };
        let run_book = |at: Timestamp, book: CommandBook| {
            let sampled = Sampled {
                now: at,
                command_id: System.command_id(),
            };
            async move {
                Kernel::new(registry)
                    .handle::<SessionAggregate, _>(store, book, &sampled)
                    .await
                    .map(drop)
                    .map_err(|failure| seed_error(format!("{failure:?}")))
            }
        };
        let run = |at: Timestamp, command: SessionCommand| {
            let book = CommandBook {
                cover: Some(session_cover.clone()),
                page: Some(CommandPage {
                    body: Some(command_page::Body::Command(Command {
                        payload: Some(command.to_any()),
                    })),
                    ..CommandPage::default()
                }),
                handling: None,
            };
            run_book(at, book)
        };
        let started = before(seed.started_ago);
        // The name first, as the real mint takes it (plan 3.1).
        let claim = NameClaimCommand::from(ClaimName {
            project_id: project.id.clone(),
            harp: seed.harp.clone(),
            session: uuid::Uuid::from_bytes(id).to_string(),
        });
        let claims = sessions_application::registry();
        Kernel::new(&claims)
            .handle::<NameClaimAggregate, _>(
                store,
                CommandBook {
                    cover: Some(Cover {
                        domain: NAMES_DOMAIN.to_string(),
                        root: Some(claim_root(&project.id, &seed.harp)),
                        edition: None,
                    }),
                    page: Some(CommandPage {
                        body: Some(command_page::Body::Command(Command {
                            payload: Some(claim.to_any()),
                        })),
                        ..CommandPage::default()
                    }),
                    handling: None,
                },
                &Sampled {
                    now: started,
                    command_id: System.command_id(),
                },
            )
            .await
            .map_err(|failure| seed_error(format!("{failure:?}")))?;
        run(
            started,
            MintSession {
                harp: seed.harp.clone(),
                project_id: project.id.clone(),
                project_dir: cwd.display().to_string(),
                output_dir: output_dir.display().to_string(),
                engine: "claude-code".to_string(),
                origin: if seed.oneshot {
                    Origin::Oneshot
                } else {
                    Origin::Session
                },
                started_at: Some(started),
                depth: 0,
            }
            .into(),
        )
        .await?;
        if let Some((native_id, transcript_path)) = &seed.native {
            // A bind is a fact (owner ruling 2026-11-04, S5 review item 8).
            run_book(
                before(seed.started_ago.saturating_sub(1)),
                sessions::bind_book(
                    session_cover.clone(),
                    &BindEngineSession {
                        native_id: native_id.clone(),
                        transcript_path: transcript_path.clone(),
                        engine_version: None,
                    },
                ),
            )
            .await?;
        }
        if seed.keep {
            run(now, SetKept { kept: true }.into()).await?;
        }
        if let Some(ago) = seed.ended_ago {
            run(now, RecordEnd { at: before(ago) }.into()).await?;
        }
        self.plant_files(seed)?;
        Ok(uuid::Uuid::from_bytes(id).to_string())
    }

    /// The files a fixture writes beside a session: its lock, its canonical
    /// transcript, authored files; each backdated to the session's end, as
    /// Go's `backdateSessionDir` does, so a session that ENDED then looks
    /// untouched since.
    fn plant_files(&self, seed: &Seed) -> Result<(), LibError> {
        let sessions_dir = self.home.join(".ctxloom/sessions");
        let dir = sessions_dir.join(&seed.harp);
        let mut written = Vec::new();
        let mut write = |path: &Path, body: &str| -> Result<(), LibError> {
            if let Some(parent) = path.parent() {
                self.fs
                    .create_dir_all(parent, Mode::from_bits(0o755))
                    .map_err(seed_error)?;
            }
            self.fs
                .write_atomic(path, body.as_bytes(), Perms::Exact(Mode::from_bits(0o600)))
                .map_err(seed_error)?;
            written.push(path.to_path_buf());
            Ok(())
        };
        if seed.transcript {
            let line = format!(
                r#"{{"v":1,"harp":"{h}","engine":"claude-code","seq":0,"ts":"2026-01-01T00:00:00Z","kind":"entry","entry":{{"type":"user","content":"Seeded prompt for {h}."}}}}"#,
                h = seed.harp
            );
            write(
                &dir.join("transcripts/transcript.jsonl"),
                &format!("{line}\n"),
            )?;
        }
        for rel in &seed.other_files {
            write(&dir.join(rel), "authored\n")?;
        }
        if seed.lock_free {
            let lock = sessions_dir.join(format!("{}.lock", seed.harp));
            if let Some(parent) = lock.parent() {
                self.fs
                    .create_dir_all(parent, Mode::from_bits(0o755))
                    .map_err(seed_error)?;
            }
            self.fs
                .open(&lock, Open::Create(Mode::from_bits(0o600)))
                .map_err(seed_error)?;
            written.push(lock);
        }
        if let Some(ago) = seed.ended_ago {
            let at = SystemTime::now() - Duration::from_secs(ago);
            for path in written {
                self.fs
                    .open(&path, Open::Append(Mode::from_bits(0o600)))
                    .and_then(|file| file.set_modified(at))
                    .map_err(seed_error)?;
            }
        }
        Ok(())
    }
}

/// A report from a port, as a seed error.
fn seed_error_of(report: angzarr_diagnostic::Report) -> LibError {
    seed_error(angzarr_diagnostic::Diagnostic::code(&report))
}
