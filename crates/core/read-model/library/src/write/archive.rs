//! A skill package as a zip (Go's `bundles/skill_archive.go`): the archive
//! Anthropic's Skills API takes, one top-level directory named after the
//! skill holding every file of the package.
//!
//! An archive is ADVERSARIAL INPUT: it may come from an untrusted remote or a
//! hostile publisher, and a skill's scripts are executable once materialized.
//! [`unpack`] is the one door every byte passes through, and it refuses before
//! it returns a single file: an absolute path, a `..` segment, a backslash, a
//! link, a device or other special file, a second top-level name, a path
//! twice (or one entry's bytes under two names), more entries than [`Limits::max_entries`], more bytes than
//! [`Limits::max_total_bytes`] (counted as they are decompressed: the sizes an
//! archive declares are never trusted), a checksum that does not match, and
//! what is not a zip, an encrypted entry, and a method other than stored or
//! deflate. An entry's mode gives one bit of what it carries, whether it is
//! executable; setuid, setgid, sticky and write bits are dropped. zip64 is
//! read like any other zip: the limits, not the container's width, are the
//! defence. This build reads zips only (not Go's liberal tar.gz, which Go's
//! feature files do not document).
//!
//! The container is the `zip` crate (deflate only); this module is the policy
//! over it: the checks above, the entry sizes and checksum counted here rather
//! than trusted, and every name checked as the library reports it (an Info-ZIP
//! Unicode-path extra field may replace the header's, and is checked the same).

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
    /// The central directory names one entry twice.
    Repeated,
    /// Two entries share bytes of the archive.
    Overlap,
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
            ArchiveError::Repeated => code!("archive-repeated"),
            ArchiveError::Overlap => code!("archive-overlap"),
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
            ArchiveError::Unrecognized
            | ArchiveError::Repeated
            | ArchiveError::Overlap
            | ArchiveError::Empty => Vec::new(),
        }
    }
}

const FILE_TYPE: u32 = 0o170_000;
const REGULAR: u32 = 0o100_000;
const DIRECTORY: u32 = 0o040_000;
const SYMLINK: u32 = 0o120_000;

fn invalid(detail: impl ToString) -> ArchiveError {
    ArchiveError::Invalid {
        detail: detail.to_string(),
    }
}

/// Pack `entries` as the package `name`: one top-level directory `name`, the
/// files below it in path order, deflated; modes 0755 for an executable file
/// and 0644 for the rest; a fixed modification time (the zip epoch, 1980-01-01),
/// so the same package packs to the same bytes.
pub fn pack(name: &str, entries: &[Entry]) -> Result<Vec<u8>, ArchiveError> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    if entries.is_empty() {
        return Err(ArchiveError::NothingToPack {
            name: name.to_string(),
        });
    }
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for entry in sorted {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(if entry.executable { 0o755 } else { 0o644 })
            .large_file(entry.bytes.len() as u64 >= u64::from(u32::MAX));
        writer
            .start_file(format!("{name}/{}", entry.path), options)
            .map_err(invalid)?;
        writer.write_all(&entry.bytes).map_err(invalid)?;
    }
    Ok(writer.finish().map_err(invalid)?.into_inner())
}

#[derive(PartialEq, Eq)]
enum Kind {
    File,
    Dir,
}

fn kind_of(name: &str, unix_mode: Option<u32>) -> Result<Kind, ArchiveError> {
    let named_dir = name.ends_with('/');
    let Some(mode) = unix_mode else {
        return Ok(if named_dir { Kind::Dir } else { Kind::File });
    };
    match mode & FILE_TYPE {
        SYMLINK => Err(ArchiveError::Link {
            name: name.to_string(),
        }),
        DIRECTORY => Ok(Kind::Dir),
        REGULAR | 0 => Ok(if named_dir { Kind::Dir } else { Kind::File }),
        _ => Err(ArchiveError::Special {
            name: name.to_string(),
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

/// What the central directory says of one entry, taken before any of its bytes
/// are read.
struct Meta {
    name: String,
    encrypted: bool,
    method: zip::CompressionMethod,
    unix_mode: Option<u32>,
    declared: u64,
    crc: u32,
}

fn meta_of(
    archive: &zip::ZipArchive<std::io::Cursor<&[u8]>>,
    index: usize,
) -> Result<Meta, ArchiveError> {
    let entry = archive.by_index_data(index).map_err(invalid)?;
    let raw = entry.name_raw();
    let name = String::from_utf8(raw.to_vec()).map_err(|_| ArchiveError::BadName {
        name: String::from_utf8_lossy(raw).into_owned(),
    })?;
    Ok(Meta {
        name,
        encrypted: entry.encrypted(),
        method: entry.compression(),
        unix_mode: entry.unix_mode(),
        declared: entry.size(),
        crc: entry.crc32(),
    })
}

/// The method's number, for a refusal that names it.
#[allow(deprecated)]
fn method_number(method: zip::CompressionMethod) -> u16 {
    match method {
        zip::CompressionMethod::Unsupported(n) => n,
        // Compiled out: only stored and deflate are, so what else arrives
        // here is a method this build never reads.
        _ => u16::MAX,
    }
}

/// How a library failure to open an archive reads here: two of the library's
/// refusals have their own words.
fn opened(error: &zip::result::ZipError) -> ArchiveError {
    let text = error.to_string();
    if text.contains("duplicate file names") {
        ArchiveError::Repeated
    } else if text.contains("overlapping entries") {
        ArchiveError::Overlap
    } else {
        invalid(text)
    }
}

/// The bytes of one entry, at most `budget` of them once decompressed.
fn contents(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    index: usize,
    meta: &Meta,
    budget: u64,
    max: u64,
) -> Result<Vec<u8>, ArchiveError> {
    use std::io::Read;
    let too_big = || ArchiveError::TooBig { max };
    if meta.declared > budget {
        return Err(too_big());
    }
    let options = zip::read::ZipReadOptions::new().ignore_crc32(true);
    let file = archive
        .by_index_with_options(index, options)
        .map_err(invalid)?;
    let mut out = Vec::new();
    file.take(budget.saturating_add(1))
        .read_to_end(&mut out)
        .map_err(invalid)?;
    if out.len() as u64 > budget {
        return Err(too_big());
    }
    if crc32fast::hash(&out) != meta.crc {
        return Err(ArchiveError::Checksum {
            name: meta.name.clone(),
        });
    }
    Ok(out)
}

/// Take the package out of `archive`, or refuse it (see the module's rules).
pub fn unpack(archive: &[u8], limits: &Limits) -> Result<Unpacked, ArchiveError> {
    let magic = archive.get(..4).unwrap_or_default();
    if !matches!(magic, b"PK\x03\x04" | b"PK\x05\x06" | b"PK\x07\x08") {
        return Err(ArchiveError::Unrecognized);
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).map_err(|e| opened(&e))?;
    if zip.len() > limits.max_entries {
        return Err(ArchiveError::TooManyEntries {
            max: limits.max_entries,
        });
    }
    zip.validate_header_consistency().map_err(|e| opened(&e))?;
    let mut top: Option<String> = None;
    let mut files: Vec<Entry> = Vec::new();
    let mut total: u64 = 0;
    for index in 0..zip.len() {
        let meta = meta_of(&zip, index)?;
        if meta.encrypted {
            return Err(ArchiveError::Encrypted { name: meta.name });
        }
        if !matches!(
            meta.method,
            zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
        ) {
            return Err(ArchiveError::Method {
                name: meta.name,
                method: method_number(meta.method),
            });
        }
        let kind = kind_of(&meta.name, meta.unix_mode)?;
        let parts = segments(&meta.name, kind == Kind::Dir)?;
        let first = parts[0];
        match &top {
            None => top = Some(first.to_string()),
            Some(t) if t != first => {
                return Err(ArchiveError::ForeignTop {
                    name: meta.name.clone(),
                    top: t.clone(),
                })
            }
            Some(_) => {}
        }
        if kind == Kind::Dir || parts.len() == 1 {
            continue;
        }
        let data = contents(
            &mut zip,
            index,
            &meta,
            limits.max_total_bytes - total,
            limits.max_total_bytes,
        )?;
        total += data.len() as u64;
        let path = parts[1..].join("/");
        if files.iter().any(|f| f.path == path) {
            return Err(ArchiveError::Duplicate { name: meta.name });
        }
        files.push(Entry {
            path,
            bytes: data,
            executable: meta.unix_mode.is_some_and(|m| m & 0o111 != 0),
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
        Box::new(ArchiveError::Repeated),
        Box::new(ArchiveError::Overlap),
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
