//! The client tag: what a supervisor writes into each host it starts, so that
//! a later supervisor can tell what the host is for.
//!
//! A host returns its tag verbatim in `Info` and attaches no meaning to it
//! (`crates/terminal-host/SPEC.md`, "Terms"). Everything Farhelm needs to
//! adopt a host after a restart is therefore here: which session it belongs to, and
//! whether it runs that session's agent (and for which launch generation) or
//! one of its terminal tabs (which one, whether it runs in a scope of its
//! own, and when it was created, for ordering tabs).
//!
//! The encoding is as permanent as a wire format. A host keeps its tag for as
//! long as it runs, across every update of the supervisor that reads it, so a
//! supervisor of any later release must read every version ever written.
//! Rules:
//!
//! - Every tag starts with the same six bytes, in every version: the magic
//!   `FHTG` and a big-endian u16 version. A later version changes only what
//!   follows them.
//! - A version, once shipped, is never changed. New fields mean a new
//!   version, written only once every supported reader understands it; the
//!   golden fixtures in `tests/fixtures/host-tag/v<N>/` pin each version's
//!   exact bytes, and stay for good.
//! - Decoding is strict, and failure is not an error that drops the host: a
//!   tag that is not Farhelm's, of a version this build does not know, or
//!   malformed decodes to [`UnreadableTag`]. The caller keeps such a host and
//!   shows it as unavailable; it is never an orphan to reclaim, because a
//!   newer supervisor may own it and come back.
//!
//! Version 1's layout, all integers big-endian:
//!
//! | Bytes | Field        | Meaning                                                 |
//! | ----- | ------------ | ------------------------------------------------------- |
//! | 4     | magic        | `FHTG`                                                  |
//! | 2     | version      | 1                                                       |
//! | 1     | role         | 1 agent, 2 tab                                          |
//! | 8     | `created_ns` | spawn time, nanoseconds since the Unix epoch            |
//! | 2 + n | session id   | u16 length, then 1 to [`MAX_ID_LEN`] bytes of UTF-8     |
//! |       | agent:       | `generation`, i64: the launch generation                |
//! |       | tab:         | tab id (u16 length + 1 to [`MAX_ID_LEN`] bytes of UTF-8), |
//! |       |              | then `scoped`, u8 0 or 1: the tab runs in its own scope |
//!
//! Nothing follows the last field.

use std::fmt;

/// The first four bytes of every Farhelm tag, in every version.
pub const TAG_MAGIC: [u8; 4] = *b"FHTG";

/// The tag version this build writes.
pub const TAG_VERSION: u16 = 1;

/// Longest session or tab id a tag holds, in bytes. Farhelm's ids are UUIDs
/// (36 bytes); the bound leaves room for any id a store could hold while
/// keeping the largest tag far inside the host's `MAX_TAG`.
pub const MAX_ID_LEN: usize = 256;

const ROLE_AGENT: u8 = 1;
const ROLE_TAB: u8 = 2;

/// What one host is for, as its tag records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTag {
    /// The session the host belongs to.
    pub session_id: String,
    /// When the host was spawned, in nanoseconds since the Unix epoch
    /// ([`creation_stamp`]). It orders a session's tabs, which have no stored
    /// record of their own; a wall clock rather than a counter, so that
    /// ordering needs nothing persisted. A clock stepped backwards can misorder
    /// tabs opened across the step, which costs only their order on screen.
    pub created_ns: u64,
    /// The agent's terminal, or one of the session's tabs.
    pub role: HostRole,
}

/// Which of a session's terminals a host runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRole {
    /// The session's agent, for one launch generation: binding the host to
    /// the generation is what tells a restart's new host from the one it
    /// replaced.
    Agent { generation: i64 },
    /// A terminal tab. `scoped` says whether the tab's program runs in the
    /// tab's own systemd scope, which closing the tab must then stop; a tab
    /// has no stored row to record that anywhere else.
    Tab { tab_id: String, scoped: bool },
}

/// A tag this build will not write: an id is empty or longer than
/// [`MAX_ID_LEN`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot tag a terminal host: the {field} is {len} bytes, outside 1 to {MAX_ID_LEN}")]
pub struct TagError {
    pub field: &'static str,
    pub len: usize,
}

/// A tag that this build cannot read. The host is kept and shown as
/// unavailable, never treated as an orphan: see the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnreadableTag {
    /// It does not start with [`TAG_MAGIC`]: not a tag Farhelm wrote.
    NotFarhelm,
    /// A version this build does not know, most likely written by a newer
    /// release.
    UnknownVersion(u16),
    /// A known version whose bytes break its layout.
    Malformed(&'static str),
}

impl fmt::Display for UnreadableTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFarhelm => f.write_str("the host's tag is not a Farhelm tag"),
            Self::UnknownVersion(version) => {
                write!(
                    f,
                    "the host's tag has version {version}, unknown to this build"
                )
            }
            Self::Malformed(what) => write!(f, "the host's tag is malformed: {what}"),
        }
    }
}

impl std::error::Error for UnreadableTag {}

/// Nanoseconds since the Unix epoch now, for [`HostTag::created_ns`]. A clock
/// before the epoch gives 0, and one past what a u64 holds (the year 2554)
/// gives `u64::MAX`.
pub fn creation_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_nanos()).unwrap_or(u64::MAX)
        })
}

impl HostTag {
    /// The tag's bytes, in the current version ([`TAG_VERSION`]).
    pub fn encode(&self) -> Result<Vec<u8>, TagError> {
        let mut out = Vec::with_capacity(64);
        out.extend_from_slice(&TAG_MAGIC);
        out.extend_from_slice(&TAG_VERSION.to_be_bytes());
        out.push(match self.role {
            HostRole::Agent { .. } => ROLE_AGENT,
            HostRole::Tab { .. } => ROLE_TAB,
        });
        out.extend_from_slice(&self.created_ns.to_be_bytes());
        put_id(&mut out, "session id", &self.session_id)?;
        match &self.role {
            HostRole::Agent { generation } => out.extend_from_slice(&generation.to_be_bytes()),
            HostRole::Tab { tab_id, scoped } => {
                put_id(&mut out, "tab id", tab_id)?;
                out.push(u8::from(*scoped));
            }
        }
        Ok(out)
    }

    /// Read a tag of any version this build knows.
    pub fn decode(bytes: &[u8]) -> Result<HostTag, UnreadableTag> {
        let Some(rest) = bytes.strip_prefix(&TAG_MAGIC) else {
            return Err(UnreadableTag::NotFarhelm);
        };
        let mut reader = Reader { rest };
        let version = reader
            .u16()
            .map_err(|_| UnreadableTag::Malformed("no version"))?;
        match version {
            1 => decode_v1(reader).map_err(UnreadableTag::Malformed),
            other => Err(UnreadableTag::UnknownVersion(other)),
        }
    }
}

/// Append a u16-length-prefixed id, refusing one outside 1 to
/// [`MAX_ID_LEN`] bytes.
fn put_id(out: &mut Vec<u8>, field: &'static str, id: &str) -> Result<(), TagError> {
    let len = id.len();
    if !(1..=MAX_ID_LEN).contains(&len) {
        return Err(TagError { field, len });
    }
    out.extend_from_slice(&u16::try_from(len).expect("bounded above").to_be_bytes());
    out.extend_from_slice(id.as_bytes());
    Ok(())
}

/// Version 1's body, after the magic and the version.
fn decode_v1(mut r: Reader<'_>) -> Result<HostTag, &'static str> {
    let role = r.u8()?;
    let created_ns = r.u64()?;
    let session_id = r.id()?;
    let role = match role {
        ROLE_AGENT => HostRole::Agent {
            generation: i64::from_be_bytes(r.take(8)?.try_into().expect("eight bytes")),
        },
        ROLE_TAB => HostRole::Tab {
            tab_id: r.id()?,
            scoped: match r.u8()? {
                0 => false,
                1 => true,
                _ => return Err("scope flag is neither 0 nor 1"),
            },
        },
        _ => return Err("unknown role"),
    };
    if !r.rest.is_empty() {
        return Err("trailing bytes");
    }
    Ok(HostTag {
        session_id,
        created_ns,
        role,
    })
}

/// Reads a tag's fields front to back. Every read fails, rather than guess,
/// on bytes that run out.
struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], &'static str> {
        if self.rest.len() < n {
            return Err("too short");
        }
        let (head, tail) = self.rest.split_at(n);
        self.rest = tail;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().expect("two")))
    }
    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().expect("eight")))
    }
    fn id(&mut self) -> Result<String, &'static str> {
        let len = usize::from(self.u16()?);
        if !(1..=MAX_ID_LEN).contains(&len) {
            return Err("id length out of range");
        }
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| "id is not UTF-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn golden_dir(version: u16) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/host-tag")
            .join(format!("v{version}"))
    }

    /// One tag of each shape version 1 can hold, with values that exercise
    /// every field's encoding (a negative generation, both flag values, a
    /// stamp using all eight bytes).
    fn v1_fixtures() -> Vec<(&'static str, HostTag)> {
        vec![
            (
                "agent",
                HostTag {
                    session_id: "0f8e2b9c-4c1d-4e7a-9a53-7d2f61c0b8e4".into(),
                    created_ns: 0x1867_5309_abcd_ef01,
                    role: HostRole::Agent { generation: 7 },
                },
            ),
            (
                "agent-negative-generation",
                HostTag {
                    session_id: "s".into(),
                    created_ns: 0,
                    role: HostRole::Agent { generation: -2 },
                },
            ),
            (
                "tab-scoped",
                HostTag {
                    session_id: "0f8e2b9c-4c1d-4e7a-9a53-7d2f61c0b8e4".into(),
                    created_ns: 1_793_000_000_123_456_789,
                    role: HostRole::Tab {
                        tab_id: "6b1c0d3e-2f4a-4b5c-8d6e-7f8091a2b3c4".into(),
                        scoped: true,
                    },
                },
            ),
            (
                "tab-unscoped",
                HostTag {
                    session_id: "0f8e2b9c-4c1d-4e7a-9a53-7d2f61c0b8e4".into(),
                    created_ns: u64::MAX,
                    role: HostRole::Tab {
                        tab_id: "t".into(),
                        scoped: false,
                    },
                },
            ),
        ]
    }

    /// Bytes as the fixture files hold them: 16 per line, in hex, the
    /// terminal host's golden fixture format.
    fn hex(bytes: &[u8]) -> String {
        let mut text = String::new();
        for chunk in bytes.chunks(16) {
            let line: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
            text.push_str(&line.join(" "));
            text.push('\n');
        }
        text
    }

    /// Version 1 tags encode to exactly the checked-in bytes, and those bytes
    /// decode back to the same tags; the fixture directory holds nothing
    /// else.
    ///
    /// Why: running hosts keep their tags across every update, so every later
    /// supervisor must read exactly these bytes (module docs). A failure here
    /// means the encoding changed, which is never allowed for a shipped
    /// version: add a version instead. Spec: each fixture encodes to
    /// `tests/fixtures/host-tag/v1/<name>.hex`, the file's bytes decode to
    /// the fixture, and the directory's files are exactly the fixtures'.
    #[test]
    fn version_one_tags_match_the_golden_fixtures() {
        for (name, tag) in v1_fixtures() {
            let path = golden_dir(1).join(format!("{name}.hex"));
            let golden = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
            let bytes = tag.encode().unwrap();
            assert_eq!(hex(&bytes), golden, "{name} changed its encoding");
            let from_file: Vec<u8> = golden
                .split_whitespace()
                .map(|pair| u8::from_str_radix(pair, 16).unwrap())
                .collect();
            assert_eq!(HostTag::decode(&from_file), Ok(tag), "{name}");
        }
        let mut files: Vec<String> = std::fs::read_dir(golden_dir(1))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        let mut names: Vec<String> = v1_fixtures()
            .iter()
            .map(|(name, _)| format!("{name}.hex"))
            .collect();
        names.sort();
        assert_eq!(files, names, "fixture files and fixtures");
    }

    /// Writes the golden fixtures that do not exist yet. Run it by hand when
    /// adding a fixture or a version; never to make a failing comparison
    /// pass.
    ///
    /// It only ever creates files: an existing fixture is the permanent
    /// record of bytes that running hosts hold, so rewriting one from the
    /// current encoder would hide exactly the drift the fixtures exist to
    /// catch.
    #[test]
    #[ignore = "writes tests/fixtures/host-tag; run by hand only when adding fixtures"]
    fn write_golden_fixtures() {
        use std::io::Write as _;
        std::fs::create_dir_all(golden_dir(TAG_VERSION)).unwrap();
        for (name, tag) in v1_fixtures() {
            let path = golden_dir(TAG_VERSION).join(format!("{name}.hex"));
            match std::fs::File::create_new(&path) {
                Ok(mut file) => file
                    .write_all(hex(&tag.encode().unwrap()).as_bytes())
                    .unwrap(),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("writing {}: {error}", path.display()),
            }
        }
    }

    /// A tag of a version this build does not know reads as unreadable, with
    /// that version, whatever follows it.
    ///
    /// Why: a supervisor downgraded below the release that wrote a newer tag
    /// must keep that host (and show it as unavailable) rather than misread
    /// it or reclaim it as an orphan. Spec: the magic, then version 2 or
    /// `u16::MAX`, followed by nothing or by a valid version 1 body, gives
    /// `UnknownVersion` with that number; version 0 does too.
    #[test]
    fn an_unknown_version_is_unreadable() {
        let v1_body = &v1_fixtures()[0].1.encode().unwrap()[6..];
        for version in [0u16, 2, u16::MAX] {
            for body in [&[][..], v1_body] {
                let mut bytes = TAG_MAGIC.to_vec();
                bytes.extend_from_slice(&version.to_be_bytes());
                bytes.extend_from_slice(body);
                assert_eq!(
                    HostTag::decode(&bytes),
                    Err(UnreadableTag::UnknownVersion(version))
                );
            }
        }
    }

    /// Bytes that are not a Farhelm tag, and version 1 tags that break the
    /// layout, read as unreadable rather than as some tag.
    ///
    /// Why: a lenient decoder would hand a supervisor a session id it never
    /// wrote, and an unreadable host is kept and shown, never guessed at.
    /// Spec: an empty tag, a foreign one and one with only part of the magic
    /// are `NotFarhelm`; a version 1 tag cut short at every length, with a
    /// byte appended, with an unknown role, with a scope flag of 2, with a
    /// zero-length or over-long id, or with an id that is not UTF-8 is
    /// `Malformed`.
    #[test]
    fn foreign_and_malformed_tags_are_unreadable() {
        for foreign in [&b""[..], b"test tag", b"FHT", b"fhtg\x00\x01"] {
            assert_eq!(HostTag::decode(foreign), Err(UnreadableTag::NotFarhelm));
        }
        let malformed =
            |bytes: &[u8]| matches!(HostTag::decode(bytes), Err(UnreadableTag::Malformed(_)));
        for (_, tag) in v1_fixtures() {
            let bytes = tag.encode().unwrap();
            for cut in TAG_MAGIC.len()..bytes.len() {
                assert!(malformed(&bytes[..cut]), "{tag:?} cut at {cut}");
            }
            let mut longer = bytes.clone();
            longer.push(0);
            assert!(malformed(&longer), "{tag:?} with a trailing byte");
        }
        let tab = v1_fixtures()[2].1.encode().unwrap();
        let mut role = tab.clone();
        role[6] = 3;
        assert!(malformed(&role), "unknown role");
        let mut flag = tab.clone();
        *flag.last_mut().unwrap() = 2;
        assert!(malformed(&flag), "scope flag 2");
        // The session id's length field sits after magic, version, role and
        // stamp (4 + 2 + 1 + 8 bytes).
        let mut empty_id = tab.clone();
        empty_id[15..17].copy_from_slice(&0u16.to_be_bytes());
        assert!(malformed(&empty_id), "empty session id");
        let mut long_id = tab.clone();
        long_id[15..17].copy_from_slice(&(MAX_ID_LEN as u16 + 1).to_be_bytes());
        assert!(malformed(&long_id), "over-long session id");
        let mut not_utf8 = tab;
        not_utf8[17] = 0xff;
        assert!(malformed(&not_utf8), "session id not UTF-8");
    }

    /// The largest tag this build writes fits the host's `MAX_TAG`, and ids
    /// outside 1 to [`MAX_ID_LEN`] bytes are refused before any host starts.
    ///
    /// Why: a host refuses a longer tag at start, so a tag that could exceed
    /// it would fail some spawns depending on the ids. Spec: a tab tag with
    /// both ids at [`MAX_ID_LEN`] round-trips and is at most `MAX_TAG` bytes;
    /// an empty session id and an over-long tab id each give a [`TagError`]
    /// naming the field.
    #[test]
    fn the_largest_tag_fits_and_bad_ids_are_refused() {
        let largest = HostTag {
            session_id: "s".repeat(MAX_ID_LEN),
            created_ns: u64::MAX,
            role: HostRole::Tab {
                tab_id: "t".repeat(MAX_ID_LEN),
                scoped: true,
            },
        };
        let bytes = largest.encode().unwrap();
        assert!(
            bytes.len() <= terminal_host::protocol::MAX_TAG,
            "{}",
            bytes.len()
        );
        assert_eq!(HostTag::decode(&bytes), Ok(largest));

        let empty = HostTag {
            session_id: String::new(),
            created_ns: 0,
            role: HostRole::Agent { generation: 0 },
        };
        assert_eq!(
            empty.encode(),
            Err(TagError {
                field: "session id",
                len: 0
            })
        );
        let long_tab = HostTag {
            session_id: "s".into(),
            created_ns: 0,
            role: HostRole::Tab {
                tab_id: "t".repeat(MAX_ID_LEN + 1),
                scoped: false,
            },
        };
        assert_eq!(
            long_tab.encode(),
            Err(TagError {
                field: "tab id",
                len: MAX_ID_LEN + 1
            })
        );
    }

    /// The creation stamp is the wall clock in nanoseconds.
    ///
    /// Why: tabs are ordered by it, so it must move with time and be in the
    /// unit the tag documents. Spec: a stamp lies between the clock read just
    /// before and just after it, in nanoseconds since the epoch.
    #[test]
    fn the_creation_stamp_is_nanoseconds_since_the_epoch() {
        let nanos = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        };
        let before = nanos();
        let stamp = creation_stamp();
        let after = nanos();
        assert!(
            (before..=after).contains(&stamp),
            "{before} {stamp} {after}"
        );
    }
}
