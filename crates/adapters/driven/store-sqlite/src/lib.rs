//! annals' store port on SQLite: one database per project, opened on a file
//! (or in memory, for tests). Held to annals' store contract, so the kernel
//! behaves over it exactly as over `InMemoryStore`.
//!
//! Pages are stored as written, never decoded except to read them back; the
//! command id is copied into its own indexed column for the deduplication
//! lookup. A stream's main timeline is the edition `''`: SQLite keeps NULLs
//! distinct in a key, and annals refuses an empty edition, so `''` names no
//! real one (and sorts first, as `covers` lists the main timeline first).

// clippy::result_large_err: annals' StoreError is about 152 bytes, over
// clippy's 128-byte default, so every function here returning it is flagged.
// Allowed with the user's approval: the port's trait methods fix StoreError
// unboxed as their error, annals keeps its errors unboxed on purpose so
// callers can match them (it allows this lint in its own kernel for that
// reason), and these helpers exist to be returned through those methods, so
// boxing them would only be undone at each method.
#![allow(clippy::result_large_err)]

use std::fmt;
use std::path::Path;

use annals::next::v1::event_meta::Causation;
use annals::next::v1::{Cover, EventPage, Snapshot, Uuid};
use annals::next::{EventStore, Slice, Store, StoreError, UnitOfWork};
use annals_diagnostic::{Arg, Diagnostic, Report};
use prost::Message;
use rusqlite::{params, Connection, OptionalExtension};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS events (
    domain TEXT NOT NULL,
    edition TEXT NOT NULL,
    root BLOB NOT NULL,
    sequence INTEGER NOT NULL,
    command_id BLOB,
    page BLOB NOT NULL,
    PRIMARY KEY (domain, edition, root, sequence)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS events_by_command
    ON events (domain, edition, root, command_id);
CREATE TABLE IF NOT EXISTS snapshots (
    domain TEXT NOT NULL,
    edition TEXT NOT NULL,
    root BLOB NOT NULL,
    snapshot BLOB NOT NULL,
    PRIMARY KEY (domain, edition, root)
) WITHOUT ROWID;
";

/// A project's events and snapshots in one SQLite database.
pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    /// The database at `path`, created if absent.
    pub fn open(path: &Path) -> Result<SqliteStore, StoreError> {
        SqliteStore::on(Connection::open(path).map_err(unavailable)?)
    }

    /// A database held only in memory.
    pub fn open_in_memory() -> Result<SqliteStore, StoreError> {
        SqliteStore::on(Connection::open_in_memory().map_err(unavailable)?)
    }

    fn on(conn: Connection) -> Result<SqliteStore, StoreError> {
        conn.execute_batch(SCHEMA).map_err(unavailable)?;
        Ok(SqliteStore { conn })
    }
}

impl EventStore for SqliteStore {
    fn append(
        &mut self,
        stream: &Cover,
        expected: u64,
        pages: &[EventPage],
    ) -> Result<u64, StoreError> {
        // The length check and the inserts are one transaction.
        let tx = self.conn.transaction().map_err(unavailable)?;
        let length = append(&tx, stream, expected, pages)?;
        tx.commit().map_err(unavailable)?;
        Ok(length)
    }

    fn read_from(&mut self, stream: &Cover, from: u64) -> Result<Option<Slice>, StoreError> {
        read_from(&self.conn, stream, from)
    }

    fn find_command(
        &mut self,
        stream: &Cover,
        command_id: &Uuid,
    ) -> Result<Vec<EventPage>, StoreError> {
        find_command(&self.conn, stream, command_id)
    }

    fn covers(&mut self, domain: &str) -> Result<Vec<Cover>, StoreError> {
        covers(&self.conn, domain)
    }

    fn read_snapshot(&mut self, stream: &Cover) -> Result<Option<Snapshot>, StoreError> {
        read_snapshot(&self.conn, stream)
    }

    fn write_snapshot(&mut self, stream: &Cover, snapshot: &Snapshot) -> Result<(), StoreError> {
        let (domain, edition, root) = key(stream);
        self.conn
            .execute(
                "INSERT OR REPLACE INTO snapshots (domain, edition, root, snapshot)
                 VALUES (?1, ?2, ?3, ?4)",
                params![domain, edition, root, snapshot.encode_to_vec()],
            )
            .map_err(unavailable)?;
        Ok(())
    }
}

impl Store for SqliteStore {
    type Unit<'s> = SqliteUnit<'s>;

    /// A unit of work: `BEGIN IMMEDIATE`, so it holds the write lock from
    /// its first read and its length checks cannot go stale. A failure to
    /// begin is answered by every operation on the unit.
    fn begin(&mut self) -> SqliteUnit<'_> {
        let failed = self
            .conn
            .execute_batch("BEGIN IMMEDIATE")
            .err()
            .map(unavailable);
        SqliteUnit {
            conn: &mut self.conn,
            failed,
            finished: false,
        }
    }
}

/// A unit of work: one SQLite transaction, committed by
/// [`UnitOfWork::commit`] and rolled back if dropped.
pub struct SqliteUnit<'s> {
    conn: &'s mut Connection,
    /// Why the transaction could not begin, if it could not.
    failed: Option<StoreError>,
    /// Committed: nothing to roll back.
    finished: bool,
}

impl SqliteUnit<'_> {
    /// The connection, or why the transaction never began (each operation
    /// answers that, cloned).
    fn conn(&self) -> Result<&Connection, StoreError> {
        match &self.failed {
            Some(error) => Err(error.clone()),
            None => Ok(self.conn),
        }
    }
}

impl EventStore for SqliteUnit<'_> {
    fn append(
        &mut self,
        stream: &Cover,
        expected: u64,
        pages: &[EventPage],
    ) -> Result<u64, StoreError> {
        append(self.conn()?, stream, expected, pages)
    }

    fn read_from(&mut self, stream: &Cover, from: u64) -> Result<Option<Slice>, StoreError> {
        read_from(self.conn()?, stream, from)
    }

    fn find_command(
        &mut self,
        stream: &Cover,
        command_id: &Uuid,
    ) -> Result<Vec<EventPage>, StoreError> {
        find_command(self.conn()?, stream, command_id)
    }

    fn covers(&mut self, domain: &str) -> Result<Vec<Cover>, StoreError> {
        covers(self.conn()?, domain)
    }

    fn read_snapshot(&mut self, stream: &Cover) -> Result<Option<Snapshot>, StoreError> {
        read_snapshot(self.conn()?, stream)
    }
}

impl UnitOfWork for SqliteUnit<'_> {
    fn commit(mut self) -> Result<(), StoreError> {
        self.conn()?.execute_batch("COMMIT").map_err(unavailable)?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for SqliteUnit<'_> {
    fn drop(&mut self) {
        if !self.finished && self.failed.is_none() {
            // A rollback that fails leaves SQLite to roll the transaction
            // back when the connection closes; nothing here can do more.
            let _ = self.conn.execute_batch("ROLLBACK");
        }
    }
}

// ---------------------------------------------------------------- queries

/// `stream` as its key columns: the main timeline's edition is `''`.
fn key(stream: &Cover) -> (String, String, Vec<u8>) {
    (
        stream.domain.clone(),
        stream.edition.clone().unwrap_or_default(),
        stream
            .root
            .as_ref()
            .map(|r| r.value.to_vec())
            .unwrap_or_default(),
    )
}

/// The stream's length: its pages run from 0 without a gap.
fn length(conn: &Connection, stream: &Cover) -> Result<u64, StoreError> {
    let (domain, edition, root) = key(stream);
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM events WHERE domain = ?1 AND edition = ?2 AND root = ?3",
            params![domain, edition, root],
            |row| row.get(0),
        )
        .map_err(unavailable)?;
    Ok(count.unsigned_abs())
}

fn append(
    conn: &Connection,
    stream: &Cover,
    expected: u64,
    pages: &[EventPage],
) -> Result<u64, StoreError> {
    let actual = length(conn, stream)?;
    if expected != actual {
        return Err(StoreError::Conflict {
            stream: stream.clone(),
            expected,
            actual,
        });
    }
    for (due, page) in (expected..).zip(pages) {
        if page.sequence != due {
            return Err(StoreError::Discontinuous {
                stream: stream.clone(),
                expected: due,
                found: page.sequence,
            });
        }
    }
    let (domain, edition, root) = key(stream);
    for page in pages {
        let command_id = match page.meta.as_ref().and_then(|m| m.causation.as_ref()) {
            Some(Causation::CommandId(id)) => Some(id.value.to_vec()),
            _ => None,
        };
        conn.execute(
            "INSERT INTO events (domain, edition, root, sequence, command_id, page)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                domain,
                edition,
                root,
                sequence_column(page.sequence),
                command_id,
                page.encode_to_vec()
            ],
        )
        .map_err(unavailable)?;
    }
    Ok(actual + pages.len() as u64)
}

fn read_from(conn: &Connection, stream: &Cover, from: u64) -> Result<Option<Slice>, StoreError> {
    let next_sequence = length(conn, stream)?;
    if next_sequence == 0 {
        return Ok(None);
    }
    let pages = select_pages(
        conn,
        stream,
        "AND sequence >= ?4 ORDER BY sequence",
        sequence_column(from),
    )?;
    let last = select_pages(
        conn,
        stream,
        "AND sequence = ?4",
        sequence_column(next_sequence - 1),
    )?;
    Ok(Some(Slice {
        stream: stream.clone(),
        next_sequence,
        last_recorded_at: last
            .first()
            .and_then(|p| p.meta.as_ref())
            .and_then(|m| m.recorded_at),
        pages,
    }))
}

fn find_command(
    conn: &Connection,
    stream: &Cover,
    command_id: &Uuid,
) -> Result<Vec<EventPage>, StoreError> {
    select_pages(
        conn,
        stream,
        "AND command_id = ?4 ORDER BY sequence",
        command_id.value.to_vec(),
    )
}

/// The pages of `stream` matching `filter` (SQL after the stream's key,
/// binding `?4` to `value`), decoded.
fn select_pages(
    conn: &Connection,
    stream: &Cover,
    filter: &str,
    value: impl rusqlite::ToSql,
) -> Result<Vec<EventPage>, StoreError> {
    let (domain, edition, root) = key(stream);
    let sql = format!(
        "SELECT sequence, page FROM events WHERE domain = ?1 AND edition = ?2 AND root = ?3 {filter}"
    );
    let mut statement = conn.prepare(&sql).map_err(unavailable)?;
    let rows = statement
        .query_map(params![domain, edition, root, value], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(unavailable)?;
    let mut pages = Vec::new();
    for row in rows {
        let (sequence, bytes) = row.map_err(unavailable)?;
        let page = EventPage::decode(bytes.as_slice()).map_err(|e| StoreError::Unreadable {
            stream: stream.clone(),
            sequence: sequence.unsigned_abs(),
            reason: undecodable(e),
        })?;
        pages.push(page);
    }
    Ok(pages)
}

fn covers(conn: &Connection, domain: &str) -> Result<Vec<Cover>, StoreError> {
    let mut statement = conn
        .prepare(
            "SELECT DISTINCT edition, root FROM events WHERE domain = ?1 ORDER BY edition, root",
        )
        .map_err(unavailable)?;
    let rows = statement
        .query_map(params![domain], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(unavailable)?;
    let mut covers = Vec::new();
    for row in rows {
        let (edition, root) = row.map_err(unavailable)?;
        let root: [u8; 16] = root
            .as_slice()
            .try_into()
            .map_err(|_| unavailable(format!("a stored root of {} bytes, not 16", root.len())))?;
        covers.push(Cover {
            domain: domain.to_string(),
            root: Some(Uuid::from_bytes(root)),
            edition: (!edition.is_empty()).then_some(edition),
        });
    }
    Ok(covers)
}

fn read_snapshot(conn: &Connection, stream: &Cover) -> Result<Option<Snapshot>, StoreError> {
    let (domain, edition, root) = key(stream);
    let bytes: Option<Vec<u8>> = conn
        .query_row(
            "SELECT snapshot FROM snapshots WHERE domain = ?1 AND edition = ?2 AND root = ?3",
            params![domain, edition, root],
            |row| row.get(0),
        )
        .optional()
        .map_err(unavailable)?;
    bytes
        .map(|b| {
            Snapshot::decode(b.as_slice()).map_err(|e| StoreError::Unreadable {
                stream: stream.clone(),
                sequence: 0,
                reason: undecodable(e),
            })
        })
        .transpose()
}

/// A sequence as SQLite's 64-bit signed integer. Sequences count events in
/// one stream, so they never reach the sign bit.
fn sequence_column(sequence: u64) -> i64 {
    i64::try_from(sequence).unwrap_or(i64::MAX)
}

// ---------------------------------------------------------------- errors

/// Why SQLite could not answer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SqliteError {
    /// SQLite itself failed: the store could not say.
    Unavailable { detail: String },
    /// A stored page's bytes do not decode as an `EventPage`.
    Undecodable { detail: String },
}

impl fmt::Display for SqliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqliteError::Unavailable { detail } => write!(f, "SQLite failed: {detail}"),
            SqliteError::Undecodable { detail } => {
                write!(f, "a stored page does not decode: {detail}")
            }
        }
    }
}

impl Diagnostic for SqliteError {
    fn code(&self) -> &'static str {
        match self {
            SqliteError::Unavailable { .. } => "store-sqlite-unavailable",
            SqliteError::Undecodable { .. } => "store-sqlite-undecodable",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            SqliteError::Unavailable { detail } | SqliteError::Undecodable { detail } => {
                vec![("detail", detail.as_str().into())]
            }
        }
    }
}

/// One value of every error this crate defines, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let detail = || "x".to_string();
    vec![
        Box::new(SqliteError::Unavailable { detail: detail() }),
        Box::new(SqliteError::Undecodable { detail: detail() }),
    ]
}

/// SQLite's failure, as the port's: the store could not say (`Retry`).
fn unavailable(detail: impl fmt::Display) -> StoreError {
    StoreError::Unavailable(Report::of(&SqliteError::Unavailable {
        detail: detail.to_string(),
    }))
}

/// A stored value that does not decode (`Bug`).
fn undecodable(error: prost::DecodeError) -> Report {
    Report::of(&SqliteError::Undecodable {
        detail: error.to_string(),
    })
}
