//! Delivers an engine's claims on its native files under a directory, and
//! gives them back (S9). The ownership record lives in the home's `records/`
//! (never beside the targets, which are foreign files).
//!
//! The promise, in order of strength:
//!
//! - a file ctxloom made is removed once nothing claims anything in it;
//! - a file that was there comes back byte for byte once nothing claims
//!   anything in it, as long as nobody changed it since ctxloom wrote it;
//! - a file somebody changed since has ctxloom's entries cut out where they
//!   are found, and the rest is theirs;
//! - a claim is replaced, never doubled: delivering an engine's kind again is
//!   releasing it and delivering it.

mod record;
mod render;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use surface_ports::{
    Claim, Delivered, ForeignFiles, Inspected, Kind, Released, SurfaceError, SurfaceStore,
};

use record::{FileRecord, Owned, Record};

/// The store over the process's filesystem.
pub struct SurfaceFiles {
    /// The home's `records/` directory.
    records: PathBuf,
}

impl SurfaceFiles {
    /// A store keeping its records in `records`.
    pub fn new(records: impl Into<PathBuf>) -> SurfaceFiles {
        SurfaceFiles {
            records: records.into(),
        }
    }

    fn record_path(&self, target: &str) -> PathBuf {
        let name: String = target
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.records.join(format!("{name}.claims.yaml"))
    }

    fn load(&self, target: &str) -> Result<Record, SurfaceError> {
        let path = self.record_path(target);
        match fs::read_to_string(&path) {
            Ok(text) => Record::decode(&path.display().to_string(), &text),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Record::default()),
            Err(e) => Err(io_error(&path, &e)),
        }
    }

    fn save(&self, target: &str, record: &Record) -> Result<(), SurfaceError> {
        let path = self.record_path(target);
        if record.files.is_empty() {
            return match fs::remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(io_error(&path, &e)),
                _ => Ok(()),
            };
        }
        write(&path, record.encode(target).as_bytes(), false)
    }
}

fn io_error(path: &Path, e: &io::Error) -> SurfaceError {
    SurfaceError::Io {
        file: path.display().to_string(),
        detail: e.to_string(),
    }
}

/// A path below the target, refused where it would leave it.
fn below(target: &str, rel: &str) -> Result<PathBuf, SurfaceError> {
    let path = Path::new(rel);
    let safe = !rel.is_empty()
        && path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
    if !safe {
        return Err(SurfaceError::Refused {
            file: rel.to_string(),
            reason: "it is not a path inside the target".to_string(),
        });
    }
    Ok(Path::new(target).join(path))
}

fn read(path: &Path) -> Result<Option<Vec<u8>>, SurfaceError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(path, &e)),
    }
}

/// `bytes` written whole and atomically, its directories made; an existing
/// file keeps its mode.
fn write(path: &Path, bytes: &[u8], executable: bool) -> Result<(), SurfaceError> {
    let fail = |e: io::Error| io_error(path, &e);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(fail)?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".ctxloom-tmp");
    let temp = PathBuf::from(temp);
    fs::write(&temp, bytes).map_err(fail)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = match fs::metadata(path) {
            Ok(held) if !executable => held.permissions().mode() & 0o777,
            _ if executable => 0o755,
            _ => 0o644,
        };
        fs::set_permissions(&temp, fs::Permissions::from_mode(mode)).map_err(fail)?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    fs::rename(&temp, path).map_err(|e| {
        let _ = fs::remove_file(&temp);
        fail(e)
    })
}

/// A file removed, and the directories it leaves empty (never the target).
fn remove(target: &str, path: &Path) -> Result<(), SurfaceError> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(io_error(path, &e)),
        _ => {}
    }
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d == Path::new(target) || fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
    Ok(())
}

/// A file's settled state after its claims changed.
struct Settled {
    /// The claims the file now carries.
    record: Option<FileRecord>,
    changed: bool,
}

impl SurfaceFiles {
    /// Bring the file `rel` to hold exactly `rec.claims`, writing it unless
    /// `dry`. `rec` is the file's record with the claims already changed;
    /// `prior` the claims it held before.
    fn settle(
        target: &str,
        rel: &str,
        mut rec: FileRecord,
        prior: &[Owned],
        known: bool,
        dry: bool,
        left: &mut Vec<String>,
    ) -> Result<Settled, SurfaceError> {
        let path = below(target, rel)?;
        let current = read(&path)?;
        // What the file held before ctxloom's claims, unless somebody has
        // changed it since: then ctxloom's old entries are cut out of what it
        // holds now.
        let unchanged = known && current.as_deref() == Some(rec.written.as_slice());
        let base: Option<Vec<u8>> = if !known {
            rec.existed = current.is_some();
            current.clone()
        } else if unchanged {
            rec.existed.then(|| rec.original.clone())
        } else if let Some(now) = &current {
            let old: Vec<&Claim> = prior.iter().map(|o| &o.claim).collect();
            let cleaned = render::unrender(rel, now, &old)?;
            rec.existed = true;
            Some(cleaned)
        } else {
            rec.existed = false;
            None
        };
        rec.original = base.clone().unwrap_or_default();

        if rec.claims.is_empty() {
            let changed = match (&base, &current) {
                (Some(base), Some(now)) => base != now,
                (None, Some(_)) => true,
                (Some(_), None) => true,
                (None, None) => false,
            };
            if !dry && changed {
                match &base {
                    Some(bytes) => write(&path, bytes, false)?,
                    None => remove(target, &path)?,
                }
            }
            return Ok(Settled {
                record: None,
                changed,
            });
        }

        let claims: Vec<&Claim> = rec.claims.iter().map(|o| &o.claim).collect();
        let bytes = render::render(rel, base.as_deref(), &claims, left)?;
        let executable = claims.iter().any(|c| {
            matches!(
                c,
                Claim::Whole {
                    executable: true,
                    ..
                }
            )
        });
        let changed = current.as_deref() != Some(bytes.as_slice());
        if !dry && changed {
            write(&path, &bytes, executable)?;
        }
        rec.written = bytes;
        Ok(Settled {
            record: Some(rec),
            changed,
        })
    }

    /// Change the claims of (`engine`, `kind`) to `claims` in the record, and
    /// settle every file that touches.
    fn change(
        &self,
        target: &str,
        engine: &str,
        kind: Kind,
        claims: &[Claim],
        dry: bool,
    ) -> Result<(Vec<String>, Vec<String>, bool), SurfaceError> {
        let mut record = self.load(target)?;
        let mut touched: Vec<String> = Vec::new();
        let mut prior: std::collections::BTreeMap<String, Vec<Owned>> = Default::default();
        // Where each file held the claims being replaced: they go back in
        // that place, so that delivering a kind again leaves the file as it
        // was, whichever kinds were delivered after it.
        let mut slot: std::collections::BTreeMap<String, usize> = Default::default();
        for (rel, file) in &mut record.files {
            let before = file.claims.clone();
            let first = before
                .iter()
                .position(|o| o.engine == engine && o.kind == kind);
            file.claims
                .retain(|o| !(o.engine == engine && o.kind == kind));
            if file.claims.len() != before.len() {
                touched.push(rel.clone());
                prior.insert(rel.clone(), before);
                slot.extend(first.map(|at| (rel.clone(), at)));
            }
        }
        for claim in claims {
            let rel = claim.path().to_string();
            below(target, &rel)?;
            let known = record.files.contains_key(&rel);
            let file = record.files.entry(rel.clone()).or_default();
            prior.entry(rel.clone()).or_insert_with(|| {
                if known {
                    file.claims.clone()
                } else {
                    Vec::new()
                }
            });
            let at = slot
                .get(&rel)
                .map_or(file.claims.len(), |at| (*at).min(file.claims.len()));
            file.claims.insert(
                at,
                Owned {
                    engine: engine.to_string(),
                    kind,
                    claim: claim.clone(),
                },
            );
            slot.insert(rel.clone(), at + 1);
            if !touched.contains(&rel) {
                touched.push(rel);
            }
        }
        let known: Vec<bool> = touched
            .iter()
            .map(|rel| prior.get(rel).is_some_and(|p| !p.is_empty()))
            .collect();
        let mut left = Vec::new();
        let mut changed = false;
        let mut next = record.clone();
        for (rel, known) in touched.iter().zip(known) {
            let rec = record.files.get(rel).cloned().unwrap_or_default();
            let before = prior.get(rel).cloned().unwrap_or_default();
            let settled = Self::settle(target, rel, rec, &before, known, dry, &mut left)?;
            changed |= settled.changed;
            match settled.record {
                Some(rec) => {
                    next.files.insert(rel.clone(), rec);
                }
                None => {
                    next.files.remove(rel);
                }
            }
        }
        if !dry {
            self.save(target, &next)?;
        }
        Ok((touched, left, changed))
    }
}

impl SurfaceStore for SurfaceFiles {
    fn deliver(
        &self,
        target: &str,
        engine: &str,
        kind: Kind,
        claims: &[Claim],
        dry: bool,
    ) -> Result<Delivered, SurfaceError> {
        let (mut files, left, changed) = self.change(target, engine, kind, claims, dry)?;
        files.retain(|f| claims.iter().any(|c| c.path() == f));
        Ok(Delivered {
            files,
            left,
            changed,
        })
    }

    fn release(
        &self,
        target: &str,
        engine: &str,
        kind: Kind,
        dry: bool,
    ) -> Result<Released, SurfaceError> {
        let (files, _, changed) = self.change(target, engine, kind, &[], dry)?;
        Ok(Released { files, changed })
    }

    fn claimed(&self, target: &str, engine: &str) -> Result<Vec<Kind>, SurfaceError> {
        let record = self.load(target)?;
        let mut kinds: Vec<Kind> = record
            .files
            .values()
            .flat_map(|f| f.claims.iter())
            .filter(|o| o.engine == engine)
            .map(|o| o.kind)
            .collect();
        kinds.sort();
        kinds.dedup();
        Ok(kinds)
    }

    fn read(&self, target: &str, rel: &str) -> Result<Option<Vec<u8>>, SurfaceError> {
        read(&below(target, rel)?)
    }

    fn read_absolute(&self, path: &str) -> Result<Option<Vec<u8>>, SurfaceError> {
        read(Path::new(path))
    }

    fn inspect(&self, target: &str) -> Result<Inspected, SurfaceError> {
        match fs::metadata(target) {
            Ok(meta) if meta.is_dir() => Ok(Inspected::Directory),
            Ok(_) => Ok(Inspected::Other),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Inspected::Missing),
            Err(e) => Err(io_error(Path::new(target), &e)),
        }
    }
}

impl ForeignFiles for SurfaceFiles {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, SurfaceError> {
        read(Path::new(path))
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), SurfaceError> {
        write(Path::new(path), bytes, false)
    }

    fn record(&self, path: &str) -> Result<Option<String>, SurfaceError> {
        let at = self.foreign_record(path);
        match fs::read_to_string(&at) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_error(&at, &e)),
        }
    }

    fn write_record(&self, path: &str, text: &str) -> Result<String, SurfaceError> {
        let at = self.foreign_record(path);
        write(&at, text.as_bytes(), false)?;
        Ok(at.display().to_string())
    }
}

impl SurfaceFiles {
    fn foreign_record(&self, path: &str) -> PathBuf {
        let name: String = path
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.records.join(format!("{name}.hew-record.yaml"))
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
