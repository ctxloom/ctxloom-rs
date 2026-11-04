use super::*;

fn entry(path: &str, bytes: &[u8], executable: bool) -> Entry {
    Entry {
        path: path.to_string(),
        bytes: bytes.to_vec(),
        executable,
    }
}

fn package() -> Vec<Entry> {
    vec![
        entry("SKILL.md", b"---\nname: reviewer\n---\nbody", false),
        entry("scripts/run.sh", b"#!/bin/sh\necho hi\n", true),
        entry("assets/blob.bin", &[0, 255, 1, 254, 10, 13, 0], false),
    ]
}

// ---- hand-built archives: the hostile ones are not ones `pack` would make

struct Raw<'a> {
    name: &'a str,
    /// The unix mode in the external attributes (with the file type bits);
    /// none for an archive made by a host that keeps no unix mode.
    mode: Option<u32>,
    method: u16,
    /// What is stored (already deflated for method 8).
    stored: Vec<u8>,
    /// The size the headers claim.
    claimed_size: u32,
    crc: u32,
    flags: u16,
    /// Extra fields of the local record and of the central record.
    local_extra: Vec<u8>,
    central_extra: Vec<u8>,
}

impl<'a> Raw<'a> {
    fn file(name: &'a str, data: &[u8]) -> Raw<'a> {
        Raw {
            name,
            mode: Some(0o100_644),
            method: 0,
            stored: data.to_vec(),
            claimed_size: data.len() as u32,
            crc: crc32fast::hash(data),
            flags: 0x0800,
            local_extra: Vec::new(),
            central_extra: Vec::new(),
        }
    }

    fn mode(mut self, mode: u32) -> Raw<'a> {
        self.mode = Some(mode);
        self
    }
}

fn zip(entries: &[Raw<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for e in entries {
        let offset = out.len() as u32;
        out.extend(0x0403_4b50u32.to_le_bytes());
        out.extend(20u16.to_le_bytes());
        out.extend(e.flags.to_le_bytes());
        out.extend(e.method.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(0x21u16.to_le_bytes());
        out.extend(e.crc.to_le_bytes());
        out.extend((e.stored.len() as u32).to_le_bytes());
        out.extend(e.claimed_size.to_le_bytes());
        out.extend((e.name.len() as u16).to_le_bytes());
        out.extend((e.local_extra.len() as u16).to_le_bytes());
        out.extend(e.name.as_bytes());
        out.extend(&e.local_extra);
        out.extend(&e.stored);
        central.extend(0x0201_4b50u32.to_le_bytes());
        let host: u16 = if e.mode.is_some() { 3 } else { 0 };
        central.extend(((host << 8) | 20).to_le_bytes());
        central.extend(20u16.to_le_bytes());
        central.extend(e.flags.to_le_bytes());
        central.extend(e.method.to_le_bytes());
        central.extend(0u16.to_le_bytes());
        central.extend(0x21u16.to_le_bytes());
        central.extend(e.crc.to_le_bytes());
        central.extend((e.stored.len() as u32).to_le_bytes());
        central.extend(e.claimed_size.to_le_bytes());
        central.extend((e.name.len() as u16).to_le_bytes());
        central.extend((e.central_extra.len() as u16).to_le_bytes());
        central.extend(0u16.to_le_bytes());
        central.extend(0u16.to_le_bytes());
        central.extend(0u16.to_le_bytes());
        central.extend((e.mode.unwrap_or(0) << 16).to_le_bytes());
        central.extend(offset.to_le_bytes());
        central.extend(e.name.as_bytes());
        central.extend(&e.central_extra);
    }
    let start = out.len() as u32;
    out.extend(&central);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((central.len() as u32).to_le_bytes());
    out.extend(start.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

fn refused(archive: &[u8]) -> ArchiveError {
    unpack(archive, &Limits::default()).expect_err("the archive should be refused")
}

// ---- packing

#[test]
fn test_a_package_survives_a_round_trip_byte_for_byte() {
    let packed = pack("reviewer", &package()).unwrap();
    let got = unpack(&packed, &Limits::default()).unwrap();
    assert_eq!(got.top, "reviewer");
    let mut want = package();
    want.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(got.entries, want);
}

#[test]
fn test_packing_is_deterministic_and_does_not_depend_on_the_order_given() {
    let mut reversed = package();
    reversed.reverse();
    assert_eq!(
        pack("reviewer", &package()).unwrap(),
        pack("reviewer", &reversed).unwrap()
    );
}

#[test]
fn test_the_archive_is_the_skills_api_shape() {
    let packed = pack("reviewer", &package()).unwrap();
    // A zip: local file header first, end of central directory last.
    assert_eq!(&packed[..4], b"PK\x03\x04");
    let text = String::from_utf8_lossy(&packed).into_owned();
    for name in [
        "reviewer/SKILL.md",
        "reviewer/assets/blob.bin",
        "reviewer/scripts/run.sh",
    ] {
        assert!(text.contains(name), "{name}");
    }
    // Unix, modes 0755 and 0644, in the entries' order (assets, SKILL.md?
    // byte order: the capital first).
    let modes: Vec<u32> = (0..packed.len().saturating_sub(46))
        .filter(|i| packed[*i..].starts_with(b"PK\x01\x02"))
        .map(|i| u32::from_le_bytes(packed[i + 38..i + 42].try_into().unwrap()) >> 16)
        .collect();
    assert_eq!(modes, [0o100_644, 0o100_644, 0o100_755]);
}

#[test]
fn test_there_is_nothing_to_pack_in_an_empty_package() {
    assert_eq!(
        pack("reviewer", &[]).unwrap_err(),
        ArchiveError::NothingToPack {
            name: "reviewer".to_string()
        }
    );
}

#[test]
fn test_an_empty_file_and_a_big_one_both_round_trip() {
    let big = vec![b'x'; 200_000];
    let entries = vec![entry("SKILL.md", b"", false), entry("big.txt", &big, false)];
    let got = unpack(&pack("s", &entries).unwrap(), &Limits::default()).unwrap();
    assert_eq!(got.entries, entries);
}

// ---- unpacking

#[test]
fn test_a_stored_entry_is_read_as_well_as_a_deflated_one() {
    let archive = zip(&[
        Raw::file("reviewer/SKILL.md", b"hello"),
        Raw::file("reviewer/dir/", b"").mode(0o040_755),
    ]);
    let got = unpack(&archive, &Limits::default()).unwrap();
    assert_eq!(got.entries, [entry("SKILL.md", b"hello", false)]);
}

// Go: mode normalization keeps the exec bit and nothing else
#[test]
fn test_the_mode_gives_the_exec_bit_and_nothing_else() {
    let archive = zip(&[
        Raw::file("p/a", b"1").mode(0o104_755),
        Raw::file("p/b", b"2").mode(0o100_666),
        Raw::file("p/c", b"3").mode(0o100_001),
        Raw::file("p/d", b"4").mode(0o100_100),
    ]);
    let got = unpack(&archive, &Limits::default()).unwrap();
    let exec: Vec<(&str, bool)> = got
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.executable))
        .collect();
    assert_eq!(exec, [("a", true), ("b", false), ("c", true), ("d", true)]);
}

#[test]
fn test_an_archive_from_a_host_without_unix_modes_has_no_exec_bit() {
    let mut raw = Raw::file("p/a", b"1");
    raw.mode = None;
    let got = unpack(&zip(&[raw]), &Limits::default()).unwrap();
    assert_eq!(got.entries, [entry("a", b"1", false)]);
}

#[test]
fn test_what_is_no_zip_is_refused() {
    assert_eq!(
        refused(b"this is not an archive"),
        ArchiveError::Unrecognized
    );
    assert_eq!(refused(b""), ArchiveError::Unrecognized);
    assert_eq!(
        refused(b"\x1f\x8b\x08\x00 gzip"),
        ArchiveError::Unrecognized
    );
    assert_eq!(refused(b"PK"), ArchiveError::Unrecognized);
}

#[test]
fn test_a_truncated_or_corrupted_zip_is_refused() {
    let packed = pack("reviewer", &package()).unwrap();
    for cut in [packed.len() - 1, packed.len() / 2, 30] {
        let err = refused(&packed[..cut]);
        assert!(
            matches!(
                err,
                ArchiveError::Invalid { .. } | ArchiveError::Unrecognized
            ),
            "{cut}: {err:?}"
        );
    }
    // A flipped byte in the data is the checksum's catch (stored: no decoder
    // to complain first).
    let mut archive = zip(&[Raw::file("p/SKILL.md", b"hello")]);
    let at = archive.windows(5).position(|w| w == b"hello").unwrap();
    archive[at] = b'j';
    assert_eq!(
        refused(&archive),
        ArchiveError::Checksum {
            name: "p/SKILL.md".to_string()
        }
    );
}

#[test]
fn test_an_empty_archive_and_a_marker_alone_hold_no_package() {
    assert_eq!(refused(&zip(&[])), ArchiveError::Empty);
    assert_eq!(
        refused(&zip(&[Raw::file("reviewer/", b"").mode(0o040_755)])),
        ArchiveError::NoFiles {
            top: "reviewer".to_string()
        }
    );
}

// Go: zip-slip, absolute paths, the second top-level name
#[test]
fn test_a_path_that_would_leave_the_package_is_refused() {
    for (name, want) in [
        (
            "/etc/passwd",
            ArchiveError::Absolute {
                name: "/etc/passwd".to_string(),
            },
        ),
        (
            "C:/x/y",
            ArchiveError::Absolute {
                name: "C:/x/y".to_string(),
            },
        ),
        (
            "p/../../etc/x",
            ArchiveError::Traversal {
                name: "p/../../etc/x".to_string(),
            },
        ),
        (
            "p/..",
            ArchiveError::Traversal {
                name: "p/..".to_string(),
            },
        ),
        (
            "p\\x",
            ArchiveError::BadName {
                name: "p\\x".to_string(),
            },
        ),
        (
            "p//x",
            ArchiveError::BadName {
                name: "p//x".to_string(),
            },
        ),
        (
            "p/a\0b",
            ArchiveError::BadName {
                name: "p/a\0b".to_string(),
            },
        ),
    ] {
        let archive = zip(&[Raw::file("p/SKILL.md", b"ok"), Raw::file(name, b"evil")]);
        assert_eq!(refused(&archive), want, "{name:?}");
    }
}

#[test]
fn test_an_entry_outside_the_one_top_level_directory_is_refused() {
    let archive = zip(&[Raw::file("p/SKILL.md", b"a"), Raw::file("q/x", b"b")]);
    assert_eq!(
        refused(&archive),
        ArchiveError::ForeignTop {
            name: "q/x".to_string(),
            top: "p".to_string()
        }
    );
    // A file at the top level is outside any package directory.
    let archive = zip(&[Raw::file("p/SKILL.md", b"a"), Raw::file("loose", b"b")]);
    assert!(matches!(refused(&archive), ArchiveError::ForeignTop { .. }));
}

#[test]
fn test_a_link_or_a_special_file_is_refused() {
    for (mode, link) in [
        (0o120_777, true),
        (0o020_666, false),
        (0o060_666, false),
        (0o010_666, false),
        (0o140_666, false),
    ] {
        let archive = zip(&[
            Raw::file("p/SKILL.md", b"a"),
            Raw::file("p/x", b"/etc/passwd").mode(mode),
        ]);
        let want = if link {
            ArchiveError::Link {
                name: "p/x".to_string(),
            }
        } else {
            ArchiveError::Special {
                name: "p/x".to_string(),
            }
        };
        assert_eq!(refused(&archive), want, "{mode:o}");
    }
}

#[test]
fn test_the_same_path_twice_is_refused() {
    let archive = zip(&[Raw::file("p/a", b"1"), Raw::file("p/a", b"2")]);
    assert_eq!(refused(&archive), ArchiveError::Repeated);
    // The same path by two names: a file and a directory of that name.
    let archive = zip(&[
        Raw::file("p/SKILL.md", b"a"),
        Raw::file("p/a", b"1"),
        Raw::file("p/a/b", b"2"),
    ]);
    assert_eq!(
        refused(&archive),
        ArchiveError::Duplicate {
            name: "p/a".to_string()
        }
    );
}

#[test]
fn test_more_entries_than_allowed_are_refused() {
    let archive = zip(&[
        Raw::file("p/a", b"1"),
        Raw::file("p/b", b"2"),
        Raw::file("p/c", b"3"),
    ]);
    let limits = Limits {
        max_entries: 2,
        ..Limits::default()
    };
    assert_eq!(
        unpack(&archive, &limits).unwrap_err(),
        ArchiveError::TooManyEntries { max: 2 }
    );
    let limits = Limits {
        max_entries: 3,
        ..Limits::default()
    };
    assert!(unpack(&archive, &limits).is_ok());
}

// Go: the archive's own size fields are never trusted
#[test]
fn test_more_bytes_than_allowed_are_refused_by_what_is_read_not_what_is_claimed() {
    let big = vec![0u8; 5_000];
    let packed = pack("p", &[entry("SKILL.md", &big, false)]).unwrap();
    assert!(packed.len() < 1_000, "zeros deflate: {}", packed.len());
    let limits = Limits {
        max_total_bytes: 1_000,
        ..Limits::default()
    };
    assert_eq!(
        unpack(&packed, &limits).unwrap_err(),
        ArchiveError::TooBig { max: 1_000 }
    );
    // A header that claims little and a stream that holds a lot.
    let mut liar = Raw::file("p/SKILL.md", &big);
    liar.claimed_size = 10;
    liar.method = 8;
    liar.stored = deflate(&big);
    liar.crc = crc32fast::hash(&big);
    assert!(matches!(
        unpack(&zip(&[liar]), &limits).unwrap_err(),
        ArchiveError::Invalid { .. }
    ));
    // Within the limit the same package comes out whole.
    let ok = Limits {
        max_total_bytes: 5_000,
        ..Limits::default()
    };
    assert_eq!(unpack(&packed, &ok).unwrap().entries[0].bytes, big);
}

fn deflate(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn test_an_encrypted_entry_or_an_odd_method_is_refused() {
    let mut encrypted = Raw::file("p/SKILL.md", b"x");
    encrypted.flags |= 1;
    assert_eq!(
        refused(&zip(&[encrypted])),
        ArchiveError::Encrypted {
            name: "p/SKILL.md".to_string()
        }
    );
    let mut odd = Raw::file("p/SKILL.md", b"x");
    odd.method = 12;
    assert_eq!(
        refused(&zip(&[odd])),
        ArchiveError::Method {
            name: "p/SKILL.md".to_string(),
            method: 12
        }
    );
}

#[test]
fn test_every_error_is_localized() {
    for sample in diagnostic_samples() {
        assert!(
            sample.code().starts_with("library-archive-"),
            "{}",
            sample.code()
        );
    }
}

// ---- the zip crate's slice: what Go writes, zip64, and the traps a library
// reader brings with it

/// An archive written by Go's `archive/zip` with the headers `ExportSkillZip`
/// builds (deflate, modified 1970-01-01, `SetMode`); the generator is
/// `tests/fixtures/go-reviewer.gen.go.txt`. Go sets the data-descriptor flag
/// on every entry and dates them before the DOS epoch.
const GO_ARCHIVE: &[u8] = include_bytes!("../../tests/fixtures/go-reviewer.zip");

fn go_package() -> Vec<Entry> {
    vec![
        entry(
            "SKILL.md",
            b"---\nname: reviewer\ndescription: SKILL-MARKER-reviewer-9f3c21\n---\n\nReview carefully.\n",
            false,
        ),
        entry("empty.txt", b"", false),
        entry("references/notes.txt", &b"note \xc3\xa9\n".repeat(300), false),
        entry(
            "scripts/run.sh",
            b"#!/bin/sh\n# SCRIPT-MARKER-reviewer-2b7e40\necho ok\n",
            true,
        ),
    ]
}

#[test]
fn test_an_archive_written_by_gos_exporter_imports_byte_for_byte() {
    let got = unpack(GO_ARCHIVE, &Limits::default()).unwrap();
    assert_eq!(got.top, "reviewer");
    assert_eq!(got.entries, go_package());
}

#[test]
fn test_what_rust_packs_holds_what_gos_exporter_packs() {
    // Same package, names and modes: the two archives hold the same entries
    // (the compressed bytes may differ by encoder).
    let packed = pack("reviewer", &go_package()).unwrap();
    assert_eq!(
        unpack(&packed, &Limits::default()).unwrap(),
        unpack(GO_ARCHIVE, &Limits::default()).unwrap()
    );
}

/// A zip64 archive: every size and the offset in the central directory are
/// 0xFFFFFFFF with the real values in the zip64 extra field, and the end of
/// central directory is the zip64 record and its locator. Entries are stored;
/// `claimed` overrides the uncompressed size the headers state.
fn zip64(entries: &[(&str, &[u8], Option<u64>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data, claimed) in entries {
        let offset = out.len() as u64;
        let size = claimed.unwrap_or(data.len() as u64);
        let crc = crc32fast::hash(data);
        out.extend(0x0403_4b50u32.to_le_bytes());
        out.extend(45u16.to_le_bytes());
        out.extend(0x0800u16.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(0x21u16.to_le_bytes());
        out.extend(crc.to_le_bytes());
        out.extend(u32::MAX.to_le_bytes());
        out.extend(u32::MAX.to_le_bytes());
        out.extend((name.len() as u16).to_le_bytes());
        out.extend(20u16.to_le_bytes());
        out.extend(name.as_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(16u16.to_le_bytes());
        out.extend(size.to_le_bytes());
        out.extend((data.len() as u64).to_le_bytes());
        out.extend(data.iter());
        central.extend(0x0201_4b50u32.to_le_bytes());
        central.extend(((3u16 << 8) | 45).to_le_bytes());
        central.extend(45u16.to_le_bytes());
        central.extend(0x0800u16.to_le_bytes());
        central.extend(0u16.to_le_bytes());
        central.extend(0u16.to_le_bytes());
        central.extend(0x21u16.to_le_bytes());
        central.extend(crc.to_le_bytes());
        central.extend(u32::MAX.to_le_bytes());
        central.extend(u32::MAX.to_le_bytes());
        central.extend((name.len() as u16).to_le_bytes());
        central.extend(28u16.to_le_bytes());
        central.extend([0u8; 6]);
        central.extend((0o100_644u32 << 16).to_le_bytes());
        central.extend(u32::MAX.to_le_bytes());
        central.extend(name.as_bytes());
        central.extend(1u16.to_le_bytes());
        central.extend(24u16.to_le_bytes());
        central.extend(size.to_le_bytes());
        central.extend((data.len() as u64).to_le_bytes());
        central.extend(offset.to_le_bytes());
    }
    let start = out.len() as u64;
    out.extend(&central);
    let end64 = out.len() as u64;
    out.extend(0x0606_4b50u32.to_le_bytes());
    out.extend(44u64.to_le_bytes());
    out.extend(45u16.to_le_bytes());
    out.extend(45u16.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend((entries.len() as u64).to_le_bytes());
    out.extend((entries.len() as u64).to_le_bytes());
    out.extend((central.len() as u64).to_le_bytes());
    out.extend(start.to_le_bytes());
    out.extend(0x0706_4b50u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(end64.to_le_bytes());
    out.extend(1u32.to_le_bytes());
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend(u16::MAX.to_le_bytes());
    out.extend(u16::MAX.to_le_bytes());
    out.extend(u32::MAX.to_le_bytes());
    out.extend(u32::MAX.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

// A zip64 archive inside the limits is an archive like any other: the limits,
// not the container's width, are the defence.
#[test]
fn test_a_zip64_archive_within_the_limits_is_taken() {
    let archive = zip64(&[("p/SKILL.md", b"hello", None), ("p/a/b", b"world", None)]);
    let got = unpack(&archive, &Limits::default()).unwrap();
    assert_eq!(got.top, "p");
    assert_eq!(
        got.entries,
        [
            entry("SKILL.md", b"hello", false),
            entry("a/b", b"world", false)
        ]
    );
}

#[test]
fn test_a_zip64_archive_is_held_to_the_same_limits() {
    // It claims eight GiB: refused on the claim, nothing allocated for it.
    let eight_gib = 8u64 << 30;
    let archive = zip64(&[("p/SKILL.md", b"hello", Some(eight_gib))]);
    assert_eq!(
        unpack(&archive, &Limits::default()).unwrap_err(),
        ArchiveError::TooBig {
            max: Limits::default().max_total_bytes
        }
    );
    // And by what it holds: five bytes over a four byte cap.
    let archive = zip64(&[("p/SKILL.md", b"hello", None)]);
    let limits = Limits {
        max_total_bytes: 4,
        ..Limits::default()
    };
    assert_eq!(
        unpack(&archive, &limits).unwrap_err(),
        ArchiveError::TooBig { max: 4 }
    );
}

#[test]
fn test_a_zip64_archive_holds_to_the_path_rules() {
    let archive = zip64(&[("p/SKILL.md", b"a", None), ("p/../../x", b"b", None)]);
    assert!(matches!(
        unpack(&archive, &Limits::default()).unwrap_err(),
        ArchiveError::Traversal { .. }
    ));
}

// The classic bomb: central entries naming one local entry under other names,
// so a few kilobytes expand to far more than they hold. The local header's
// name is not theirs: refused before any byte is inflated.
#[test]
fn test_entries_that_share_one_stream_are_refused() {
    let big = vec![0u8; 4_000];
    let mut raw = Raw::file("p/a", &big);
    raw.method = 8;
    raw.stored = deflate(&big);
    let mut archive = zip(&[raw]);
    let at = archive
        .windows(4)
        .position(|w| w == 0x0201_4b50u32.to_le_bytes())
        .unwrap();
    let end = archive.len() - 22;
    let record = archive[at..end].to_vec();
    let mut tail = archive.split_off(end);
    for name in ["p/b", "p/c"] {
        let mut copy = record.clone();
        copy[46..46 + 3].copy_from_slice(name.as_bytes());
        archive.extend(copy);
    }
    tail[8..10].copy_from_slice(&3u16.to_le_bytes());
    tail[10..12].copy_from_slice(&3u16.to_le_bytes());
    tail[12..16].copy_from_slice(&((archive.len() - at) as u32).to_le_bytes());
    archive.extend(tail);
    assert!(matches!(refused(&archive), ArchiveError::Invalid { .. }));
}

// Entries whose data overlap, with headers that agree: a stream's bytes would
// count under two names. Refused for the overlap.
#[test]
fn test_entries_whose_data_overlap_are_refused() {
    let mut archive = zip(&[Raw::file("p/a", b"1234"), Raw::file("p/b", b"5678")]);
    // Stretch the first entry (both records) over the second's header and data.
    let stretched = 4u32 + 30 + 3 + 4;
    let patch = |bytes: &mut Vec<u8>, at: usize| {
        bytes[at..at + 4].copy_from_slice(&stretched.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&stretched.to_le_bytes());
    };
    patch(&mut archive, 18);
    let central = archive
        .windows(4)
        .position(|w| w == 0x0201_4b50u32.to_le_bytes())
        .unwrap();
    patch(&mut archive, central + 20);
    assert_eq!(refused(&archive), ArchiveError::Overlap);
}

// An AES or password-protected entry is refused whole (no password is ever
// asked for), however its method field reads.
#[test]
fn test_an_aes_entry_is_refused_as_encrypted() {
    let mut aes = Raw::file("p/SKILL.md", b"x");
    aes.flags |= 1;
    aes.method = 99;
    assert_eq!(
        refused(&zip(&[aes])),
        ArchiveError::Encrypted {
            name: "p/SKILL.md".to_string()
        }
    );
}

// Other methods a general zip reader knows (bzip2, lzma, zstd, ppmd) are not
// read.
#[test]
fn test_bzip2_lzma_zstd_and_ppmd_entries_are_refused_by_method() {
    for method in [12, 14, 93, 98] {
        let mut odd = Raw::file("p/SKILL.md", b"x");
        odd.method = method;
        assert_eq!(
            refused(&zip(&[odd])),
            ArchiveError::Method {
                name: "p/SKILL.md".to_string(),
                method
            },
            "{method}"
        );
    }
}

// Without the UTF-8 flag a library reader takes names as CP437; here a name is
// UTF-8 bytes or the entry is refused, never re-read as another alphabet.
#[test]
fn test_a_name_that_is_not_utf8_is_refused_with_or_without_the_flag() {
    for flags in [0x0800, 0] {
        let mut raw = Raw::file("p/cafX", b"b");
        raw.flags = flags;
        let mut archive = zip(&[Raw::file("p/SKILL.md", b"a"), raw]);
        for at in 0..archive.len() - 6 {
            if &archive[at..at + 6] == b"p/cafX" {
                archive[at + 5] = 0xff;
            }
        }
        assert!(
            matches!(refused(&archive), ArchiveError::BadName { .. }),
            "{flags:x}"
        );
    }
}

// An "Info-ZIP Unicode Path" extra field names the entry again, and the
// library reports that name in place of the header's. Whichever name is used
// is the one every path rule is applied to: an alias that climbs out is
// refused as any name that does, and one record carrying an alias the other
// lacks is refused as inconsistent.
fn unicode_path(header: &str, alias: &str) -> Vec<u8> {
    let mut extra = Vec::new();
    extra.extend(0x7075u16.to_le_bytes());
    extra.extend(((5 + alias.len()) as u16).to_le_bytes());
    extra.push(1);
    extra.extend(crc32fast::hash(header.as_bytes()).to_le_bytes());
    extra.extend(alias.as_bytes());
    extra
}

#[test]
fn test_a_unicode_path_extra_field_is_held_to_the_path_rules() {
    let extra = unicode_path("p/a", "../../etc/x");
    let mut raw = Raw::file("p/a", b"1");
    raw.local_extra = extra.clone();
    raw.central_extra = extra.clone();
    assert_eq!(
        refused(&zip(&[Raw::file("p/SKILL.md", b"s"), raw])),
        ArchiveError::Traversal {
            name: "../../etc/x".to_string()
        }
    );
    // An alias that stays in the package is the entry's name.
    let extra = unicode_path("p/a", "p/b");
    let mut raw = Raw::file("p/a", b"1");
    raw.local_extra = extra.clone();
    raw.central_extra = extra.clone();
    let got = unpack(
        &zip(&[Raw::file("p/SKILL.md", b"s"), raw]),
        &Limits::default(),
    )
    .unwrap();
    let paths: Vec<&str> = got.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["SKILL.md", "b"]);
    // On the central record alone: the two records disagree.
    let mut raw = Raw::file("p/a", b"1");
    raw.central_extra = extra;
    let archive = zip(&[Raw::file("p/SKILL.md", b"s"), raw]);
    assert!(matches!(refused(&archive), ArchiveError::Invalid { .. }));
}

// A zip64 end record that claims a trillion entries must be refused, not
// allocated for.
#[test]
fn test_a_zip64_count_far_beyond_the_archive_is_refused_without_allocating() {
    let mut archive = zip64(&[("p/SKILL.md", b"hello", None)]);
    let at = archive
        .windows(4)
        .rposition(|w| w == 0x0606_4b50u32.to_le_bytes())
        .unwrap();
    let trillion = (1u64 << 40).to_le_bytes();
    archive[at + 24..at + 32].copy_from_slice(&trillion);
    archive[at + 32..at + 40].copy_from_slice(&trillion);
    let err = refused(&archive);
    assert!(
        matches!(
            err,
            ArchiveError::Invalid { .. } | ArchiveError::TooManyEntries { .. }
        ),
        "{err:?}"
    );
}
