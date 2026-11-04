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
    /// An extra field in the local header, and in the central one.
    local_extra: Vec<u8>,
    central_extra: Vec<u8>,
    /// A file comment in the central directory.
    comment: Vec<u8>,
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
            comment: Vec::new(),
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
        central.extend((e.comment.len() as u16).to_le_bytes());
        central.extend(0u16.to_le_bytes());
        central.extend(0u16.to_le_bytes());
        central.extend((e.mode.unwrap_or(0) << 16).to_le_bytes());
        central.extend(offset.to_le_bytes());
        central.extend(e.name.as_bytes());
        central.extend(&e.central_extra);
        central.extend(&e.comment);
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
    assert_eq!(
        unpack(&zip(&[liar]), &limits).unwrap_err(),
        ArchiveError::TooBig { max: 1_000 }
    );
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
fn test_an_encrypted_entry_or_an_odd_method_or_zip64_is_refused() {
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
    let mut wide = Raw::file("p/SKILL.md", b"x");
    wide.claimed_size = 0xFFFF_FFFF;
    assert_eq!(refused(&zip(&[wide])), ArchiveError::Zip64);
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

// A header's extra fields and a comment are skipped by their lengths: the
// next entry and the entry's data are found after them.
#[test]
fn test_extra_fields_and_comments_are_stepped_over() {
    let mut first = Raw::file("p/a", b"one");
    first.local_extra = vec![9; 5];
    first.central_extra = vec![7; 2];
    first.comment = vec![8; 3];
    let mut second = Raw::file("p/b", b"two");
    second.local_extra = vec![1; 4];
    second.central_extra = vec![2; 3];
    second.comment = vec![3; 2];
    let got = unpack(&zip(&[first, second]), &Limits::default()).unwrap();
    assert_eq!(
        got.entries,
        [entry("a", b"one", false), entry("b", b"two", false)]
    );
}

/// The archive with the u32 at `field` of its first central entry replaced.
fn with_central_u32(mut archive: Vec<u8>, field: usize, value: u32) -> Vec<u8> {
    let end = archive.len() - 22;
    let start = u32::from_le_bytes(archive[end + 16..end + 20].try_into().unwrap()) as usize;
    archive[start + field..start + field + 4].copy_from_slice(&value.to_le_bytes());
    archive
}

#[test]
fn test_zip64_is_refused_by_any_one_of_the_three_sizes() {
    let archive = zip(&[Raw::file("p/a", b"one")]);
    for (field, what) in [(20, "compressed"), (24, "uncompressed"), (42, "offset")] {
        let wide = with_central_u32(archive.clone(), field, u32::MAX);
        assert_eq!(refused(&wide), ArchiveError::Zip64, "{what}");
    }
}

// A stored entry is held to the budget by its length: the budget itself is
// allowed, a byte past it is not, and nor is a long way past it.
#[test]
fn test_a_stored_entry_may_fill_the_budget_exactly() {
    let limits = |max_total_bytes| Limits {
        max_total_bytes,
        ..Limits::default()
    };
    let archive = zip(&[Raw::file("p/a", b"12345")]);
    assert!(unpack(&archive, &limits(5)).is_ok());
    for max in [4, 3] {
        assert_eq!(
            unpack(&archive, &limits(max)).unwrap_err(),
            ArchiveError::TooBig { max }
        );
    }
}
