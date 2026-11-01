//! The wire format's tests: golden fixtures, round trips and malformed input
//! (SPEC.md, "Protocol").
//!
//! The golden fixtures are the permanent record of version 1's bytes. A host
//! started by one release is talked to by every later release's client, so
//! these files must never change: a test that fails here means the encoding
//! changed, which the change policy forbids. New versions add new fixture
//! directories beside `v1`, never edit it.

use proptest::prelude::*;
use std::path::PathBuf;
use terminal_host::protocol::{
    ChildExit, ClientMessage, ErrorCode, HostMessage, Info, MAX_FRAME, MAX_INPUT, Malformed,
    Position, Size, SnapshotRejection, Spawn, decode_client, decode_host, decode_spawn,
    encode_client, encode_host, encode_spawn, split_frame,
};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/v1")
}

/// One example of every version-1 message and of the child spec, each with
/// fields chosen to exercise every encoding (options both ways, signed
/// values, empty and nonempty bytes).
fn fixtures() -> Vec<(&'static str, Vec<u8>)> {
    let size = Size {
        cols: 120,
        rows: 40,
    };
    let position = Position {
        offset: 0x0102_0304_0506_0708,
        resizes: 3,
    };
    vec![
        (
            "spawn",
            encode_spawn(&Spawn {
                argv: vec![b"/bin/sh".to_vec(), b"-l".to_vec()],
                cwd: b"/home/user".to_vec(),
                env: vec![b"TERM=xterm-256color".to_vec(), b"EMPTY=".to_vec()],
                size,
                journal_cap: 16 * 1024 * 1024,
                tag: b"opaque tag".to_vec(),
                initial_snapshot: None,
            }),
        ),
        (
            "spawn-with-snapshot",
            encode_spawn(&Spawn {
                argv: vec![b"sh".to_vec()],
                cwd: b"/".to_vec(),
                env: vec![],
                size: Size { cols: 1, rows: 1 },
                journal_cap: 64 * 1024,
                tag: vec![],
                initial_snapshot: Some(b"history".to_vec()),
            }),
        ),
        (
            "hello",
            encode_client(&ClientMessage::Hello {
                version: 1,
                take_over: true,
            }),
        ),
        ("info", encode_client(&ClientMessage::Info)),
        ("read", encode_client(&ClientMessage::Read(position))),
        (
            "snapshot",
            encode_client(&ClientMessage::Snapshot {
                position,
                blob: b"blob".to_vec(),
            }),
        ),
        (
            "input",
            encode_client(&ClientMessage::Input(b"\x1b\r".to_vec())),
        ),
        ("resize", encode_client(&ClientMessage::Resize(size))),
        ("shutdown", encode_client(&ClientMessage::Shutdown)),
        ("welcome", encode_host(&HostMessage::Welcome { version: 1 })),
        ("busy", encode_host(&HostMessage::Busy)),
        (
            "info-reply-running",
            encode_host(&HostMessage::InfoReply(Info {
                host_pid: 100,
                child_pid: 101,
                child_exit: None,
                size,
                initial_size: size,
                snapshot: Position::default(),
                tail_start: 0,
                end: 42,
                resizes: 0,
                gap: false,
                delivered_end: 40,
                tag: b"tag".to_vec(),
            })),
        ),
        (
            "info-reply-exited",
            encode_host(&HostMessage::InfoReply(Info {
                host_pid: 100,
                child_pid: 101,
                child_exit: Some(ChildExit::Signal(9)),
                size,
                initial_size: Size { cols: 80, rows: 24 },
                snapshot: position,
                tail_start: 0x0102_0304_0506_0708,
                end: 0x0102_0304_0506_0800,
                resizes: 5,
                gap: true,
                delivered_end: 0x0102_0304_0506_0800,
                tag: vec![],
            })),
        ),
        (
            "snapshot-record",
            encode_host(&HostMessage::SnapshotRecord {
                position,
                blob: b"blob".to_vec(),
            }),
        ),
        (
            "output",
            encode_host(&HostMessage::Output {
                offset: 7,
                data: b"hello\n".to_vec(),
            }),
        ),
        (
            "resized",
            encode_host(&HostMessage::Resized {
                offset: 7,
                index: 2,
                size,
            }),
        ),
        (
            "gap",
            encode_host(&HostMessage::Gap {
                from: 1,
                to: 1000,
                resizes: 4,
                size,
            }),
        ),
        (
            "exited-code",
            encode_host(&HostMessage::Exited {
                offset: 9,
                exit: ChildExit::Code(0),
            }),
        ),
        (
            "exited-signal",
            encode_host(&HostMessage::Exited {
                offset: 9,
                exit: ChildExit::Signal(9),
            }),
        ),
        (
            "snapshot-accepted",
            encode_host(&HostMessage::SnapshotAccepted(position)),
        ),
        (
            "snapshot-rejected",
            encode_host(&HostMessage::SnapshotRejected {
                position,
                reason: SnapshotRejection::InconsistentResizes,
            }),
        ),
        (
            "input-ack",
            encode_host(&HostMessage::InputAck {
                written: 2,
                discarded: 0x0102_0304_0506_0708,
            }),
        ),
        (
            "error",
            encode_host(&HostMessage::Error {
                code: ErrorCode::NotAllowed,
                message: "Hello after Welcome".into(),
            }),
        ),
        ("goodbye", encode_host(&HostMessage::Goodbye)),
    ]
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::new();
    for chunk in bytes.chunks(16) {
        let line: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        text.push_str(&line.join(" "));
        text.push('\n');
    }
    text
}

/// Version 1's bytes are exactly the checked-in fixtures, and every fixture
/// decodes back to the message it came from.
///
/// Why: hosts outlive the clients that started them, so a client of any
/// later release must speak exactly these bytes (SPEC.md, "Change
/// policy"). Spec: each message and the child spec encode to the bytes in
/// `tests/golden/v1/<name>.hex`, and those bytes decode to an equal value.
#[test]
fn version_one_bytes_match_the_golden_fixtures() {
    for (name, bytes) in fixtures() {
        let path = golden_dir().join(format!("{name}.hex"));
        let golden = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
        assert_eq!(hex(&bytes), golden, "{name} changed its encoding");
        let frame = split_frame(&bytes, usize::MAX)
            .unwrap()
            .expect("a whole frame");
        assert_eq!(frame.consumed, bytes.len(), "{name}");
        let reencoded = match frame.kind {
            0x01 => encode_spawn(&decode_spawn(&bytes).unwrap()),
            0x10..=0x7f => encode_client(&decode_client(frame, 1).unwrap()),
            _ => encode_host(&decode_host(frame, 1).unwrap()),
        };
        assert_eq!(reencoded, bytes, "{name} decodes to the same message");
    }
}

/// Writes the golden fixtures that do not exist yet. Run it by hand when
/// adding fixtures; never to make a failing comparison pass.
///
/// It only ever creates files: an existing fixture is the permanent record
/// of what a shipped host sends, so rewriting one from the current encoder
/// would hide exactly the drift the fixtures exist to catch.
#[test]
#[ignore = "writes tests/golden; run by hand only when adding fixtures"]
fn write_golden_fixtures() {
    use std::io::Write as _;
    std::fs::create_dir_all(golden_dir()).unwrap();
    for (name, bytes) in fixtures() {
        let path = golden_dir().join(format!("{name}.hex"));
        match std::fs::File::create_new(&path) {
            Ok(mut file) => file.write_all(hex(&bytes).as_bytes()).unwrap(),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("writing {}: {error}", path.display()),
        }
    }
}

/// Which client message `message` is, numbered from 0 to
/// [`CLIENT_VARIANTS`] - 1. The match has no catch-all arm, so a new
/// message does not compile until it is numbered here, and
/// [`every_message_has_a_golden_fixture_and_every_fixture_a_message`] then
/// fails until it has a fixture.
fn client_variant(message: &ClientMessage) -> usize {
    match message {
        ClientMessage::Hello { .. } => 0,
        ClientMessage::Info => 1,
        ClientMessage::Read(_) => 2,
        ClientMessage::Snapshot { .. } => 3,
        ClientMessage::Input(_) => 4,
        ClientMessage::Resize(_) => 5,
        ClientMessage::Shutdown => 6,
    }
}
const CLIENT_VARIANTS: usize = 7;

/// As [`client_variant`], for host messages.
fn host_variant(message: &HostMessage) -> usize {
    match message {
        HostMessage::Welcome { .. } => 0,
        HostMessage::Busy => 1,
        HostMessage::InfoReply(_) => 2,
        HostMessage::SnapshotRecord { .. } => 3,
        HostMessage::Output { .. } => 4,
        HostMessage::Resized { .. } => 5,
        HostMessage::Gap { .. } => 6,
        HostMessage::Exited { .. } => 7,
        HostMessage::SnapshotAccepted(_) => 8,
        HostMessage::SnapshotRejected { .. } => 9,
        HostMessage::InputAck { .. } => 10,
        HostMessage::Error { .. } => 11,
        HostMessage::Goodbye => 12,
    }
}
const HOST_VARIANTS: usize = 13;

/// Every message of version 1 has a golden fixture, and every fixture file
/// is one the golden test reads.
///
/// Why: the golden test checks only the fixtures it is given. A message
/// added without one, or a fixture dropped from the list while its file
/// stays (in a cleanup, say), would pass it, and the permanent record of
/// version 1's bytes (SPEC.md, "Change policy") would silently have a hole.
/// Spec: the files in `tests/golden/v1/` are exactly the fixtures' names
/// with `.hex`; the fixtures decode to every client message, every host
/// message and the child spec at least once.
#[test]
fn every_message_has_a_golden_fixture_and_every_fixture_a_message() {
    let mut files: Vec<String> = std::fs::read_dir(golden_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    let mut names: Vec<String> = fixtures()
        .iter()
        .map(|(name, _)| format!("{name}.hex"))
        .collect();
    names.sort();
    assert_eq!(files, names, "fixture files and fixtures");
    let (mut clients, mut hosts, mut spawns) = (Vec::new(), Vec::new(), 0);
    for (_, bytes) in fixtures() {
        let frame = split_frame(&bytes, usize::MAX).unwrap().unwrap();
        match frame.kind {
            0x01 => spawns += 1,
            0x10..=0x7f => clients.push(client_variant(&decode_client(frame, 1).unwrap())),
            _ => hosts.push(host_variant(&decode_host(frame, 1).unwrap())),
        }
    }
    for (seen, count, what) in [
        (&mut clients, CLIENT_VARIANTS, "client"),
        (&mut hosts, HOST_VARIANTS, "host"),
    ] {
        seen.sort();
        seen.dedup();
        assert_eq!(
            *seen,
            (0..count).collect::<Vec<_>>(),
            "{what} messages with a fixture"
        );
    }
    assert!(spawns > 0, "the child spec has a fixture");
}

fn size() -> impl Strategy<Value = Size> {
    (1u16..=10_000, 1u16..=10_000).prop_map(|(cols, rows)| Size { cols, rows })
}

fn position() -> impl Strategy<Value = Position> {
    (any::<u64>(), any::<u64>()).prop_map(|(offset, resizes)| Position { offset, resizes })
}

fn exit() -> impl Strategy<Value = ChildExit> {
    prop_oneof![
        any::<i32>().prop_map(ChildExit::Code),
        any::<i32>().prop_map(ChildExit::Signal)
    ]
}

fn bytes(max: usize) -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..max)
}

fn client_message() -> impl Strategy<Value = ClientMessage> {
    prop_oneof![
        (any::<u32>(), any::<bool>())
            .prop_map(|(version, take_over)| ClientMessage::Hello { version, take_over }),
        Just(ClientMessage::Info),
        position().prop_map(ClientMessage::Read),
        (position(), bytes(512))
            .prop_map(|(position, blob)| ClientMessage::Snapshot { position, blob }),
        proptest::collection::vec(any::<u8>(), 1..512).prop_map(ClientMessage::Input),
        size().prop_map(ClientMessage::Resize),
        Just(ClientMessage::Shutdown),
    ]
}

fn host_message() -> impl Strategy<Value = HostMessage> {
    let info = (
        any::<u32>(),
        any::<u32>(),
        proptest::option::of(exit()),
        (size(), size()),
        position(),
        (any::<u64>(), any::<u64>(), any::<u64>()),
        any::<bool>(),
        any::<u64>(),
        bytes(64),
    )
        .prop_map(
            |(
                host_pid,
                child_pid,
                child_exit,
                (size, initial_size),
                snapshot,
                (tail_start, end, resizes),
                gap,
                delivered_end,
                tag,
            )| {
                HostMessage::InfoReply(Info {
                    host_pid,
                    child_pid,
                    child_exit,
                    size,
                    initial_size,
                    snapshot,
                    tail_start,
                    end,
                    resizes,
                    gap,
                    delivered_end,
                    tag,
                })
            },
        );
    prop_oneof![
        any::<u32>().prop_map(|version| HostMessage::Welcome { version }),
        Just(HostMessage::Busy),
        info,
        (position(), bytes(512))
            .prop_map(|(position, blob)| HostMessage::SnapshotRecord { position, blob }),
        (any::<u64>(), proptest::collection::vec(any::<u8>(), 1..512))
            .prop_map(|(offset, data)| HostMessage::Output { offset, data }),
        (any::<u64>(), any::<u64>(), size()).prop_map(|(offset, index, size)| {
            HostMessage::Resized {
                offset,
                index,
                size,
            }
        }),
        (any::<u64>(), any::<u64>(), any::<u64>(), size()).prop_map(|(from, to, resizes, size)| {
            HostMessage::Gap {
                from,
                to,
                resizes,
                size,
            }
        }),
        (any::<u64>(), exit()).prop_map(|(offset, exit)| HostMessage::Exited { offset, exit }),
        position().prop_map(HostMessage::SnapshotAccepted),
        (position(), 1u8..=4).prop_map(|(position, reason)| HostMessage::SnapshotRejected {
            position,
            reason: match reason {
                1 => SnapshotRejection::TooLarge,
                2 => SnapshotRejection::BeforeCurrent,
                3 => SnapshotRejection::OutsideRetained,
                _ => SnapshotRejection::InconsistentResizes,
            },
        }),
        (any::<u64>(), any::<u64>())
            .prop_map(|(written, discarded)| HostMessage::InputAck { written, discarded }),
        ("[ -~]{0,200}", 1u16..=3).prop_map(|(message, code)| HostMessage::Error {
            code: match code {
                1 => ErrorCode::Malformed,
                2 => ErrorCode::NotAllowed,
                _ => ErrorCode::UnsupportedVersion,
            },
            message,
        }),
        Just(HostMessage::Goodbye),
    ]
}

proptest! {
    /// Every client and host message survives encoding, splitting and
    /// decoding unchanged, even when its frame arrives one byte at a time
    /// behind other frames.
    ///
    /// Why: a stream reader sees frames cut at arbitrary points; the
    /// splitter must neither return a frame early nor lose the boundary.
    /// Spec: split_frame returns nothing until a frame is complete, then the
    /// frame, and decoding gives back the original message.
    #[test]
    fn messages_round_trip_through_a_byte_stream(
        client in proptest::collection::vec(client_message(), 1..8),
        host in proptest::collection::vec(host_message(), 1..8),
    ) {
        let stream: Vec<u8> = client.iter().flat_map(encode_client).collect();
        let mut buf = Vec::new();
        let mut decoded = Vec::new();
        for byte in stream {
            buf.push(byte);
            if let Some(frame) = split_frame(&buf, MAX_FRAME).unwrap() {
                decoded.push(decode_client(frame, 1).unwrap());
                let consumed = frame.consumed;
                buf.drain(..consumed);
            }
        }
        prop_assert!(buf.is_empty());
        prop_assert_eq!(decoded, client);

        let stream: Vec<u8> = host.iter().flat_map(encode_host).collect();
        let mut rest = &stream[..];
        let mut decoded = Vec::new();
        while let Some(frame) = split_frame(rest, MAX_FRAME).unwrap() {
            decoded.push(decode_host(frame, 1).unwrap());
            rest = &rest[frame.consumed..];
        }
        prop_assert!(rest.is_empty());
        prop_assert_eq!(decoded, host);
    }

    /// Any byte string is either a valid message, an incomplete frame or
    /// malformed, and decoding never panics.
    ///
    /// Why: the host decodes whatever a peer sends; a panic there would end
    /// the host and its terminal. Spec: split_frame and both decoders return
    /// a value or an error for arbitrary input, never a panic.
    #[test]
    fn arbitrary_bytes_never_panic_the_decoders(input in bytes(256)) {
        if let Ok(Some(frame)) = split_frame(&input, MAX_FRAME) {
            let _ = decode_client(frame, 1);
            let _ = decode_host(frame, 1);
        }
        let _ = decode_spawn(&input);
    }
}

/// Malformed frames are refused for each reason SPEC.md names, and an
/// oversized length is refused before its body arrives.
///
/// Why: strictness is what keeps every byte this version accepts something
/// a later version can still rely on; a lenient decoder would let peers
/// depend on accidents. Spec: a frame of exactly the length limit splits;
/// an empty frame, a length above the limit, a body with trailing bytes, a
/// body cut short, a flag other than 0 or 1, a size of 0, empty input,
/// input above the limit, and unknown kinds are all malformed.
#[test]
fn malformed_frames_are_refused() {
    let refused = |bytes: &[u8]| -> Malformed {
        let frame = split_frame(bytes, MAX_FRAME)
            .expect("the frame itself splits")
            .expect("whole frame");
        decode_client(frame, 1).expect_err("malformed")
    };
    assert_eq!(
        split_frame(&[0, 0, 0, 0], MAX_FRAME),
        Err(Malformed("empty frame"))
    );
    assert_eq!(
        split_frame(&[0x7f, 0xff, 0xff, 0xff], MAX_FRAME),
        Err(Malformed("frame too long")),
        "refused from the length alone"
    );
    assert_eq!(
        split_frame(&[0, 0, 0, 5, 0x11], MAX_FRAME),
        Ok(None),
        "incomplete"
    );
    assert_eq!(
        split_frame(&[0, 0, 0, 3, 0x11, 1, 2], 3).map(|frame| frame.map(|f| f.consumed)),
        Ok(Some(7)),
        "a frame of exactly the limit is accepted"
    );
    assert_eq!(
        split_frame(&[0, 0, 0, 4, 0x11, 1, 2, 3], 3),
        Err(Malformed("frame too long")),
        "one byte over the limit is refused"
    );
    // Info with a trailing byte.
    refused(&[0, 0, 0, 2, 0x11, 0]);
    // Read cut short.
    refused(&[0, 0, 0, 5, 0x12, 0, 0, 0, 0]);
    // Hello with take_over = 2.
    refused(&[0, 0, 0, 6, 0x10, 0, 0, 0, 1, 2]);
    // Resize to 0 columns.
    refused(&[0, 0, 0, 5, 0x15, 0, 0, 0, 24]);
    // Empty input.
    refused(&[0, 0, 0, 5, 0x14, 0, 0, 0, 0]);
    // Input above the limit.
    let big = encode_client(&ClientMessage::Input(vec![b'x'; MAX_INPUT + 1]));
    let refusal = refused(&big);
    assert_eq!(refusal, Malformed("field too long"));
    // Unknown kinds, and a host kind sent by a client.
    refused(&[0, 0, 0, 1, 0x17]);
    refused(&[0, 0, 0, 1, 0x80]);
}

/// The child spec is checked for everything SPEC.md requires of it.
///
/// Why: the host must refuse a spec it cannot honor at start, while the
/// caller is still waiting, rather than fail later with nobody watching.
/// Spec: an empty argv, a NUL byte, a relative working directory, an
/// environment entry without `=` (or with an empty name), a cap out of
/// range, a size of 0, another spec version, and bytes after the frame are
/// all refused.
#[test]
fn invalid_child_specs_are_refused() {
    let valid = Spawn {
        argv: vec![b"/bin/sh".to_vec()],
        cwd: b"/".to_vec(),
        env: vec![b"A=1".to_vec()],
        size: Size { cols: 80, rows: 24 },
        journal_cap: 64 * 1024,
        tag: vec![],
        initial_snapshot: None,
    };
    assert_eq!(decode_spawn(&encode_spawn(&valid)), Ok(valid.clone()));
    // Each case is refused for its own reason, not some other check's.
    let refused = |change: &dyn Fn(&mut Spawn), reason: &'static str| {
        let mut spawn = valid.clone();
        change(&mut spawn);
        assert_eq!(
            decode_spawn(&encode_spawn(&spawn)),
            Err(Malformed(reason)),
            "{spawn:?}"
        );
    };
    refused(&|s| s.argv.clear(), "empty argv");
    refused(&|s| s.argv = vec![vec![]], "empty argv");
    refused(
        &|s| s.argv = vec![b"a\0b".to_vec()],
        "NUL byte in argv, env or cwd",
    );
    refused(
        &|s| s.cwd = b"relative".to_vec(),
        "working directory is not absolute",
    );
    refused(
        &|s| s.env = vec![b"NOEQUALS".to_vec()],
        "environment entry is not NAME=value",
    );
    refused(
        &|s| s.env = vec![b"=value".to_vec()],
        "environment entry is not NAME=value",
    );
    refused(&|s| s.journal_cap = 1, "journal cap out of range");
    refused(&|s| s.journal_cap = u64::MAX, "journal cap out of range");
    refused(
        &|s| s.size = Size { cols: 0, rows: 24 },
        "size out of range",
    );
    let mut trailing = encode_spawn(&valid);
    trailing.push(0);
    assert_eq!(
        decode_spawn(&trailing),
        Err(Malformed("bytes after the child spec"))
    );
    let mut other_version = encode_spawn(&valid);
    other_version[8] = 2; // the spec version's low byte, after length and kind
    assert_eq!(
        decode_spawn(&other_version),
        Err(Malformed("unsupported child spec version"))
    );
}

/// A long error message is cut at a character boundary, so the limit never
/// turns a message into invalid text.
///
/// Why: the host builds error text from its own diagnostics, which may
/// contain any character; a cut mid-character would make the client's
/// strict decoder refuse the very message meant to explain a refusal.
/// Spec: a 2000-byte message of multi-byte characters encodes to at most
/// 1024 bytes of valid UTF-8 and decodes.
#[test]
fn long_error_messages_are_cut_at_a_character_boundary() {
    let message = "é".repeat(1000);
    let bytes = encode_host(&HostMessage::Error {
        code: ErrorCode::Malformed,
        message,
    });
    let frame = split_frame(&bytes, MAX_FRAME).unwrap().unwrap();
    let HostMessage::Error { message, .. } = decode_host(frame, 1).unwrap() else {
        panic!("an error");
    };
    assert!(message.len() <= 1024 && message.chars().all(|c| c == 'é'));
}

/// Every refusal the decoders make for a well-framed but invalid body.
///
/// Why: each is a promise that a version-1 peer never relies on that byte
/// pattern; a refusal path that silently accepted would become one. Spec:
/// a list count larger than the body, a list over its byte limit, an
/// unknown exit kind, a frame of another kind given as a child spec, a
/// running child with a nonzero exit field, an unknown child state, empty
/// output, an unknown rejection reason, an unknown error code, a non-UTF-8
/// error message and an unknown host kind are all malformed.
#[test]
fn invalid_bodies_are_refused() {
    let host = |kind: u8, body: &[u8]| -> Result<HostMessage, Malformed> {
        let mut bytes = ((body.len() + 1) as u32).to_be_bytes().to_vec();
        bytes.push(kind);
        bytes.extend_from_slice(body);
        decode_host(split_frame(&bytes, MAX_FRAME).unwrap().unwrap(), 1)
    };
    let u64s =
        |values: &[u64]| -> Vec<u8> { values.iter().flat_map(|v| v.to_be_bytes()).collect() };
    // Exited with an unknown status kind.
    let mut body = u64s(&[1]);
    body.extend([7, 0, 0, 0, 0]);
    assert!(host(0x88, &body).is_err());
    // InfoReply: running with a nonzero exit value, and an unknown state.
    let info = |state: u8, status: u8, value: i32| {
        let mut body = vec![0, 0, 0, 1, 0, 0, 0, 2, state, status];
        body.extend(value.to_be_bytes());
        // The size now and the initial size.
        body.extend([0, 80, 0, 24, 0, 80, 0, 24]);
        body.extend(u64s(&[0, 0, 0, 0, 0]));
        body.push(0);
        body.extend(u64s(&[0]));
        body.extend([0, 0, 0, 0]);
        body
    };
    assert!(
        host(0x83, &info(0, 0, 0)).is_ok(),
        "the well-formed variant decodes"
    );
    assert!(host(0x83, &info(0, 0, 1)).is_err());
    assert!(host(0x83, &info(2, 0, 0)).is_err());
    // Empty output.
    let mut body = u64s(&[0]);
    body.extend([0, 0, 0, 0]);
    assert!(host(0x85, &body).is_err());
    // Unknown rejection reason.
    let mut body = u64s(&[0, 0]);
    body.push(9);
    assert!(host(0x8a, &body).is_err());
    // Unknown error code, and a non-UTF-8 message.
    assert!(host(0x8c, &[0, 9, 0, 0, 0, 0]).is_err());
    assert!(host(0x8c, &[0, 1, 0, 0, 0, 1, 0xff]).is_err());
    // Unknown host kind.
    assert!(host(0x9f, &[]).is_err());
    // A child spec whose argv count is larger than its body could hold.
    let mut spec = vec![0, 0, 0, 1];
    spec.extend([0xff, 0xff, 0xff, 0xff]);
    let mut bytes = ((spec.len() + 1) as u32).to_be_bytes().to_vec();
    bytes.push(0x01);
    bytes.extend(&spec);
    assert_eq!(
        decode_spawn(&bytes),
        Err(Malformed("list count exceeds the body"))
    );
    // A child spec whose environment is larger than the spec allows in total.
    let big = Spawn {
        argv: vec![b"/bin/sh".to_vec()],
        cwd: b"/".to_vec(),
        env: vec![vec![b'A'; 9 * 1024 * 1024]; 4]
            .into_iter()
            .map(|mut entry| {
                entry[1] = b'=';
                entry
            })
            .collect(),
        size: Size { cols: 80, rows: 24 },
        journal_cap: 64 * 1024,
        tag: vec![],
        initial_snapshot: None,
    };
    assert!(
        decode_spawn(&encode_spawn(&big)).is_err(),
        "a spec beyond its limit"
    );
    // A host frame given as a child spec.
    assert_eq!(
        decode_spawn(&encode_host(&HostMessage::Busy)),
        Err(Malformed("not a child spec"))
    );
}

/// A cut that lands inside a three-byte character backs off to the
/// character before it.
///
/// Why: complements the two-byte case above, where every cut point is
/// already a boundary. Spec: 1000 euro signs encode to a message of whole
/// characters under the limit.
#[test]
fn an_error_message_cut_backs_off_a_three_byte_character() {
    let bytes = encode_host(&HostMessage::Error {
        code: ErrorCode::NotAllowed,
        message: "€".repeat(1000),
    });
    let frame = split_frame(&bytes, MAX_FRAME).unwrap().unwrap();
    let HostMessage::Error { message, .. } = decode_host(frame, 1).unwrap() else {
        panic!("an error");
    };
    assert_eq!(message.len(), 1023);
}

/// A list whose item count the rest of the body cannot hold is refused from
/// the count alone, before anything is allocated for it.
///
/// Why: the count is a u32 from the peer; allocating for it first would let
/// one small frame ask the host for gigabytes. Spec: a child spec whose argv
/// count is one more than the remaining body could hold (each item needs at
/// least its four-byte length) is refused with "list count exceeds the
/// body".
#[test]
fn a_list_count_the_body_cannot_hold_is_refused_from_the_count() {
    // kind, spec version 1, argv count 3, then only 8 more bytes.
    let mut body = vec![0x01];
    body.extend(1u32.to_be_bytes());
    body.extend(3u32.to_be_bytes());
    body.extend([0u8; 8]);
    let mut input = (body.len() as u32).to_be_bytes().to_vec();
    input.extend(body);
    assert_eq!(
        decode_spawn(&input),
        Err(Malformed("list count exceeds the body"))
    );
}

/// A refusal's text says it is a malformed frame and why.
///
/// Why: the host writes it to stderr or into an `Error` message, where it is
/// the only diagnostic a refused peer gets. Spec: `Malformed("x")` displays
/// as "malformed frame: x".
#[test]
fn a_refusal_displays_its_reason() {
    assert_eq!(Malformed("x").to_string(), "malformed frame: x");
}

/// Every rejection reason and error code has the number SPEC.md gives it,
/// on the wire as well as in the enum.
///
/// Why: these numbers are permanent, and the round-trip property test cannot
/// see a renumbering made consistently in the enum and the decoder; only one
/// value of each has a golden fixture. Spec: SPEC.md, "Host messages":
/// rejection reasons 1 too large, 2 before the current snapshot, 3 outside
/// retained output, 4 not consistent with the resizes; error codes 1
/// malformed, 2 not allowed, 3 unsupported version.
#[test]
fn rejection_reasons_and_error_codes_have_their_permanent_numbers() {
    for (reason, number) in [
        (SnapshotRejection::TooLarge, 1u8),
        (SnapshotRejection::BeforeCurrent, 2),
        (SnapshotRejection::OutsideRetained, 3),
        (SnapshotRejection::InconsistentResizes, 4),
    ] {
        let bytes = encode_host(&HostMessage::SnapshotRejected {
            position: Position::default(),
            reason,
        });
        assert_eq!(*bytes.last().unwrap(), number, "{reason:?}");
    }
    for (code, number) in [
        (ErrorCode::Malformed, 1u16),
        (ErrorCode::NotAllowed, 2),
        (ErrorCode::UnsupportedVersion, 3),
    ] {
        let bytes = encode_host(&HostMessage::Error {
            code,
            message: String::new(),
        });
        // length, kind, then the code.
        assert_eq!(u16::from_be_bytes([bytes[5], bytes[6]]), number, "{code:?}");
    }
}

/// Any frame body, under every kind byte, is decoded or refused without a
/// panic, by every decoder.
///
/// Why: the decoders face a peer that may be broken or hostile, and a panic
/// would end a host its program depends on. Random bytes alone almost never
/// get past the length check, so this frames each body correctly first.
/// Spec: SPEC.md, "Framing": a malformed frame is refused, never anything
/// worse.
#[test]
fn any_framed_body_never_panics_the_decoders() {
    proptest!(|(kind in any::<u8>(), body in bytes(300))| {
        let mut input = (body.len() as u32 + 1).to_be_bytes().to_vec();
        input.push(kind);
        input.extend(&body);
        let frame = split_frame(&input, MAX_FRAME).unwrap().unwrap();
        let _ = decode_client(frame, 1);
        let frame = split_frame(&input, MAX_FRAME).unwrap().unwrap();
        let _ = decode_host(frame, 1);
        let _ = decode_spawn(&input);
    });
}

/// The decoders refuse a protocol version they do not know, and kind 0x82
/// is refused in version 1.
///
/// Why: what a frame may hold, and its limits, belong to the version that
/// states them (SPEC.md, "Change policy"), so a decoder must never apply
/// version 1's rules to another version's frames; and 0x82 is reserved
/// forever, never sent (it was to be a displacement notice), so a peer that
/// sent one is broken. Spec: a well-formed `Info` and `Goodbye` decode as
/// version 1 and are refused as versions 0 and 2; a frame of kind 0x82 with
/// an empty body is malformed for both decoders.
#[test]
fn unknown_versions_and_the_reserved_kind_are_refused() {
    let info = encode_client(&ClientMessage::Info);
    let goodbye = encode_host(&HostMessage::Goodbye);
    fn frame(bytes: &[u8]) -> terminal_host::protocol::RawFrame<'_> {
        split_frame(bytes, MAX_FRAME).unwrap().unwrap()
    }
    assert_eq!(decode_client(frame(&info), 1), Ok(ClientMessage::Info));
    assert_eq!(decode_host(frame(&goodbye), 1), Ok(HostMessage::Goodbye));
    for version in [0, 2] {
        assert!(
            decode_client(frame(&info), version).is_err(),
            "client v{version}"
        );
        assert!(
            decode_host(frame(&goodbye), version).is_err(),
            "host v{version}"
        );
    }
    let reserved = [0, 0, 0, 1, 0x82];
    assert!(decode_host(frame(&reserved), 1).is_err());
    assert!(decode_client(frame(&reserved), 1).is_err());
}

/// A child spec frame of exactly `MAX_SPEC` decodes, and one byte more is
/// refused from its length.
///
/// Why: `MAX_SPEC` is the limit on the spec's length field, not on the
/// bytes read (SPEC.md, "Command line and child spec"); a decoder off by
/// one either way would refuse the largest spec a caller may send, or
/// accept one the host is not obliged to read. Spec: a valid spec padded
/// with an environment entry to a length field of exactly `MAX_SPEC`
/// decodes to that spec; the same with one more byte of padding is refused
/// as too long.
#[test]
fn a_child_spec_of_exactly_its_limit_decodes() {
    let spec = |padding: usize| {
        let mut entry = b"PAD=".to_vec();
        entry.resize(4 + padding, b'x');
        Spawn {
            argv: vec![b"/bin/sh".to_vec()],
            cwd: b"/".to_vec(),
            env: vec![entry],
            size: Size { cols: 80, rows: 24 },
            journal_cap: 64 * 1024,
            tag: vec![],
            initial_snapshot: None,
        }
    };
    let base = encode_spawn(&spec(0)).len() - 4;
    let at_limit = spec(terminal_host::protocol::MAX_SPEC - base);
    let bytes = encode_spawn(&at_limit);
    assert_eq!(bytes.len(), terminal_host::protocol::MAX_SPEC + 4);
    assert_eq!(decode_spawn(&bytes), Ok(at_limit));
    let over = encode_spawn(&spec(terminal_host::protocol::MAX_SPEC - base + 1));
    assert_eq!(decode_spawn(&over), Err(Malformed("frame too long")));
}
