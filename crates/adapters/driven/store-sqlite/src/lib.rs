//! annals' store port on SQLite: one database per project, opened on a file
//! (or in memory, for tests), migrated on open. Held to annals' store
//! contract, so the kernel behaves over it exactly as over `InMemoryStore`.
//!
//! sqlx runs the queries and the migrations (`migrations/`); every query is
//! built with sea-query from the identifiers in `schema`, never written as a
//! string. Pages are stored as written, never decoded except to read them
//! back; the command id is copied into its own indexed column for the
//! deduplication lookup.

// clippy::result_large_err: annals' StoreError is about 152 bytes, over
// clippy's 128-byte default, so every function here returning it is flagged.
// Allowed with the user's approval: the port's trait methods fix StoreError
// unboxed as their error, annals keeps its errors unboxed on purpose so
// callers can match them (it allows this lint in its own kernel for that
// reason), and these helpers exist to be returned through those methods, so
// boxing them would only be undone at each method.
#![allow(clippy::result_large_err)]

mod schema;

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use annals::next::v1::event_meta::Causation;
use annals::next::v1::{Cover, EventPage, Snapshot, Uuid};
use annals::next::{AsyncEventStore, AsyncStore, AsyncUnitOfWork, Slice, StoreError};
use annals_diagnostic::{Arg, Diagnostic, Report};
use prost::Message;
use sea_query::{Expr, Func, OnConflict, Order, Query, SelectStatement, SqliteQueryBuilder};
use sea_query_binder::SqlxBinder;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection, SqliteRow};
use sqlx::{ConnectOptions, Connection, Row, Sqlite, Transaction};

use schema::{Events, Snapshots};

/// A project's events and snapshots in one SQLite database.
pub struct SqliteStore {
    conn: SqliteConnection,
}

impl SqliteStore {
    /// The database at `path`, created if absent and migrated.
    pub async fn open(path: &Path) -> Result<SqliteStore, StoreError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        SqliteStore::on(options).await
    }

    /// A database held only in memory, migrated.
    pub async fn open_in_memory() -> Result<SqliteStore, StoreError> {
        SqliteStore::on(SqliteConnectOptions::from_str("sqlite::memory:").map_err(unavailable)?)
            .await
    }

    async fn on(options: SqliteConnectOptions) -> Result<SqliteStore, StoreError> {
        let mut conn = options.connect().await.map_err(unavailable)?;
        sqlx::migrate!("./migrations")
            .run(&mut conn)
            .await
            .map_err(unavailable)?;
        Ok(SqliteStore { conn })
    }
}

impl AsyncEventStore for SqliteStore {
    async fn append(
        &mut self,
        stream: &Cover,
        expected: u64,
        pages: &[EventPage],
    ) -> Result<u64, StoreError> {
        // The length check and the inserts are one transaction.
        let mut tx = self.conn.begin().await.map_err(unavailable)?;
        let length = append(&mut tx, stream, expected, pages).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(length)
    }

    async fn read_from(&mut self, stream: &Cover, from: u64) -> Result<Option<Slice>, StoreError> {
        read_from(&mut self.conn, stream, from).await
    }

    async fn find_command(
        &mut self,
        stream: &Cover,
        command_id: &Uuid,
    ) -> Result<Vec<EventPage>, StoreError> {
        find_command(&mut self.conn, stream, command_id).await
    }

    async fn covers(&mut self, domain: &str) -> Result<Vec<Cover>, StoreError> {
        covers(&mut self.conn, domain).await
    }

    async fn read_snapshot(&mut self, stream: &Cover) -> Result<Option<Snapshot>, StoreError> {
        read_snapshot(&mut self.conn, stream).await
    }

    async fn write_snapshot(
        &mut self,
        stream: &Cover,
        snapshot: &Snapshot,
    ) -> Result<(), StoreError> {
        let (domain, edition, root) = key(stream);
        let (sql, values) = Query::insert()
            .into_table(Snapshots::Table)
            .columns([
                Snapshots::Domain,
                Snapshots::Edition,
                Snapshots::Root,
                Snapshots::Snapshot,
            ])
            .values([
                domain.into(),
                edition.into(),
                root.into(),
                snapshot.encode_to_vec().into(),
            ])
            .map_err(unavailable)?
            .on_conflict(
                OnConflict::columns([Snapshots::Domain, Snapshots::Edition, Snapshots::Root])
                    .update_column(Snapshots::Snapshot)
                    .to_owned(),
            )
            .build_sqlx(SqliteQueryBuilder);
        sqlx::query_with(&sql, values)
            .execute(&mut self.conn)
            .await
            .map_err(unavailable)?;
        Ok(())
    }
}

impl AsyncStore for SqliteStore {
    type Unit<'s> = SqliteUnit<'s>;

    /// A unit of work: `BEGIN IMMEDIATE`, so it holds the write lock from
    /// its first read and its length checks cannot go stale. A failure to
    /// begin is answered by every operation on the unit.
    async fn begin(&mut self) -> SqliteUnit<'_> {
        SqliteUnit {
            tx: self
                .conn
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(unavailable),
        }
    }
}

/// A unit of work: one SQLite transaction, committed by
/// [`AsyncUnitOfWork::commit`] and rolled back if dropped.
pub struct SqliteUnit<'s> {
    tx: Result<Transaction<'s, Sqlite>, StoreError>,
}

impl SqliteUnit<'_> {
    /// The transaction's connection, or why the transaction never began.
    fn conn(&mut self) -> Result<&mut SqliteConnection, StoreError> {
        match &mut self.tx {
            Ok(tx) => Ok(&mut **tx),
            Err(error) => Err(error.clone()),
        }
    }
}

impl AsyncEventStore for SqliteUnit<'_> {
    async fn append(
        &mut self,
        stream: &Cover,
        expected: u64,
        pages: &[EventPage],
    ) -> Result<u64, StoreError> {
        append(self.conn()?, stream, expected, pages).await
    }

    async fn read_from(&mut self, stream: &Cover, from: u64) -> Result<Option<Slice>, StoreError> {
        read_from(self.conn()?, stream, from).await
    }

    async fn find_command(
        &mut self,
        stream: &Cover,
        command_id: &Uuid,
    ) -> Result<Vec<EventPage>, StoreError> {
        find_command(self.conn()?, stream, command_id).await
    }

    async fn covers(&mut self, domain: &str) -> Result<Vec<Cover>, StoreError> {
        covers(self.conn()?, domain).await
    }

    async fn read_snapshot(&mut self, stream: &Cover) -> Result<Option<Snapshot>, StoreError> {
        read_snapshot(self.conn()?, stream).await
    }
}

impl AsyncUnitOfWork for SqliteUnit<'_> {
    async fn commit(self) -> Result<(), StoreError> {
        self.tx?.commit().await.map_err(unavailable)
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

/// A select over `stream`'s rows.
fn on_stream(stream: &Cover) -> SelectStatement {
    let (domain, edition, root) = key(stream);
    Query::select()
        .from(Events::Table)
        .and_where(Expr::col(Events::Domain).eq(domain))
        .and_where(Expr::col(Events::Edition).eq(edition))
        .and_where(Expr::col(Events::Root).eq(root))
        .to_owned()
}

/// The stream's length: its pages run from 0 without a gap.
async fn length(conn: &mut SqliteConnection, stream: &Cover) -> Result<u64, StoreError> {
    let (sql, values) = on_stream(stream)
        .expr(Func::count(Expr::col(Events::Sequence)))
        .build_sqlx(SqliteQueryBuilder);
    let row = sqlx::query_with(&sql, values)
        .fetch_one(conn)
        .await
        .map_err(unavailable)?;
    let count: i64 = row.try_get(0).map_err(unavailable)?;
    Ok(count.unsigned_abs())
}

async fn append(
    conn: &mut SqliteConnection,
    stream: &Cover,
    expected: u64,
    pages: &[EventPage],
) -> Result<u64, StoreError> {
    let actual = length(conn, stream).await?;
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
        let (sql, values) = Query::insert()
            .into_table(Events::Table)
            .columns([
                Events::Domain,
                Events::Edition,
                Events::Root,
                Events::Sequence,
                Events::CommandId,
                Events::Page,
            ])
            .values([
                domain.clone().into(),
                edition.clone().into(),
                root.clone().into(),
                sequence_column(page.sequence).into(),
                command_id.into(),
                page.encode_to_vec().into(),
            ])
            .map_err(unavailable)?
            .build_sqlx(SqliteQueryBuilder);
        sqlx::query_with(&sql, values)
            .execute(&mut *conn)
            .await
            .map_err(unavailable)?;
    }
    Ok(actual + pages.len() as u64)
}

async fn read_from(
    conn: &mut SqliteConnection,
    stream: &Cover,
    from: u64,
) -> Result<Option<Slice>, StoreError> {
    let next_sequence = length(conn, stream).await?;
    if next_sequence == 0 {
        return Ok(None);
    }
    let pages = select_pages(
        conn,
        stream,
        on_stream(stream)
            .and_where(Expr::col(Events::Sequence).gte(sequence_column(from)))
            .to_owned(),
    )
    .await?;
    let last = select_pages(
        conn,
        stream,
        on_stream(stream)
            .and_where(Expr::col(Events::Sequence).eq(sequence_column(next_sequence - 1)))
            .to_owned(),
    )
    .await?;
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

async fn find_command(
    conn: &mut SqliteConnection,
    stream: &Cover,
    command_id: &Uuid,
) -> Result<Vec<EventPage>, StoreError> {
    select_pages(
        conn,
        stream,
        on_stream(stream)
            .and_where(Expr::col(Events::CommandId).eq(command_id.value.to_vec()))
            .to_owned(),
    )
    .await
}

/// The pages `select` (a select over `stream`'s rows) matches, in sequence
/// order, decoded.
async fn select_pages(
    conn: &mut SqliteConnection,
    stream: &Cover,
    mut select: SelectStatement,
) -> Result<Vec<EventPage>, StoreError> {
    let (sql, values) = select
        .columns([Events::Sequence, Events::Page])
        .order_by(Events::Sequence, Order::Asc)
        .build_sqlx(SqliteQueryBuilder);
    let rows: Vec<SqliteRow> = sqlx::query_with(&sql, values)
        .fetch_all(conn)
        .await
        .map_err(unavailable)?;
    let mut pages = Vec::with_capacity(rows.len());
    for row in rows {
        let sequence: i64 = row.try_get(0).map_err(unavailable)?;
        let bytes: Vec<u8> = row.try_get(1).map_err(unavailable)?;
        let page = EventPage::decode(bytes.as_slice()).map_err(|e| StoreError::Unreadable {
            stream: stream.clone(),
            sequence: sequence.unsigned_abs(),
            reason: undecodable(e),
        })?;
        pages.push(page);
    }
    Ok(pages)
}

async fn covers(conn: &mut SqliteConnection, domain: &str) -> Result<Vec<Cover>, StoreError> {
    let (sql, values) = Query::select()
        .distinct()
        .columns([Events::Edition, Events::Root])
        .from(Events::Table)
        .and_where(Expr::col(Events::Domain).eq(domain))
        .order_by(Events::Edition, Order::Asc)
        .order_by(Events::Root, Order::Asc)
        .build_sqlx(SqliteQueryBuilder);
    let rows: Vec<SqliteRow> = sqlx::query_with(&sql, values)
        .fetch_all(conn)
        .await
        .map_err(unavailable)?;
    let mut covers = Vec::with_capacity(rows.len());
    for row in rows {
        let edition: String = row.try_get(0).map_err(unavailable)?;
        let root: Vec<u8> = row.try_get(1).map_err(unavailable)?;
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

async fn read_snapshot(
    conn: &mut SqliteConnection,
    stream: &Cover,
) -> Result<Option<Snapshot>, StoreError> {
    let (domain, edition, root) = key(stream);
    let (sql, values) = Query::select()
        .column(Snapshots::Snapshot)
        .from(Snapshots::Table)
        .and_where(Expr::col(Snapshots::Domain).eq(domain))
        .and_where(Expr::col(Snapshots::Edition).eq(edition))
        .and_where(Expr::col(Snapshots::Root).eq(root))
        .build_sqlx(SqliteQueryBuilder);
    let row: Option<SqliteRow> = sqlx::query_with(&sql, values)
        .fetch_optional(conn)
        .await
        .map_err(unavailable)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let bytes: Vec<u8> = row.try_get(0).map_err(unavailable)?;
    Snapshot::decode(bytes.as_slice())
        .map(Some)
        .map_err(|e| StoreError::Unreadable {
            stream: stream.clone(),
            sequence: 0,
            reason: undecodable(e),
        })
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

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
