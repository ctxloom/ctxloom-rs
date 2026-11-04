//! A skill package as a zip (Go's `bundles/skill_archive.go`): the archive
//! Anthropic's Skills API takes, one top-level directory named after the
//! skill holding every file of the package.
//!
//! An archive is ADVERSARIAL INPUT: it may come from an untrusted remote or a
//! hostile publisher, and a skill's scripts are executable once materialized.
//! [`unpack`] is the one door every byte passes through, and it refuses before
//! it returns a single file: an absolute path, a `..` segment, a backslash, a
//! link, a device or other special file, a second top-level name, a path
//! twice, more entries than [`Limits::max_entries`], more bytes than
//! [`Limits::max_total_bytes`] (counted as they are decompressed: the sizes an
//! archive declares are never trusted), a checksum that does not match, and
//! what is not a zip. An entry's mode gives one bit of what it carries, whether
//! it is executable; setuid, setgid, sticky and write bits are dropped.
//! This build reads zips only (not Go's liberal tar.gz).

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

/// One file of a package.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Entry {
    /// Relative to the package directory, slash separated.
    pub path: String,
    pub bytes: Vec<u8>,
    /// Whether it carries an execute bit in the archive.
    pub executable: bool,
}

/// What [`unpack`] will take.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub max_entries: usize,
    pub max_total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_entries: 4096,
            max_total_bytes: 30 * 1024 * 1024,
        }
    }
}

/// A package unpacked: its top-level directory's name and its files, sorted by
/// path.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Unpacked {
    pub top: String,
    pub entries: Vec<Entry>,
}

/// Why an archive was not packed or taken.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ArchiveError {
    /// Nothing to pack.
    NothingToPack { name: String },
    /// Not a zip.
    Unrecognized,
    /// A zip that does not read.
    Invalid { detail: String },
    /// A zip64 archive (this build does not read them).
    Zip64,
    /// An encrypted entry.
    Encrypted { name: String },
    /// An entry compressed some way other than stored or deflated.
    Method { name: String, method: u16 },
    /// More entries than allowed.
    TooManyEntries { max: usize },
    /// More bytes than allowed once decompressed.
    TooBig { max: u64 },
    /// An absolute path.
    Absolute { name: String },
    /// A `..` segment.
    Traversal { name: String },
    /// A name that is no path (empty segment, backslash, NUL, not UTF-8).
    BadName { name: String },
    /// A symbolic link.
    Link { name: String },
    /// A device, a FIFO or a socket.
    Special { name: String },
    /// The same path twice.
    Duplicate { name: String },
    /// An entry outside the one top-level directory.
    ForeignTop { name: String, top: String },
    /// No entry at all.
    Empty,
    /// A top-level directory and no file under it.
    NoFiles { top: String },
    /// An entry whose bytes do not match its checksum.
    Checksum { name: String },
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for ArchiveError {}

impl Diagnostic for ArchiveError {
    fn code(&self) -> &str {
        match self {
            ArchiveError::NothingToPack { .. } => code!("archive-nothing-to-pack"),
            ArchiveError::Unrecognized => code!("archive-unrecognized"),
            ArchiveError::Invalid { .. } => code!("archive-invalid"),
            ArchiveError::Zip64 => code!("archive-zip64"),
            ArchiveError::Encrypted { .. } => code!("archive-encrypted"),
            ArchiveError::Method { .. } => code!("archive-method"),
            ArchiveError::TooManyEntries { .. } => code!("archive-too-many-entries"),
            ArchiveError::TooBig { .. } => code!("archive-too-big"),
            ArchiveError::Absolute { .. } => code!("archive-absolute"),
            ArchiveError::Traversal { .. } => code!("archive-traversal"),
            ArchiveError::BadName { .. } => code!("archive-bad-name"),
            ArchiveError::Link { .. } => code!("archive-link"),
            ArchiveError::Special { .. } => code!("archive-special"),
            ArchiveError::Duplicate { .. } => code!("archive-duplicate"),
            ArchiveError::ForeignTop { .. } => code!("archive-foreign-top"),
            ArchiveError::Empty => code!("archive-empty"),
            ArchiveError::NoFiles { .. } => code!("archive-no-files"),
            ArchiveError::Checksum { .. } => code!("archive-checksum"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            ArchiveError::NothingToPack { name }
            | ArchiveError::Encrypted { name }
            | ArchiveError::Absolute { name }
            | ArchiveError::Traversal { name }
            | ArchiveError::BadName { name }
            | ArchiveError::Link { name }
            | ArchiveError::Special { name }
            | ArchiveError::Duplicate { name }
            | ArchiveError::Checksum { name } => vec![("name", name.as_str().into())],
            ArchiveError::Invalid { detail } => vec![("detail", detail.as_str().into())],
            ArchiveError::Method { name, method } => vec![
                ("name", name.as_str().into()),
                ("method", u64::from(*method).into()),
            ],
            ArchiveError::TooManyEntries { max } => vec![("max", (*max as u64).into())],
            ArchiveError::TooBig { max } => vec![("max", (*max).into())],
            ArchiveError::ForeignTop { name, top } => {
                vec![("name", name.as_str().into()), ("top", top.as_str().into())]
            }
            ArchiveError::NoFiles { top } => vec![("top", top.as_str().into())],
            ArchiveError::Unrecognized | ArchiveError::Zip64 | ArchiveError::Empty => Vec::new(),
        }
    }
}

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;
const UTF8_NAMES: u16 = 0x0800;
const ENCRYPTED: u16 = 1;
/// 1980-01-01, the zip epoch: every entry carries it, so a package packs to
/// the same bytes whenever it is packed.
const DOS_DATE: u16 = 0x0021;
/// Unix, zip specification 2.0.
const MADE_BY_UNIX: u16 = (3 << 8) | 20;
const STORED: u16 = 0;
const DEFLATED: u16 = 8;
const FILE_TYPE: u32 = 0o170_000;
const REGULAR: u32 = 0o100_000;
const DIRECTORY: u32 = 0o040_000;
const SYMLINK: u32 = 0o120_000;

fn invalid(detail: &str) -> ArchiveError {
    ArchiveError::Invalid {
        detail: detail.to_string(),
    }
}

/// A length that fits a classic (non-zip64) header field.
fn small(len: usize, what: &str) -> Result<u32, ArchiveError> {
    u32::try_from(len)
        .ok()
        .filter(|n| *n != u32::MAX)
        .ok_or_else(|| invalid(&format!("{what} is too large for a zip that is not zip64")))
}

fn deflated(bytes: &[u8]) -> Result<Vec<u8>, ArchiveError> {
    use std::io::Write;
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(bytes)
        .and_then(|()| encoder.finish())
        .map_err(|e| invalid(&e.to_string()))
}

/// Pack `entries` as the package `name`: one top-level directory `name`, the
/// files below it in path order, deflated; modes 0755 for an executable file
/// and 0644 for the rest; a fixed modification time, so the same package packs
/// to the same bytes.
pub fn pack(name: &str, entries: &[Entry]) -> Result<Vec<u8>, ArchiveError> {
    if entries.is_empty() {
        return Err(ArchiveError::NothingToPack {
            name: name.to_string(),
        });
    }
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let count = u16::try_from(sorted.len())
        .ok()
        .filter(|n| *n != u16::MAX)
        .ok_or_else(|| invalid("too many entries for a zip that is not zip64"))?;
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for entry in sorted {
        let full = format!("{name}/{}", entry.path);
        let (method, data) = if entry.bytes.is_empty() {
            (STORED, Vec::new())
        } else {
            (DEFLATED, deflated(&entry.bytes)?)
        };
        let crc = crc32fast::hash(&entry.bytes);
        let (csize, usize_) = (small(data.len(), &full)?, small(entry.bytes.len(), &full)?);
        let name_len = u16::try_from(full.len()).map_err(|_| invalid("a name is too long"))?;
        let offset = small(out.len(), "the archive")?;
        out.extend(LOCAL.to_le_bytes());
        out.extend(20u16.to_le_bytes());
        out.extend(UTF8_NAMES.to_le_bytes());
        out.extend(method.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(DOS_DATE.to_le_bytes());
        out.extend(crc.to_le_bytes());
        out.extend(csize.to_le_bytes());
        out.extend(usize_.to_le_bytes());
        out.extend(name_len.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(full.as_bytes());
        out.extend(&data);
        let mode = REGULAR | if entry.executable { 0o755 } else { 0o644 };
        central.extend(CENTRAL.to_le_bytes());
        central.extend(MADE_BY_UNIX.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        central.extend(UTF8_NAMES.to_le_bytes());
        central.extend(method.to_le_bytes());
        central.extend(0u16.to_le_bytes());
        central.extend(DOS_DATE.to_le_bytes());
        central.extend(crc.to_le_bytes());
        central.extend(csize.to_le_bytes());
        central.extend(usize_.to_le_bytes());
        central.extend(name_len.to_le_bytes());
        central.extend([0u8; 8]);
        central.extend((mode << 16).to_le_bytes());
        central.extend(offset.to_le_bytes());
        central.extend(full.as_bytes());
    }
    let start = small(out.len(), "the archive")?;
    let size = small(central.len(), "the central directory")?;
    out.extend(&central);
    out.extend(END.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend(count.to_le_bytes());
    out.extend(count.to_le_bytes());
    out.extend(size.to_le_bytes());
    out.extend(start.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    Ok(out)
}

/// A reader of little-endian fields that refuses to run past the archive.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn get(&self, at: usize, len: usize) -> Result<&[u8], ArchiveError> {
        at.checked_add(len)
            .and_then(|end| self.0.get(at..end))
            .ok_or_else(|| invalid("an entry runs past the end of the archive"))
    }

    fn u16(&self, at: usize) -> Result<u16, ArchiveError> {
        Ok(u16::from_le_bytes([
            self.get(at, 2)?[0],
            self.get(at, 2)?[1],
        ]))
    }

    fn u32(&self, at: usize) -> Result<u32, ArchiveError> {
        let b = self.get(at, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// What the central directory says of one entry.
struct Central {
    name: String,
    flags: u16,
    method: u16,
    crc: u32,
    compressed: usize,
    unix_mode: Option<u32>,
    offset: usize,
}

/// The end-of-central-directory record: the last 22 bytes, or the last place
/// the signature is within a comment's reach.
fn end_record(bytes: &Bytes<'_>) -> Result<usize, ArchiveError> {
    let len = bytes.0.len();
    let last = len
        .checked_sub(22)
        .ok_or_else(|| invalid("no end of central directory"))?;
    let first = last.saturating_sub(usize::from(u16::MAX));
    (first..=last)
        .rev()
        .find(|at| bytes.u32(*at).is_ok_and(|sig| sig == END))
        .ok_or_else(|| invalid("no end of central directory"))
}

fn central_entries(bytes: &Bytes<'_>, limits: &Limits) -> Result<Vec<Central>, ArchiveError> {
    let end = end_record(bytes)?;
    let total = bytes.u16(end + 10)?;
    let size = bytes.u32(end + 12)?;
    let start = bytes.u32(end + 16)?;
    if total == u16::MAX || size == u32::MAX || start == u32::MAX {
        return Err(ArchiveError::Zip64);
    }
    if usize::from(total) > limits.max_entries {
        return Err(ArchiveError::TooManyEntries {
            max: limits.max_entries,
        });
    }
    let mut at = start as usize;
    let mut out = Vec::with_capacity(usize::from(total));
    for _ in 0..total {
        if bytes.u32(at)? != CENTRAL {
            return Err(invalid("a central directory entry is damaged"));
        }
        let made_by = bytes.u16(at + 4)?;
        let flags = bytes.u16(at + 8)?;
        let method = bytes.u16(at + 10)?;
        let crc = bytes.u32(at + 16)?;
        let compressed = bytes.u32(at + 20)?;
        let uncompressed = bytes.u32(at + 24)?;
        let name_len = usize::from(bytes.u16(at + 28)?);
        let extra_len = usize::from(bytes.u16(at + 30)?);
        let comment_len = usize::from(bytes.u16(at + 32)?);
        let attrs = bytes.u32(at + 38)?;
        let offset = bytes.u32(at + 42)?;
        let raw = bytes.get(at + 46, name_len)?;
        let name = String::from_utf8(raw.to_vec()).map_err(|_| ArchiveError::BadName {
            name: String::from_utf8_lossy(raw).into_owned(),
        })?;
        if compressed == u32::MAX || uncompressed == u32::MAX || offset == u32::MAX {
            return Err(ArchiveError::Zip64);
        }
        out.push(Central {
            name,
            flags,
            method,
            crc,
            compressed: compressed as usize,
            unix_mode: (made_by >> 8 == 3).then_some(attrs >> 16),
            offset: offset as usize,
        });
        at += 46 + name_len + extra_len + comment_len;
    }
    Ok(out)
}

#[derive(PartialEq, Eq)]
enum Kind {
    File,
    Dir,
}

fn kind_of(entry: &Central) -> Result<Kind, ArchiveError> {
    let named_dir = entry.name.ends_with('/');
    let Some(mode) = entry.unix_mode else {
        return Ok(if named_dir { Kind::Dir } else { Kind::File });
    };
    match mode & FILE_TYPE {
        SYMLINK => Err(ArchiveError::Link {
            name: entry.name.clone(),
        }),
        DIRECTORY => Ok(Kind::Dir),
        REGULAR | 0 => Ok(if named_dir { Kind::Dir } else { Kind::File }),
        _ => Err(ArchiveError::Special {
            name: entry.name.clone(),
        }),
    }
}

/// The segments of an entry's name, or why it is no path a package may hold.
fn segments(name: &str, dir: bool) -> Result<Vec<&str>, ArchiveError> {
    let bad = || ArchiveError::BadName {
        name: name.to_string(),
    };
    let bytes = name.as_bytes();
    if name.starts_with('/')
        || (bytes.len() > 1 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic())
    {
        return Err(ArchiveError::Absolute {
            name: name.to_string(),
        });
    }
    if name.contains(['\\', '\0']) {
        return Err(bad());
    }
    let trimmed = if dir {
        name.strip_suffix('/').unwrap_or(name)
    } else {
        name
    };
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.contains(&"..") {
        return Err(ArchiveError::Traversal {
            name: name.to_string(),
        });
    }
    if parts.iter().any(|p| p.is_empty() || *p == ".") {
        return Err(bad());
    }
    Ok(parts)
}

/// The bytes of one entry, at most `budget` of them once decompressed.
fn contents(
    bytes: &Bytes<'_>,
    entry: &Central,
    budget: u64,
    max: u64,
) -> Result<Vec<u8>, ArchiveError> {
    use std::io::Read;
    if bytes.u32(entry.offset)? != LOCAL {
        return Err(invalid("a local file header is damaged"));
    }
    let name_len = usize::from(bytes.u16(entry.offset + 26)?);
    let extra_len = usize::from(bytes.u16(entry.offset + 28)?);
    let data = bytes.get(entry.offset + 30 + name_len + extra_len, entry.compressed)?;
    let too_big = || ArchiveError::TooBig { max };
    let out = if entry.method == STORED {
        if data.len() as u64 > budget {
            return Err(too_big());
        }
        data.to_vec()
    } else {
        let mut out = Vec::new();
        flate2::read::DeflateDecoder::new(data)
            .take(budget.saturating_add(1))
            .read_to_end(&mut out)
            .map_err(|e| invalid(&e.to_string()))?;
        if out.len() as u64 > budget {
            return Err(too_big());
        }
        out
    };
    if crc32fast::hash(&out) != entry.crc {
        return Err(ArchiveError::Checksum {
            name: entry.name.clone(),
        });
    }
    Ok(out)
}

/// Take the package out of `archive`, or refuse it (see the module's rules).
pub fn unpack(archive: &[u8], limits: &Limits) -> Result<Unpacked, ArchiveError> {
    let bytes = Bytes(archive);
    let magic = archive.get(..4).unwrap_or_default();
    if !matches!(magic, b"PK\x03\x04" | b"PK\x05\x06" | b"PK\x07\x08") {
        return Err(ArchiveError::Unrecognized);
    }
    let central = central_entries(&bytes, limits)?;
    let mut top: Option<String> = None;
    let mut files: Vec<Entry> = Vec::new();
    let mut total: u64 = 0;
    for entry in &central {
        if entry.flags & ENCRYPTED != 0 {
            return Err(ArchiveError::Encrypted {
                name: entry.name.clone(),
            });
        }
        if entry.method != STORED && entry.method != DEFLATED {
            return Err(ArchiveError::Method {
                name: entry.name.clone(),
                method: entry.method,
            });
        }
        let kind = kind_of(entry)?;
        let parts = segments(&entry.name, kind == Kind::Dir)?;
        let first = parts[0];
        match &top {
            None => top = Some(first.to_string()),
            Some(t) if t != first => {
                return Err(ArchiveError::ForeignTop {
                    name: entry.name.clone(),
                    top: t.clone(),
                })
            }
            Some(_) => {}
        }
        if kind == Kind::Dir || parts.len() == 1 {
            continue;
        }
        let data = contents(
            &bytes,
            entry,
            limits.max_total_bytes - total,
            limits.max_total_bytes,
        )?;
        total += data.len() as u64;
        let path = parts[1..].join("/");
        if files.iter().any(|f| f.path == path) {
            return Err(ArchiveError::Duplicate {
                name: entry.name.clone(),
            });
        }
        files.push(Entry {
            path,
            bytes: data,
            executable: entry.unix_mode.is_some_and(|m| m & 0o111 != 0),
        });
    }
    let top = top.ok_or(ArchiveError::Empty)?;
    if files.is_empty() {
        return Err(ArchiveError::NoFiles { top });
    }
    if let Some(clash) = files.iter().find(|f| {
        files
            .iter()
            .any(|g| g.path.starts_with(&format!("{}/", f.path)))
    }) {
        return Err(ArchiveError::Duplicate {
            name: format!("{top}/{}", clash.path),
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Unpacked {
        top,
        entries: files,
    })
}

/// One value of every error here, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let name = || "reviewer/x".to_string();
    vec![
        Box::new(ArchiveError::NothingToPack {
            name: "reviewer".to_string(),
        }),
        Box::new(ArchiveError::Unrecognized),
        Box::new(ArchiveError::Invalid {
            detail: "truncated".to_string(),
        }),
        Box::new(ArchiveError::Zip64),
        Box::new(ArchiveError::Encrypted { name: name() }),
        Box::new(ArchiveError::Method {
            name: name(),
            method: 12,
        }),
        Box::new(ArchiveError::TooManyEntries { max: 4096 }),
        Box::new(ArchiveError::TooBig { max: 1024 }),
        Box::new(ArchiveError::Absolute { name: name() }),
        Box::new(ArchiveError::Traversal { name: name() }),
        Box::new(ArchiveError::BadName { name: name() }),
        Box::new(ArchiveError::Link { name: name() }),
        Box::new(ArchiveError::Special { name: name() }),
        Box::new(ArchiveError::Duplicate { name: name() }),
        Box::new(ArchiveError::ForeignTop {
            name: name(),
            top: "reviewer".to_string(),
        }),
        Box::new(ArchiveError::Empty),
        Box::new(ArchiveError::NoFiles {
            top: "reviewer".to_string(),
        }),
        Box::new(ArchiveError::Checksum { name: name() }),
    ]
}

#[cfg(test)]
#[path = "archive.test.rs"]
mod tests;
