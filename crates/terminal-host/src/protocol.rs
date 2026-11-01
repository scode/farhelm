//! The wire format: frames, every message of every protocol version, the
//! child spec, and the limits SPEC.md states.
//!
//! Pure: nothing here does I/O, so a client can drive the protocol over any
//! transport (a blocking socket, an async one, a test buffer) and the host's
//! event loop and the tests share one definition of every byte. Everything
//! here is permanent once shipped (SPEC.md, "Change policy"): a message or
//! field is never changed or removed, and the golden fixtures in
//! `tests/golden/` pin the exact bytes.
//!
//! Decoding is strict. A frame with trailing bytes, an option tag other than
//! 0 or 1, a field outside its stated range or an unknown kind is malformed,
//! never guessed at: a lenient decoder would let a peer depend on bytes this
//! version never meant to accept, which a later version could then not
//! change.
//!
//! The limits below are version 1's. A limit on what one frame may hold or
//! one connection may send belongs to the protocol version that states it
//! (SPEC.md, "Change policy"): a later version may raise one only for
//! connections that negotiated that version, so a v2 author adds per-version
//! limits beside these instead of editing them, and the decoders take the
//! negotiated version for that reason. Limits on what the host keeps for
//! every client ([`MAX_SNAPSHOT`], [`MAX_TAG`]) and the host-wide timings are
//! not per connection, and hold whatever version a connection negotiated.

use std::fmt;

/// The protocol version this crate speaks, and the highest it offers.
pub const PROTOCOL_VERSION: u32 = 1;
/// The child spec format this crate reads.
pub const SPEC_VERSION: u32 = 1;

/// Longest frame body (kind byte included), in bytes. The child spec is the
/// one frame with a limit of its own, [`MAX_SPEC`].
pub const MAX_FRAME: usize = 16 * 1024 * 1024 + 64 * 1024;
/// Longest child spec frame body (kind byte included), in bytes: the limit
/// on its length field, so a host reads at most this plus the four bytes of
/// that field from stdin.
pub const MAX_SPEC: usize = 32 * 1024 * 1024 + 64 * 1024;
/// Largest snapshot blob, in bytes.
pub const MAX_SNAPSHOT: usize = 16 * 1024 * 1024;
/// Longest client tag, in bytes.
pub const MAX_TAG: usize = 4 * 1024;
/// Smallest journal cap a spawn may ask for.
pub const MIN_JOURNAL_CAP: u64 = 64 * 1024;
/// Largest journal cap a spawn may ask for.
pub const MAX_JOURNAL_CAP: u64 = 1024 * 1024 * 1024;
/// What one resize record costs against the journal cap.
pub const RECORD_COST: u64 = 64;
/// Longest `Input`, in bytes.
pub const MAX_INPUT: usize = 64 * 1024;
/// Inputs up to this size are offered to the pty whole, in one `write`
/// call, so the host never splits a key's bytes itself. The pty may still
/// take only part of one when its input queue is nearly full, and how the
/// program's reads divide them is not the host's to promise (SPEC.md,
/// "Input").
pub const ATOMIC_INPUT: usize = 64;
/// Most pending input the host holds, counting each pending `Input` as its
/// unwritten bytes plus [`INPUT_COST`]. An `Input` that does not fit with
/// what is pending waits, with everything the client sent after it, until
/// it does.
pub const MAX_PENDING_INPUT: usize = 1024 * 1024;
/// What each pending `Input` costs against [`MAX_PENDING_INPUT`] beyond its
/// data. Holding one really costs a queue entry of 32 bytes, up to as much
/// again in room the queue's storage keeps to grow into, and the
/// allocator's header and rounding on its data: between about 64 and 96
/// bytes. So pending input takes less than one and a half times the bound
/// however small the `Input`s are; without this cost, a client pipelining
/// one-byte `Input`s could make the host hold a million queue entries, tens
/// of MiB, under a bound that says 1 MiB.
pub const INPUT_COST: usize = 64;
/// Most unsent bytes queued for the client (replies and stream records)
/// before the host stops acting on its messages and adding to its stream.
/// The queue holds at most this plus one frame, plus acknowledgements: one
/// is never queued right behind another that has not started, so they
/// number at most two more than the other frames (SPEC.md, "Resource
/// bounds", which also says what the frames cost beyond their bytes).
pub const OUTGOING_HIGH_WATER: usize = 256 * 1024;
/// Longest `Output` chunk the host sends.
pub const MAX_OUTPUT_CHUNK: usize = 64 * 1024;
/// Most connections that may wait for their `Hello` at once.
pub const MAX_PENDING_CONNECTIONS: usize = 8;
/// Most the host reads from a connection before its `Hello`. A `Hello` is
/// 10 bytes in every version, so one that sent this much without a whole
/// frame sent something else first.
pub const MAX_BEFORE_HELLO: usize = 64;
/// How long a new connection may take to send `Hello`.
pub const HELLO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
/// How long the host keeps writing `Goodbye` after `Shutdown` before it
/// exits without it, and an `Error` before it closes the connection without
/// it.
pub const GOODBYE_FLUSH: std::time::Duration = std::time::Duration::from_millis(1000);
/// How long the host keeps reading output after reaping its child before it
/// journals the exit.
pub const EXIT_DRAIN_TIME: std::time::Duration = std::time::Duration::from_millis(200);
/// How much output the host reads after reaping its child before it
/// journals the exit.
pub const EXIT_DRAIN_BYTES: usize = 4 * 1024 * 1024;
/// Longest `Error` message, in bytes.
pub const MAX_ERROR_MESSAGE: usize = 1024;
/// Largest terminal dimension, in cells, either way.
pub const MAX_DIMENSION: u16 = 10_000;

/// Message kinds. Client kinds are below 0x80, host kinds at or above it, and
/// the spawn frame (read only from stdin) is 0x01.
///
/// 0x82 is reserved: it was once meant for a `Displaced` notice to a client
/// that lost its connection to a takeover, and was left out of version 1
/// before it shipped (an end of stream already tells the old client, and it
/// had to handle that anyway). No version may give 0x82 another meaning.
pub mod kind {
    pub const SPAWN: u8 = 0x01;
    pub const HELLO: u8 = 0x10;
    pub const INFO: u8 = 0x11;
    pub const READ: u8 = 0x12;
    pub const SNAPSHOT: u8 = 0x13;
    pub const INPUT: u8 = 0x14;
    pub const RESIZE: u8 = 0x15;
    pub const SHUTDOWN: u8 = 0x16;
    pub const WELCOME: u8 = 0x80;
    pub const BUSY: u8 = 0x81;
    pub const INFO_REPLY: u8 = 0x83;
    pub const SNAPSHOT_RECORD: u8 = 0x84;
    pub const OUTPUT: u8 = 0x85;
    pub const RESIZED: u8 = 0x86;
    pub const GAP: u8 = 0x87;
    pub const EXITED: u8 = 0x88;
    pub const SNAPSHOT_ACCEPTED: u8 = 0x89;
    pub const SNAPSHOT_REJECTED: u8 = 0x8a;
    pub const INPUT_ACK: u8 = 0x8b;
    pub const ERROR: u8 = 0x8c;
    pub const GOODBYE: u8 = 0x8d;
}

/// A terminal size in cells. Each dimension is 1 to [`MAX_DIMENSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

impl Size {
    /// Whether both dimensions are in range.
    pub fn is_valid(self) -> bool {
        (1..=MAX_DIMENSION).contains(&self.cols) && (1..=MAX_DIMENSION).contains(&self.rows)
    }
}

/// A point in the journal: every output byte before `offset` and every resize
/// numbered below `resizes` lie before it (SPEC.md, "Terms").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Position {
    pub offset: u64,
    pub resizes: u64,
}

/// How the child ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChildExit {
    /// It exited with this status.
    Code(i32),
    /// A signal with this number ended it.
    Signal(i32),
}

/// The child spec: everything about the child, read from the host's stdin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spawn {
    pub argv: Vec<Vec<u8>>,
    pub cwd: Vec<u8>,
    pub env: Vec<Vec<u8>>,
    pub size: Size,
    pub journal_cap: u64,
    pub tag: Vec<u8>,
    pub initial_snapshot: Option<Vec<u8>>,
}

/// Why the host rejected a snapshot. The discriminants are the wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SnapshotRejection {
    TooLarge = 1,
    BeforeCurrent = 2,
    OutsideRetained = 3,
    InconsistentResizes = 4,
}

/// Why the host closed a connection with an error. The discriminants are the
/// wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ErrorCode {
    Malformed = 1,
    NotAllowed = 2,
    UnsupportedVersion = 3,
}

/// The host's answer to `Info`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    pub host_pid: u32,
    pub child_pid: u32,
    /// `None` while the child runs.
    pub child_exit: Option<ChildExit>,
    /// The size in force now.
    pub size: Size,
    /// The size given at spawn: where output that follows no snapshot
    /// record and no gap starts.
    pub initial_size: Size,
    /// Position of the stored snapshot ((0, 0) when there is none).
    pub snapshot: Position,
    /// Offset where retained output starts (`end` when none is retained).
    pub tail_start: u64,
    /// Offset of the next output byte.
    pub end: u64,
    /// Resizes applied so far.
    pub resizes: u64,
    /// Whether the tail starts with a gap.
    pub gap: bool,
    /// A conservative watermark of what clients were sent: the highest
    /// offset at which an `Output` frame written whole to any client's
    /// socket ended. It never decreases. No whole `Output` frame extends past
    /// it; output below it may have been sent, but not necessarily all of
    /// it (SPEC.md, "Reading").
    pub delivered_end: u64,
    pub tag: Vec<u8>,
}

/// A message from the client to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientMessage {
    Hello { version: u32, take_over: bool },
    Info,
    Read(Position),
    Snapshot { position: Position, blob: Vec<u8> },
    Input(Vec<u8>),
    Resize(Size),
    Shutdown,
}

/// A message from the host to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostMessage {
    Welcome {
        version: u32,
    },
    Busy,
    InfoReply(Info),
    SnapshotRecord {
        position: Position,
        blob: Vec<u8>,
    },
    Output {
        offset: u64,
        data: Vec<u8>,
    },
    Resized {
        offset: u64,
        index: u64,
        size: Size,
    },
    Gap {
        from: u64,
        to: u64,
        resizes: u64,
        size: Size,
    },
    Exited {
        offset: u64,
        exit: ChildExit,
    },
    SnapshotAccepted(Position),
    SnapshotRejected {
        position: Position,
        reason: SnapshotRejection,
    },
    /// The fate of the `Input`s resolved so far on this connection, each
    /// count cumulative: `written` bytes reached the pty, `discarded` bytes
    /// were dropped (the terminal had no reader, or a write failed). Their
    /// sum is the total size of the resolved `Input`s, which are always a
    /// prefix of those sent.
    InputAck {
        written: u64,
        discarded: u64,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
    Goodbye,
}

/// A frame or message that breaks the format. The text names what was wrong,
/// for diagnostics; callers must not match on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed(pub &'static str);

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "malformed frame: {}", self.0)
    }
}

impl std::error::Error for Malformed {}

// ---- Framing ----------------------------------------------------------------

/// One frame split off the front of a buffer: its kind, its body and how
/// many bytes of the buffer it used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawFrame<'a> {
    pub kind: u8,
    pub body: &'a [u8],
    pub consumed: usize,
}

/// Split the first frame off `buf`, if a whole one is there.
///
/// `Ok(None)` means more bytes are needed. A length of 0 or above `max` is
/// malformed as soon as the length field is complete, so a peer cannot make
/// the reader buffer an oversized frame before refusing it.
pub fn split_frame(buf: &[u8], max: usize) -> Result<Option<RawFrame<'_>>, Malformed> {
    let Some(length) = buf.get(..4) else {
        return Ok(None);
    };
    let length = u32::from_be_bytes(length.try_into().expect("four bytes")) as usize;
    if length == 0 {
        return Err(Malformed("empty frame"));
    }
    if length > max {
        return Err(Malformed("frame too long"));
    }
    let Some(frame) = buf.get(4..4 + length) else {
        return Ok(None);
    };
    Ok(Some(RawFrame {
        kind: frame[0],
        body: &frame[1..],
        consumed: 4 + length,
    }))
}

/// Builds one frame's body.
struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn new(kind: u8) -> Self {
        // The length is patched in by `finish`.
        Self {
            buf: vec![0, 0, 0, 0, kind],
        }
    }
    fn u8(&mut self, value: u8) -> &mut Self {
        self.buf.push(value);
        self
    }
    fn u16(&mut self, value: u16) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }
    fn u32(&mut self, value: u32) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }
    fn u64(&mut self, value: u64) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }
    fn i32(&mut self, value: i32) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }
    fn bytes(&mut self, value: &[u8]) -> &mut Self {
        self.u32(u32::try_from(value.len()).expect("field longer than 4 GiB"));
        self.buf.extend_from_slice(value);
        self
    }
    fn list(&mut self, values: &[Vec<u8>]) -> &mut Self {
        self.u32(u32::try_from(values.len()).expect("list longer than 4 Gi items"));
        for value in values {
            self.bytes(value);
        }
        self
    }
    fn size(&mut self, size: Size) -> &mut Self {
        self.u16(size.cols).u16(size.rows)
    }
    fn position(&mut self, position: Position) -> &mut Self {
        self.u64(position.offset).u64(position.resizes)
    }
    fn exit(&mut self, exit: ChildExit) -> &mut Self {
        match exit {
            ChildExit::Code(code) => self.u8(0).i32(code),
            ChildExit::Signal(signal) => self.u8(1).i32(signal),
        }
    }
    fn finish(&mut self) -> Vec<u8> {
        let length = u32::try_from(self.buf.len() - 4).expect("frame longer than 4 GiB");
        self.buf[..4].copy_from_slice(&length.to_be_bytes());
        std::mem::take(&mut self.buf)
    }
}

/// Reads one frame's body, field by field.
struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        if self.rest.len() < n {
            return Err(Malformed("body too short"));
        }
        let (head, tail) = self.rest.split_at(n);
        self.rest = tail;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, Malformed> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, Malformed> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().expect("two")))
    }
    fn u32(&mut self) -> Result<u32, Malformed> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().expect("four")))
    }
    fn u64(&mut self) -> Result<u64, Malformed> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().expect("eight")))
    }
    fn i32(&mut self) -> Result<i32, Malformed> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().expect("four")))
    }
    fn bool(&mut self) -> Result<bool, Malformed> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Malformed("flag is neither 0 nor 1")),
        }
    }
    fn bytes(&mut self, max: usize) -> Result<Vec<u8>, Malformed> {
        let length = self.u32()? as usize;
        if length > max {
            return Err(Malformed("field too long"));
        }
        Ok(self.take(length)?.to_vec())
    }
    /// A list of byte strings, each at most `max_item` bytes. The list as a
    /// whole needs no bound of its own: it lies inside one frame, which
    /// [`split_frame`] has already bounded.
    fn list(&mut self, max_item: usize) -> Result<Vec<Vec<u8>>, Malformed> {
        let count = self.u32()? as usize;
        // Every item needs at least its four-byte length, so a count the
        // remaining body cannot hold is refused before anything is allocated.
        if count > self.rest.len() / 4 {
            return Err(Malformed("list count exceeds the body"));
        }
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(self.bytes(max_item)?);
        }
        Ok(items)
    }
    fn size(&mut self) -> Result<Size, Malformed> {
        let size = Size {
            cols: self.u16()?,
            rows: self.u16()?,
        };
        if !size.is_valid() {
            return Err(Malformed("size out of range"));
        }
        Ok(size)
    }
    fn position(&mut self) -> Result<Position, Malformed> {
        Ok(Position {
            offset: self.u64()?,
            resizes: self.u64()?,
        })
    }
    fn exit(&mut self) -> Result<ChildExit, Malformed> {
        match self.u8()? {
            0 => Ok(ChildExit::Code(self.i32()?)),
            1 => Ok(ChildExit::Signal(self.i32()?)),
            _ => Err(Malformed("unknown exit status kind")),
        }
    }
    fn end(self) -> Result<(), Malformed> {
        if self.rest.is_empty() {
            Ok(())
        } else {
            Err(Malformed("trailing bytes"))
        }
    }
}

// ---- The child spec -----------------------------------------------------------

/// Encode a child spec as the one frame a host reads from stdin.
pub fn encode_spawn(spawn: &Spawn) -> Vec<u8> {
    let mut w = Writer::new(kind::SPAWN);
    w.u32(SPEC_VERSION)
        .list(&spawn.argv)
        .bytes(&spawn.cwd)
        .list(&spawn.env)
        .size(spawn.size)
        .u64(spawn.journal_cap)
        .bytes(&spawn.tag);
    match &spawn.initial_snapshot {
        None => w.u8(0),
        Some(blob) => w.u8(1).bytes(blob),
    };
    w.finish()
}

/// Decode a whole child spec, as read from stdin to end of file.
///
/// Everything SPEC.md requires of the spec is checked here: the version, a
/// nonempty argv without NUL bytes, an absolute working directory,
/// `NAME=value` environment entries, the size, cap, tag and snapshot limits,
/// and that nothing follows the frame.
pub fn decode_spawn(input: &[u8]) -> Result<Spawn, Malformed> {
    let frame = split_frame(input, MAX_SPEC)?.ok_or(Malformed("incomplete child spec"))?;
    if frame.consumed != input.len() {
        return Err(Malformed("bytes after the child spec"));
    }
    if frame.kind != kind::SPAWN {
        return Err(Malformed("not a child spec"));
    }
    let mut r = Reader { rest: frame.body };
    if r.u32()? != SPEC_VERSION {
        return Err(Malformed("unsupported child spec version"));
    }
    let argv = r.list(MAX_SPEC)?;
    let cwd = r.bytes(MAX_SPEC)?;
    let env = r.list(MAX_SPEC)?;
    let size = r.size()?;
    let journal_cap = r.u64()?;
    let tag = r.bytes(MAX_TAG)?;
    let initial_snapshot = if r.bool()? {
        Some(r.bytes(MAX_SNAPSHOT)?)
    } else {
        None
    };
    r.end()?;
    if argv.is_empty() || argv[0].is_empty() {
        return Err(Malformed("empty argv"));
    }
    if argv
        .iter()
        .chain(&env)
        .chain([&cwd])
        .any(|item| item.contains(&0))
    {
        return Err(Malformed("NUL byte in argv, env or cwd"));
    }
    if cwd.first() != Some(&b'/') {
        return Err(Malformed("working directory is not absolute"));
    }
    if env.iter().any(|entry| {
        entry
            .iter()
            .position(|&b| b == b'=')
            .is_none_or(|eq| eq == 0)
    }) {
        return Err(Malformed("environment entry is not NAME=value"));
    }
    if !(MIN_JOURNAL_CAP..=MAX_JOURNAL_CAP).contains(&journal_cap) {
        return Err(Malformed("journal cap out of range"));
    }
    Ok(Spawn {
        argv,
        cwd,
        env,
        size,
        journal_cap,
        tag,
        initial_snapshot,
    })
}

// ---- Client messages ------------------------------------------------------------

/// Encode a client message as one frame.
pub fn encode_client(message: &ClientMessage) -> Vec<u8> {
    match message {
        ClientMessage::Hello { version, take_over } => Writer::new(kind::HELLO)
            .u32(*version)
            .u8(u8::from(*take_over))
            .finish(),
        ClientMessage::Info => Writer::new(kind::INFO).finish(),
        ClientMessage::Read(position) => Writer::new(kind::READ).position(*position).finish(),
        ClientMessage::Snapshot { position, blob } => Writer::new(kind::SNAPSHOT)
            .position(*position)
            .bytes(blob)
            .finish(),
        ClientMessage::Input(data) => Writer::new(kind::INPUT).bytes(data).finish(),
        ClientMessage::Resize(size) => Writer::new(kind::RESIZE).size(*size).finish(),
        ClientMessage::Shutdown => Writer::new(kind::SHUTDOWN).finish(),
    }
}

/// Decode one client frame (as split by [`split_frame`]) of the negotiated
/// protocol `version`. A version this crate does not know is refused: what a
/// frame may hold, and its limits, belong to a version.
///
/// A snapshot blob larger than [`MAX_SNAPSHOT`] still decodes (up to the
/// frame limit): rejecting it is the journal's decision, with its own reply,
/// not a malformed frame.
pub fn decode_client(frame: RawFrame<'_>, version: u32) -> Result<ClientMessage, Malformed> {
    match version {
        1 => decode_client_v1(frame),
        _ => Err(Malformed("unknown protocol version")),
    }
}

fn decode_client_v1(frame: RawFrame<'_>) -> Result<ClientMessage, Malformed> {
    let mut r = Reader { rest: frame.body };
    let message = match frame.kind {
        kind::HELLO => ClientMessage::Hello {
            version: r.u32()?,
            take_over: r.bool()?,
        },
        kind::INFO => ClientMessage::Info,
        kind::READ => ClientMessage::Read(r.position()?),
        kind::SNAPSHOT => ClientMessage::Snapshot {
            position: r.position()?,
            blob: r.bytes(MAX_FRAME)?,
        },
        kind::INPUT => {
            let data = r.bytes(MAX_INPUT)?;
            if data.is_empty() {
                return Err(Malformed("empty input"));
            }
            ClientMessage::Input(data)
        }
        kind::RESIZE => ClientMessage::Resize(r.size()?),
        kind::SHUTDOWN => ClientMessage::Shutdown,
        _ => return Err(Malformed("unknown client message kind")),
    };
    r.end()?;
    Ok(message)
}

// ---- Host messages --------------------------------------------------------------

/// Encode a host message as one frame.
pub fn encode_host(message: &HostMessage) -> Vec<u8> {
    match message {
        HostMessage::Welcome { version } => Writer::new(kind::WELCOME).u32(*version).finish(),
        HostMessage::Busy => Writer::new(kind::BUSY).finish(),
        HostMessage::InfoReply(info) => {
            let mut w = Writer::new(kind::INFO_REPLY);
            w.u32(info.host_pid).u32(info.child_pid);
            match info.child_exit {
                None => w.u8(0).u8(0).i32(0),
                Some(exit) => w.u8(1).exit(exit),
            };
            w.size(info.size)
                .size(info.initial_size)
                .position(info.snapshot)
                .u64(info.tail_start)
                .u64(info.end)
                .u64(info.resizes)
                .u8(u8::from(info.gap))
                .u64(info.delivered_end)
                .bytes(&info.tag)
                .finish()
        }
        HostMessage::SnapshotRecord { position, blob } => Writer::new(kind::SNAPSHOT_RECORD)
            .position(*position)
            .bytes(blob)
            .finish(),
        HostMessage::Output { offset, data } => {
            Writer::new(kind::OUTPUT).u64(*offset).bytes(data).finish()
        }
        HostMessage::Resized {
            offset,
            index,
            size,
        } => Writer::new(kind::RESIZED)
            .u64(*offset)
            .u64(*index)
            .size(*size)
            .finish(),
        HostMessage::Gap {
            from,
            to,
            resizes,
            size,
        } => Writer::new(kind::GAP)
            .u64(*from)
            .u64(*to)
            .u64(*resizes)
            .size(*size)
            .finish(),
        HostMessage::Exited { offset, exit } => {
            Writer::new(kind::EXITED).u64(*offset).exit(*exit).finish()
        }
        HostMessage::SnapshotAccepted(position) => Writer::new(kind::SNAPSHOT_ACCEPTED)
            .position(*position)
            .finish(),
        HostMessage::SnapshotRejected { position, reason } => Writer::new(kind::SNAPSHOT_REJECTED)
            .position(*position)
            .u8(*reason as u8)
            .finish(),
        HostMessage::InputAck { written, discarded } => Writer::new(kind::INPUT_ACK)
            .u64(*written)
            .u64(*discarded)
            .finish(),
        HostMessage::Error { code, message } => {
            // Truncated at a character boundary, so the limit can never turn
            // a valid message into malformed text.
            let mut end = message.len().min(MAX_ERROR_MESSAGE);
            while !message.is_char_boundary(end) {
                end -= 1;
            }
            Writer::new(kind::ERROR)
                .u16(*code as u16)
                .bytes(&message.as_bytes()[..end])
                .finish()
        }
        HostMessage::Goodbye => Writer::new(kind::GOODBYE).finish(),
    }
}

/// Decode one host frame (as split by [`split_frame`]) of the negotiated
/// protocol `version`; a version this crate does not know is refused, as
/// for [`decode_client`]. `Welcome`, `Busy` and an `Error` that arrive
/// before a version is agreed are decoded as version 1, whose encodings of
/// them hold in every version (SPEC.md, "Connections and version
/// negotiation").
pub fn decode_host(frame: RawFrame<'_>, version: u32) -> Result<HostMessage, Malformed> {
    match version {
        1 => decode_host_v1(frame),
        _ => Err(Malformed("unknown protocol version")),
    }
}

fn decode_host_v1(frame: RawFrame<'_>) -> Result<HostMessage, Malformed> {
    let mut r = Reader { rest: frame.body };
    let message = match frame.kind {
        kind::WELCOME => HostMessage::Welcome { version: r.u32()? },
        kind::BUSY => HostMessage::Busy,
        kind::INFO_REPLY => {
            let host_pid = r.u32()?;
            let child_pid = r.u32()?;
            let child_exit = match r.u8()? {
                0 => {
                    if (r.u8()?, r.i32()?) != (0, 0) {
                        return Err(Malformed("exit status set while running"));
                    }
                    None
                }
                1 => Some(r.exit()?),
                _ => return Err(Malformed("unknown child state")),
            };
            HostMessage::InfoReply(Info {
                host_pid,
                child_pid,
                child_exit,
                size: r.size()?,
                initial_size: r.size()?,
                snapshot: r.position()?,
                tail_start: r.u64()?,
                end: r.u64()?,
                resizes: r.u64()?,
                gap: r.bool()?,
                delivered_end: r.u64()?,
                tag: r.bytes(MAX_TAG)?,
            })
        }
        kind::SNAPSHOT_RECORD => HostMessage::SnapshotRecord {
            position: r.position()?,
            blob: r.bytes(MAX_SNAPSHOT)?,
        },
        kind::OUTPUT => {
            let offset = r.u64()?;
            let data = r.bytes(MAX_OUTPUT_CHUNK)?;
            if data.is_empty() {
                return Err(Malformed("empty output"));
            }
            HostMessage::Output { offset, data }
        }
        kind::RESIZED => HostMessage::Resized {
            offset: r.u64()?,
            index: r.u64()?,
            size: r.size()?,
        },
        kind::GAP => HostMessage::Gap {
            from: r.u64()?,
            to: r.u64()?,
            resizes: r.u64()?,
            size: r.size()?,
        },
        kind::EXITED => HostMessage::Exited {
            offset: r.u64()?,
            exit: r.exit()?,
        },
        kind::SNAPSHOT_ACCEPTED => HostMessage::SnapshotAccepted(r.position()?),
        kind::SNAPSHOT_REJECTED => HostMessage::SnapshotRejected {
            position: r.position()?,
            reason: match r.u8()? {
                1 => SnapshotRejection::TooLarge,
                2 => SnapshotRejection::BeforeCurrent,
                3 => SnapshotRejection::OutsideRetained,
                4 => SnapshotRejection::InconsistentResizes,
                _ => return Err(Malformed("unknown rejection reason")),
            },
        },
        kind::INPUT_ACK => HostMessage::InputAck {
            written: r.u64()?,
            discarded: r.u64()?,
        },
        kind::ERROR => {
            let code = match r.u16()? {
                1 => ErrorCode::Malformed,
                2 => ErrorCode::NotAllowed,
                3 => ErrorCode::UnsupportedVersion,
                _ => return Err(Malformed("unknown error code")),
            };
            let message = String::from_utf8(r.bytes(MAX_ERROR_MESSAGE)?)
                .map_err(|_| Malformed("error message is not UTF-8"))?;
            HostMessage::Error { code, message }
        }
        kind::GOODBYE => HostMessage::Goodbye,
        _ => return Err(Malformed("unknown host message kind")),
    };
    r.end()?;
    Ok(message)
}
