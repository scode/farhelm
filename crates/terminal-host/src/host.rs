//! The host proper: one `poll` loop over the listening socket, connections
//! waiting for their `Hello`, the client, the pty master and a signal pipe
//! (SPEC.md, "Protocol", "Input", "Ending").
//!
//! Nothing here ever blocks: every descriptor is non-blocking, output is
//! read from the pty whatever the client does (the journal is the buffer),
//! and what the client cannot take yet waits in the outgoing queue, refilled
//! from its journal cursor only as the socket drains. That is what keeps a
//! slow or stuck client from slowing the program, and a program that stops
//! reading its terminal from wedging control messages.
//!
//! Everything the client sends goes into one buffer and is acted on in
//! order, frame by frame ([`Host::act_on_buffered`]). Two bounds hold frames
//! back in that buffer instead: an `Input` that would take pending input
//! past `MAX_PENDING_INPUT` (it, and everything after it, waits until it
//! fits), and more than `OUTGOING_HIGH_WATER` bytes of replies and stream
//! records the client has not taken (nothing is acted on until it takes
//! some). While either holds, the socket is still read, but only until the
//! buffer holds one maximum frame. When the connection ends or fails, what
//! it sent is acted on regardless of either bound before it is dropped
//! ([`Host::close_client`]), so a `Shutdown` sent before a close the host
//! notices is never lost; `Input` among it is dropped as it is decoded.
//!
//! Input is acknowledged per batch, not per `Input`: whenever one or more
//! `Input`s are resolved (written whole, or discarded), one `InputAck`
//! carries the connection's running totals of written and discarded bytes,
//! and an acknowledgement still waiting unstarted at the back of the
//! outgoing queue is updated in place rather than followed by another. That
//! keeps the number of queued acknowledgements at most two more than the
//! other frames between them (one partly written at the front can have a
//! fresh one right behind it), however many tiny `Input`s a burst resolves.
//!
//! A connection ends with at most one last frame of its own: `Goodbye` after
//! `Shutdown`, `Error` after a refusal. Both drop what is queued and not
//! started, except a `Welcome` not written yet (which must come first), and
//! both are flushed for at most `GOODBYE_FLUSH` ([`Client::flush_deadline`]).
//!
//! [`Host::new`] without a signal pipe, on given descriptors, is the
//! in-process test seam: the unit tests run the same loop on a pty and child
//! they started themselves, which is where coverage of this file comes from.

use crate::journal::{Cursor, Journal};
use crate::protocol::{
    ChildExit, ClientMessage, EXIT_DRAIN_BYTES, EXIT_DRAIN_TIME, ErrorCode, GOODBYE_FLUSH,
    HELLO_TIMEOUT, HostMessage, INPUT_COST, Info, MAX_BEFORE_HELLO, MAX_FRAME,
    MAX_PENDING_CONNECTIONS, MAX_PENDING_INPUT, Malformed, OUTGOING_HIGH_WATER, PROTOCOL_VERSION,
    Size, decode_client, encode_host, split_frame,
};
use std::collections::VecDeque;
use std::io;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How much pty output one loop iteration reads before serving the rest.
const READ_BUDGET: usize = 1024 * 1024;
/// How much of what the client sent the host buffers: one maximum frame,
/// its length field included. A client whose frames are held back can fill
/// it, and is then not read until they can be acted on; below it, the
/// socket is always read, so a close is noticed.
const RECEIVE_LIMIT: usize = MAX_FRAME + 4;
/// How long the host stops polling its listener after `accept` failed for
/// want of descriptors. The connection stays queued and the listener stays
/// readable, so polling it at once again would spin a CPU until a
/// descriptor frees up.
const ACCEPT_PAUSE: Duration = Duration::from_millis(100);

/// What a host is given to serve.
///
/// The initial snapshot is not part of it: [`Host::new`] takes that by value
/// and moves it into the journal, so a host spawned with a large one keeps a
/// single copy for its whole life rather than one here as well.
pub(crate) struct Setup {
    pub(crate) listener: OwnedFd,
    pub(crate) socket: PathBuf,
    /// The file `listen` bound at `socket`: what the host removes when it
    /// ends, and only if that file is still there.
    pub(crate) socket_file: SocketFile,
    pub(crate) master: OwnedFd,
    pub(crate) child: u32,
    pub(crate) size: Size,
    pub(crate) journal_cap: u64,
    pub(crate) tag: Vec<u8>,
}

/// Why `run` returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ended {
    Shutdown,
    Terminated,
}

/// One outgoing frame and how much of it was written.
struct Outgoing {
    bytes: Vec<u8>,
    written: usize,
    /// For output frames: the offset after their data, which counts as
    /// delivered once the frame is fully written.
    output_end: Option<u64>,
    /// Whether this is an `InputAck`, which a later acknowledgement may
    /// update in place while it is the last frame queued and unstarted.
    ack: bool,
    /// Whether this is the connection's `Welcome`. Until it is written,
    /// nothing may drop it: every other frame a client gets assumes the
    /// version it names, a `Goodbye` or `Error` included.
    welcome: bool,
}

/// A connection that has not sent its `Hello` yet.
struct Pending {
    fd: OwnedFd,
    buf: Vec<u8>,
    deadline: Instant,
}

/// One queued `Input` and how much of it reached the pty so far.
struct InputSegment {
    data: Vec<u8>,
    written: usize,
}

/// The connected client.
struct Client {
    fd: OwnedFd,
    version: u32,
    /// What the client sent and the host has not acted on yet: a partial
    /// frame, or the frames a bound holds back. Never more than
    /// [`RECEIVE_LIMIT`] bytes, but for the last read of a closed
    /// connection ([`Host::close_client`]).
    buf: Vec<u8>,
    outgoing: VecDeque<Outgoing>,
    /// The bytes of `outgoing` not written yet: the reply budget that
    /// [`OUTGOING_HIGH_WATER`] bounds, kept as a running total because it
    /// is checked before every frame.
    unsent: usize,
    /// The stream, from the connection's one `Read` on.
    cursor: Option<Cursor>,
    input: VecDeque<InputSegment>,
    /// What `input` counts against [`MAX_PENDING_INPUT`]: each segment's
    /// unwritten bytes plus [`INPUT_COST`].
    input_cost: usize,
    /// Bytes of the resolved `Input`s that reached the pty, and that were
    /// discarded: the running totals each `InputAck` carries.
    written: u64,
    discarded: u64,
    /// Set by `Shutdown` or a refusal: nothing more is acted on, and the
    /// client is dropped once what is queued for it is written, or at
    /// `flush_deadline`.
    closing: bool,
    /// When a closing client is dropped whether or not its last frame
    /// (`Goodbye` or `Error`) is out: `GOODBYE_FLUSH` after it was queued.
    /// Without it, a client that stops reading would stay the client for
    /// good, and every newcomer without take-over would get `Busy`.
    flush_deadline: Option<Instant>,
    /// Set once the connection ended or failed, while what it sent is acted
    /// on for the last time: replies and input go nowhere.
    gone: bool,
}

pub(crate) struct Host {
    setup: Setup,
    journal: Journal,
    signals: Option<OwnedFd>,
    pending: VecDeque<Pending>,
    client: Option<Client>,
    pty_open: bool,
    child_exit: Option<ChildExit>,
    /// Set from the child's reaping until its exit is journaled.
    draining: Option<(ChildExit, Instant, usize)>,
    delivered_end: u64,
    /// Set when `accept` ran out of descriptors: the listener is not polled
    /// until then ([`ACCEPT_PAUSE`]).
    accept_paused_until: Option<Instant>,
    /// Set when told to stop; `run` returns it.
    ended: Option<Ended>,
}

/// The write end of the signal pipe, for the handlers.
static SIGNAL_PIPE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
/// Set by the SIGTERM handler.
static TERMINATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The handler for SIGTERM and SIGCHLD: note a SIGTERM, then wake the loop
/// through the signal pipe. The pipe is what closes the window between the
/// loop's last look at its state and its entry into `poll`, where a signal
/// that only interrupted a system call would go unnoticed until the next
/// event.
///
/// It leaves `errno` as it found it: the handler can run between any system
/// call of the loop and its caller's look at `errno`, and its own `write`
/// sets `errno` when it fails (on a full pipe, which already wakes the
/// loop, so the failure itself loses nothing).
extern "C" fn on_signal(signal: libc::c_int) {
    // SAFETY: the calling thread's errno location, valid for its lifetime.
    let errno = unsafe { crate::spawn::errno_location() };
    // SAFETY: as above.
    let saved = unsafe { *errno };
    if signal == libc::SIGTERM {
        TERMINATED.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    let fd = SIGNAL_PIPE.load(std::sync::atomic::Ordering::SeqCst);
    if fd >= 0 {
        // SAFETY: write is async-signal-safe.
        unsafe { libc::write(fd, [0u8].as_ptr().cast(), 1) };
    }
    // SAFETY: as above.
    unsafe { *errno = saved };
}

/// Whether a SIGTERM has arrived since the handlers were installed. The
/// start checks it before it starts the child and before it reports
/// readiness, so that a SIGTERM during the start fails the start rather
/// than starting a child for a host that is about to end (SPEC.md, "Start
/// and readiness").
pub(crate) fn terminated() -> bool {
    TERMINATED.load(std::sync::atomic::Ordering::SeqCst)
}

impl Host {
    /// A host serving `setup`. `signals` is the read end of the signal pipe
    /// from [`install_signal_handlers`], which a real host installs before
    /// it starts its child (so the child's exit can never come before the
    /// handler); without it, the loop instead polls for the child's exit,
    /// for in-process tests. `initial_snapshot` moves into the journal.
    pub(crate) fn new(
        setup: Setup,
        initial_snapshot: Option<Vec<u8>>,
        signals: Option<OwnedFd>,
    ) -> Self {
        let journal = Journal::new(setup.size, setup.journal_cap, initial_snapshot);
        Self {
            setup,
            journal,
            signals,
            pending: VecDeque::new(),
            client: None,
            pty_open: true,
            child_exit: None,
            draining: None,
            delivered_end: 0,
            accept_paused_until: None,
            ended: None,
        }
    }

    /// End a host whose readiness could not be reported, before it served
    /// anything: kill and reap its child as a failed start does, and remove
    /// its socket file, if it is still the one this host bound (SPEC.md,
    /// "Start and readiness").
    pub(crate) fn abandon(self) {
        crate::spawn::kill_child_group(self.setup.child);
        remove_socket(&self.setup.socket, self.setup.socket_file);
    }

    /// Serve until `Shutdown` or SIGTERM, then close the pty and remove the
    /// socket file, if it is still the one this host bound.
    pub(crate) fn run(mut self) -> Ended {
        loop {
            if let Some(ended) = self.step() {
                remove_socket(&self.setup.socket, self.setup.socket_file);
                return ended;
            }
        }
    }

    /// One iteration: poll, then serve whatever is ready. Returns why the
    /// host ended, once it has.
    pub(crate) fn step(&mut self) -> Option<Ended> {
        if terminated() && self.signals.is_some() {
            return Some(Ended::Terminated);
        }
        // A closing client whose last frame did not go out in time is
        // dropped; after `Shutdown`, that is also what ends the host.
        if self.client.as_ref().is_some_and(|client| {
            client
                .flush_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
        }) {
            self.drop_client();
        }
        if let Some(ended) = self.ended
            && self
                .client
                .as_ref()
                .is_none_or(|client| client.outgoing.is_empty())
        {
            return Some(ended);
        }
        self.refill_outgoing();
        let mut fds = Vec::new();
        if self
            .accept_paused_until
            .is_some_and(|until| Instant::now() >= until)
        {
            self.accept_paused_until = None;
        }
        // After `Shutdown` the host takes no new connection: it only flushes
        // `Goodbye` to the client that asked. Nor does it while accepting is
        // paused for want of descriptors.
        let listener = (self.ended.is_none() && self.accept_paused_until.is_none())
            .then(|| push(&mut fds, self.setup.listener.as_raw_fd(), libc::POLLIN));
        let signals = self
            .signals
            .as_ref()
            .map(|fd| push(&mut fds, fd.as_raw_fd(), libc::POLLIN));
        let mut pty_events = 0;
        if self.pty_open {
            pty_events |= libc::POLLIN;
            if self
                .client
                .as_ref()
                .is_some_and(|client| !client.input.is_empty())
            {
                pty_events |= libc::POLLOUT;
            }
        }
        let pty =
            (pty_events != 0).then(|| push(&mut fds, self.setup.master.as_raw_fd(), pty_events));
        // A client with nothing to wait for is left out of the poll set
        // rather than polled with no events: what poll reports for such an
        // entry differs by platform (Linux still reports a hangup, macOS
        // reports nothing), and the host must behave the same on both.
        let client = self.client.as_ref().and_then(|client| {
            let events = client_events(client);
            (events != 0).then(|| push(&mut fds, client.fd.as_raw_fd(), events))
        });
        let pending: Vec<usize> = self
            .pending
            .iter()
            .map(|pending| push(&mut fds, pending.fd.as_raw_fd(), libc::POLLIN))
            .collect();
        let timeout = self.poll_timeout();
        // SAFETY: fds is a valid array of pollfd of the given length.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
        if ready == -1 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            // Nothing sensible to do but try again; poll on valid fds does
            // not fail persistently.
            return None;
        }
        let revents = |index: usize| fds[index].revents;
        if let Some(index) = signals
            && revents(index) != 0
        {
            self.drain_signal_pipe();
        }
        self.reap_child();
        if let Some(index) = pty {
            if revents(index) & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                self.read_pty();
            }
            if revents(index) & libc::POLLOUT != 0 {
                self.write_input();
            }
        }
        self.finish_drain();
        if let Some(index) = client {
            let events = revents(index);
            if events & libc::POLLOUT != 0 {
                self.write_client();
            }
            // A hangup or an error means the peer is gone (both directions
            // shut): read what it sent before, act on it, and drop it. A
            // plain end of stream shows up as a read of 0 instead.
            if events & (libc::POLLHUP | libc::POLLERR) != 0 {
                self.close_client();
            } else if events & libc::POLLIN != 0 {
                self.read_client();
            }
        }
        for (position, index) in pending.into_iter().enumerate().rev() {
            // A `Shutdown` acted on just now (from a connection promoted in
            // this loop) closed every waiting connection.
            if self.ended.is_some() {
                break;
            }
            if revents(index) != 0 {
                self.read_pending(position);
            }
        }
        self.expire_pending();
        if let Some(index) = listener
            && self.ended.is_none()
            && revents(index) & libc::POLLIN != 0
        {
            self.accept();
        }
        // Input that arrived this iteration may be writable at once.
        if self
            .client
            .as_ref()
            .is_some_and(|client| !client.input.is_empty())
        {
            self.write_input();
        }
        // Frames that a bound held back in the client's buffer are acted on
        // once it no longer holds. They were read from the socket already,
        // so no socket event will ever announce them again; waiting for one
        // would leave a client that sent them waiting forever. This comes
        // after the last write to the pty and to the client in the iteration
        // (either may have released a bound, and the end of output drops all
        // pending input), so no held frame that could be acted on is left
        // for the next poll; whatever it queues brings its own wakeup (the
        // pty becoming writable for input, the socket for replies). Does
        // nothing while a bound still holds, the client is closing, or no
        // whole frame is buffered.
        self.act_on_buffered();
        // A read, a resize or new output this iteration may have given the
        // client's cursor something to send; queue it now, so the next poll
        // waits for the socket to take it rather than for some other event.
        self.refill_outgoing();
        None
    }

    fn poll_timeout(&self) -> libc::c_int {
        let now = Instant::now();
        let mut deadlines: Vec<Instant> = self
            .pending
            .iter()
            .map(|pending| pending.deadline)
            .collect();
        if let Some((_, deadline, _)) = self.draining {
            deadlines.push(deadline);
        }
        deadlines.extend(
            self.client
                .as_ref()
                .and_then(|client| client.flush_deadline),
        );
        deadlines.extend(self.accept_paused_until);
        // Rounded up: a deadline less than a millisecond away would
        // otherwise poll with 0, and return at once, until it has passed.
        let mut timeout = deadlines.into_iter().min().map(|deadline| {
            deadline
                .saturating_duration_since(now)
                .as_micros()
                .div_ceil(1000)
                .min(60_000) as libc::c_int
        });
        // Without a SIGCHLD wakeup, look for the child's exit now and then.
        if self.signals.is_none() && self.child_exit.is_none() {
            timeout = Some(timeout.map_or(20, |t| t.min(20)));
        }
        timeout.unwrap_or(-1)
    }

    fn drain_signal_pipe(&mut self) {
        let Some(fd) = &self.signals else { return };
        let mut buf = [0u8; 64];
        // SAFETY: reading into a valid buffer from a non-blocking pipe.
        while unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) } > 0 {}
    }

    /// Reap the child if it exited, and start draining its last output.
    fn reap_child(&mut self) {
        if self.child_exit.is_some() {
            return;
        }
        let mut status = 0;
        // SAFETY: waitpid on this host's own child, non-blocking.
        let pid =
            unsafe { libc::waitpid(self.setup.child as libc::pid_t, &mut status, libc::WNOHANG) };
        if pid != self.setup.child as libc::pid_t {
            return;
        }
        let exit = if libc::WIFSIGNALED(status) {
            ChildExit::Signal(libc::WTERMSIG(status))
        } else {
            ChildExit::Code(libc::WEXITSTATUS(status))
        };
        self.child_exit = Some(exit);
        self.draining = Some((exit, Instant::now() + EXIT_DRAIN_TIME, 0));
        // Read what is already there now, so a child that wrote and exited
        // has its output before its exit.
        self.read_pty();
    }

    /// Journal the exit once the output available at reaping is read, or
    /// the drain's bounds are reached.
    fn finish_drain(&mut self) {
        if let Some((exit, deadline, bytes)) = self.draining
            && (!self.pty_has_more() || Instant::now() >= deadline || bytes >= EXIT_DRAIN_BYTES)
        {
            self.journal.push_exit(exit);
            self.draining = None;
        }
    }

    /// Whether the pty has output ready right now.
    fn pty_has_more(&self) -> bool {
        if !self.pty_open {
            return false;
        }
        let mut fd = libc::pollfd {
            fd: self.setup.master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd, no wait.
        unsafe { libc::poll(&mut fd, 1, 0) == 1 && fd.revents & libc::POLLIN != 0 }
    }

    /// Read available output into the journal, up to the per-iteration
    /// budget. End of output (EIO or 0 on the master) stops polling the pty.
    fn read_pty(&mut self) {
        if !self.pty_open {
            return;
        }
        let mut buf = vec![0u8; 64 * 1024];
        let mut total = 0;
        while total < READ_BUDGET {
            // SAFETY: reading into a valid buffer from a non-blocking fd.
            let n = unsafe {
                libc::read(
                    self.setup.master.as_raw_fd(),
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                )
            };
            if n > 0 {
                let n = n as usize;
                self.journal.push_output(&buf[..n]);
                total += n;
                if let Some((_, _, bytes)) = &mut self.draining {
                    *bytes += n;
                }
                continue;
            }
            if n == 0 {
                self.end_of_output();
                break;
            }
            match io::Error::last_os_error().raw_os_error() {
                Some(libc::EAGAIN) => break,
                Some(libc::EINTR) => continue,
                // EIO: no process holds the slave any more.
                _ => {
                    self.end_of_output();
                    break;
                }
            }
        }
    }

    /// The pty has no reader and no writer any more: stop polling it, and
    /// discard the input that can no longer go anywhere.
    fn end_of_output(&mut self) {
        self.pty_open = false;
        self.discard_input();
    }

    /// Drop the client's pending input, and acknowledge it as discarded: it
    /// went where typing into a finished terminal goes (SPEC.md, "Input").
    /// Bytes of a partly written `Input` that did reach the pty count as
    /// written. The pending cost returns to zero, which is what lets held
    /// `Input`s in.
    fn discard_input(&mut self) {
        let Some(client) = &mut self.client else {
            return;
        };
        if client.input.is_empty() {
            return;
        }
        while let Some(segment) = client.input.pop_front() {
            resolve(client, segment);
        }
        acknowledge(client);
    }

    /// Write queued input to the pty, in order, as far as the pty takes it,
    /// and acknowledge the `Input`s this wrote in full, in one `InputAck`.
    ///
    /// Each `Input` starts with one `write` of everything it holds, so one
    /// of at most [`crate::protocol::ATOMIC_INPUT`] bytes is offered to the
    /// pty whole and the host never splits a key's bytes itself (SPEC.md,
    /// "Input"). The pty may still take only part of it (macOS's does when
    /// its input queue is nearly full), and the rest then follows in later
    /// writes, as for a longer one. There is no waiting for room first: no
    /// pty interface says how much room there is, and holding a small
    /// `Input` back would hold every `Input` after it too.
    fn write_input(&mut self) {
        let master = self.setup.master.as_raw_fd();
        if !self.pty_open {
            self.discard_input();
            return;
        }
        let Some(client) = &mut self.client else {
            return;
        };
        let mut resolved = false;
        while let Some(segment) = client.input.front_mut() {
            let rest = &segment.data[segment.written..];
            // SAFETY: writing from a valid buffer to a non-blocking fd.
            let n = unsafe { libc::write(master, rest.as_ptr().cast(), rest.len()) };
            if n < 0 {
                match io::Error::last_os_error().raw_os_error() {
                    Some(libc::EAGAIN) | Some(libc::EINTR) => break,
                    // The pty refuses input for good (on Linux, EIO once
                    // nothing holds the slave). The input is discarded, but
                    // whether output has ended is left to `read_pty`: the
                    // master can still hold output the program wrote before
                    // it let go, and only a read finds its end. (Discarding
                    // acknowledges what this call wrote, too.)
                    _ => {
                        self.discard_input();
                        return;
                    }
                }
            }
            let n = n as usize;
            segment.written += n;
            client.input_cost -= n;
            if segment.written < segment.data.len() {
                break;
            }
            let segment = client.input.pop_front().expect("the front was there");
            resolve(client, segment);
            resolved = true;
        }
        if resolved {
            acknowledge(client);
        }
    }

    /// Move records from the client's cursor into its outgoing queue while
    /// less than [`OUTGOING_HIGH_WATER`] waits unsent, so the queue holds
    /// at most that plus one frame however far the client lags.
    fn refill_outgoing(&mut self) {
        let Some(client) = &mut self.client else {
            return;
        };
        let Some(cursor) = &mut client.cursor else {
            return;
        };
        while client.unsent < OUTGOING_HIGH_WATER {
            let Some(message) = self.journal.next(cursor) else {
                break;
            };
            let output_end = match &message {
                HostMessage::Output { offset, data } => Some(offset + data.len() as u64),
                _ => None,
            };
            let bytes = encode_host(&message);
            client.unsent += bytes.len();
            client.outgoing.push_back(Outgoing {
                bytes,
                written: 0,
                output_end,
                ack: false,
                welcome: false,
            });
        }
    }

    /// Write queued frames as far as the socket takes them. A failed write
    /// closes the client the way an end of stream does: what it sent before
    /// is still acted on.
    fn write_client(&mut self) {
        let Some(client) = &mut self.client else {
            return;
        };
        while let Some(out) = client.outgoing.front_mut() {
            let rest = &out.bytes[out.written..];
            match send(client.fd.as_raw_fd(), rest) {
                Ok(n) => {
                    out.written += n;
                    client.unsent -= n;
                    if out.written < out.bytes.len() {
                        return;
                    }
                    if let Some(end) = out.output_end {
                        self.delivered_end = self.delivered_end.max(end);
                    }
                    client.outgoing.pop_front();
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    return;
                }
                Err(_) => {
                    self.close_client();
                    return;
                }
            }
        }
        if self
            .client
            .as_ref()
            .is_some_and(|client| client.closing && client.outgoing.is_empty())
        {
            self.drop_client();
        }
    }

    /// Read what the client sent, up to [`RECEIVE_LIMIT`] buffered, and act
    /// on what the bounds let through; at end of stream, close it.
    fn read_client(&mut self) {
        let Some(client) = &mut self.client else {
            return;
        };
        if receive(client.fd.as_raw_fd(), &mut client.buf, RECEIVE_LIMIT) == Received::Ended {
            self.close_client();
            return;
        }
        self.act_on_buffered();
    }

    /// The client's connection ended or failed: act on every whole frame it
    /// sent, then drop it.
    ///
    /// Frames already buffered and what the socket still holds are acted on
    /// in order, ignoring both bounds that would otherwise hold them back.
    /// Input already pending is discarded first, and `Input` among those
    /// frames is dropped as it is decoded, never queued (the connection is
    /// going away; SPEC.md, "Input"), so pending input stays empty however
    /// much the closed client sent; replies are not queued (nobody will read
    /// them). A `Shutdown` among them therefore always ends the host,
    /// wherever it sat behind held input. One read of at most
    /// [`RECEIVE_LIMIT`] more bytes takes in what the socket holds: a closed
    /// peer cannot add to that, and it is far less, but one whose write side
    /// is still open (it shut down only its reading) could keep a loop that
    /// read to the end here for as long as it writes.
    fn close_client(&mut self) {
        let Some(client) = &mut self.client else {
            return;
        };
        client.gone = true;
        client.input.clear();
        client.input_cost = 0;
        client.outgoing.clear();
        client.unsent = 0;
        // What is buffered first: that leaves at most a partial frame, so the
        // one read after it holds at most two maximum frames in all.
        self.act_on_buffered();
        if let Some(client) = &mut self.client
            && !client.closing
        {
            let limit = client.buf.len() + RECEIVE_LIMIT;
            receive(client.fd.as_raw_fd(), &mut client.buf, limit);
            self.act_on_buffered();
        }
        self.drop_client();
    }

    /// Act on one message from the client.
    fn handle(&mut self, message: ClientMessage) {
        let Some(client) = &mut self.client else {
            return;
        };
        match message {
            ClientMessage::Hello { .. } => {
                self.refuse_client(ErrorCode::NotAllowed, "Hello after Welcome");
            }
            ClientMessage::Info => {
                let info = HostMessage::InfoReply(Info {
                    host_pid: std::process::id(),
                    child_pid: self.setup.child,
                    // The reaped status at once, even while the exit record
                    // still waits for the last output: a client may treat
                    // the child pid as its process tree's root only while
                    // the child runs, and a reaped pid can be reused.
                    child_exit: self.child_exit,
                    size: self.journal.size(),
                    initial_size: self.journal.initial_size(),
                    snapshot: self.journal.snapshot_position(),
                    tail_start: self.journal.tail_start(),
                    end: self.journal.end(),
                    resizes: self.journal.resize_count(),
                    gap: self.journal.has_gap(),
                    delivered_end: self.delivered_end,
                    tag: self.setup.tag.clone(),
                });
                queue(client, &info);
            }
            ClientMessage::Read(_) if client.cursor.is_some() => {
                // One stream per connection: a client re-reads by
                // reconnecting, so nothing has to mark where a second stream
                // would start among records already in the socket.
                self.refuse_client(ErrorCode::NotAllowed, "a second Read");
            }
            ClientMessage::Read(position) => {
                client.cursor = Some(self.journal.read(position));
            }
            ClientMessage::Snapshot { position, blob } => {
                let reply = match self.journal.accept_snapshot(position, blob) {
                    Ok(()) => HostMessage::SnapshotAccepted(position),
                    Err(reason) => HostMessage::SnapshotRejected { position, reason },
                };
                queue(client, &reply);
            }
            // A closed connection's input could only be dropped later, so it
            // is dropped now: queueing it would hold up to a buffer's worth
            // of segments past the pending-input bound, which a closed
            // connection's frames bypass.
            ClientMessage::Input(_) if client.gone => {}
            ClientMessage::Input(data) => {
                client.input_cost += data.len() + INPUT_COST;
                client.input.push_back(InputSegment { data, written: 0 });
            }
            ClientMessage::Resize(size) => {
                let winsize = libc::winsize {
                    ws_row: size.rows,
                    ws_col: size.cols,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };
                // SAFETY: TIOCSWINSZ on the pty master with a valid winsize.
                unsafe {
                    libc::ioctl(
                        self.setup.master.as_raw_fd(),
                        libc::TIOCSWINSZ as _,
                        &winsize,
                    )
                };
                self.journal.push_resize(size);
            }
            ClientMessage::Shutdown => {
                // Pending input is discarded unacknowledged, and `Goodbye`
                // goes out next, after at most the frame being written
                // (SPEC.md, "Ending").
                end_connection(client, &HostMessage::Goodbye);
                // From here on only `Goodbye` is flushed, for at most
                // GOODBYE_FLUSH: no connection waiting for its Hello may
                // still become the client.
                self.pending.clear();
                self.ended = Some(Ended::Shutdown);
            }
        }
    }

    /// Send the client an error and close it once the error is out, or at
    /// its flush deadline (SPEC.md, "Host messages").
    ///
    /// Ends the connection as `Shutdown` does, but for the host: a frame
    /// already partly written is finished first, since cutting it would
    /// break the framing of the `Error` after it, and so is a `Welcome` not
    /// written yet; everything else not yet started is dropped, and so is
    /// the pending input, unacknowledged, so that no `InputAck` can follow
    /// the `Error`.
    fn refuse_client(&mut self, code: ErrorCode, message: &str) {
        let Some(client) = &mut self.client else {
            return;
        };
        end_connection(
            client,
            &HostMessage::Error {
                code,
                message: message.into(),
            },
        );
    }

    fn drop_client(&mut self) {
        self.client = None;
    }

    /// Accept the connections waiting on the listener, at most
    /// `MAX_PENDING_CONNECTIONS` per iteration. Peers that connect as fast
    /// as the host accepts could otherwise keep this loop from ever
    /// returning to the pty, the client or a SIGTERM; whatever is left stays
    /// in the listen queue, and the listener polls readable again. Out of
    /// descriptors or memory, the listener is left alone for
    /// [`ACCEPT_PAUSE`] instead.
    fn accept(&mut self) {
        for _ in 0..MAX_PENDING_CONNECTIONS {
            let fd = match accept_connection(self.setup.listener.as_raw_fd()) {
                Ok(fd) => fd,
                Err(error) => {
                    // Out of descriptors or of kernel memory for the
                    // connection: either way it stays queued.
                    if matches!(
                        error.raw_os_error(),
                        Some(libc::EMFILE | libc::ENFILE | libc::ENOBUFS | libc::ENOMEM)
                    ) {
                        self.accept_paused_until = Some(Instant::now() + ACCEPT_PAUSE);
                    }
                    return;
                }
            };
            if !same_user(fd.as_raw_fd()) {
                continue;
            }
            if self.pending.len() >= MAX_PENDING_CONNECTIONS {
                self.pending.pop_front();
            }
            self.pending.push_back(Pending {
                fd,
                buf: Vec::new(),
                deadline: Instant::now() + HELLO_TIMEOUT,
            });
        }
    }

    fn expire_pending(&mut self) {
        let now = Instant::now();
        self.pending.retain(|pending| pending.deadline > now);
    }

    /// Read from a connection waiting for its `Hello`, and promote it once
    /// one arrives.
    ///
    /// At most [`MAX_BEFORE_HELLO`] bytes are read before the `Hello`: a
    /// `Hello` is 10 bytes in every version, so a connection that sent that
    /// much without a whole frame sent something else first and is closed,
    /// and none of the waiting connections costs more than that. What
    /// follows a `Hello` in the bytes read with it becomes the start of the
    /// client's buffer; the rest stays in the socket until the host reads it
    /// as the client's.
    ///
    /// A takeover closes the old client without a word: what it was sent in
    /// part stays cut short, the rest of its queue and its pending input are
    /// dropped, and so is whatever it sent that was not acted on (SPEC.md,
    /// "Connections and version negotiation"). Its end of stream is the
    /// whole notice; a client must take one as possible displacement anyway,
    /// since a stuck connection is the likeliest to be mid-frame.
    fn read_pending(&mut self, index: usize) {
        let pending = &mut self.pending[index];
        let outcome = receive(pending.fd.as_raw_fd(), &mut pending.buf, MAX_BEFORE_HELLO);
        let frame = match split_frame(&pending.buf, MAX_FRAME) {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                // Closed before a whole frame, or no frame where a Hello
                // would have fit.
                if outcome != Received::Drained {
                    self.pending.remove(index);
                }
                return;
            }
            Err(_) => {
                self.pending.remove(index);
                return;
            }
        };
        let consumed = frame.consumed;
        let message = decode_client(frame, PROTOCOL_VERSION);
        let pending = self.pending.remove(index).expect("index is valid");
        let (version, take_over) = match message {
            Ok(ClientMessage::Hello { version, take_over }) => (version, take_over),
            Ok(_) | Err(Malformed(_)) => return,
        };
        if version == 0 {
            let _ = send(
                pending.fd.as_raw_fd(),
                &encode_host(&HostMessage::Error {
                    code: ErrorCode::UnsupportedVersion,
                    message: "version 0".into(),
                }),
            );
            return;
        }
        if self.client.is_some() && !take_over {
            let _ = send(pending.fd.as_raw_fd(), &encode_host(&HostMessage::Busy));
            return;
        }
        self.drop_client();
        let version = version.min(PROTOCOL_VERSION);
        let mut client = Client {
            fd: pending.fd,
            version,
            buf: pending.buf[consumed..].to_vec(),
            outgoing: VecDeque::new(),
            unsent: 0,
            cursor: None,
            input: VecDeque::new(),
            input_cost: 0,
            written: 0,
            discarded: 0,
            closing: false,
            flush_deadline: None,
            gone: false,
        };
        queue(&mut client, &HostMessage::Welcome { version });
        self.client = Some(client);
        // Anything that followed the Hello in the same read, and the close
        // if one came with it.
        if outcome == Received::Ended {
            self.close_client();
        } else {
            self.act_on_buffered();
        }
    }

    /// Act on the whole frames buffered for the client, in order, without
    /// reading, until one is held back or none is left; then drop what was
    /// acted on from the buffer, once for the whole batch.
    ///
    /// A frame is held back, with everything after it, while more than
    /// [`OUTGOING_HIGH_WATER`] bytes wait unsent for the client (its replies
    /// would otherwise pile up without bound behind a client that does not
    /// read), and an `Input` is held while it does not fit under
    /// `MAX_PENDING_INPUT` with what is pending, each `Input` counting its
    /// data plus `INPUT_COST`. Neither holds once the
    /// connection is gone ([`Host::close_client`]). Nothing is acted on
    /// after `Shutdown` or a refusal.
    fn act_on_buffered(&mut self) {
        let mut at = 0;
        while let Some(client) = &mut self.client {
            if client.closing || (!client.gone && client.unsent > OUTGOING_HIGH_WATER) {
                break;
            }
            let frame = match split_frame(&client.buf[at..], MAX_FRAME) {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(error) => {
                    self.refuse_client(ErrorCode::Malformed, &error.to_string());
                    break;
                }
            };
            let consumed = frame.consumed;
            match decode_client(frame, client.version) {
                Ok(ClientMessage::Input(data))
                    if !client.gone
                        && client.input_cost + data.len() + INPUT_COST > MAX_PENDING_INPUT =>
                {
                    break;
                }
                Ok(message) => {
                    at += consumed;
                    self.handle(message);
                }
                // Refused: nothing in the buffer is looked at again, so the
                // malformed frame may as well stay in it.
                Err(error) => {
                    self.refuse_client(ErrorCode::Malformed, &error.to_string());
                    break;
                }
            }
        }
        if let Some(client) = &mut self.client {
            client.buf.drain(..at);
        }
    }
}

/// The events to poll the client for: its socket while its buffer has room
/// (also while frames are held, so that a close is noticed), and writable
/// while anything is queued for it. Both can be off at once, for a client
/// whose held frames fill its buffer while nothing is queued for it: one
/// that sent far more input than it may have unacknowledged. Such a client
/// is not polled at all until the program takes input, so its close goes
/// unnoticed until then (SPEC.md, "Input").
fn client_events(client: &Client) -> libc::c_short {
    let mut events = 0;
    if !client.closing && client.buf.len() < RECEIVE_LIMIT {
        events |= libc::POLLIN;
    }
    if !client.outgoing.is_empty() {
        events |= libc::POLLOUT;
    }
    events
}

/// Queue a reply for the client, counted against its reply budget. A client
/// whose connection is gone gets nothing queued.
fn queue(client: &mut Client, message: &HostMessage) {
    if client.gone {
        return;
    }
    let bytes = encode_host(message);
    client.unsent += bytes.len();
    client.outgoing.push_back(Outgoing {
        bytes,
        written: 0,
        output_end: None,
        ack: false,
        welcome: matches!(message, HostMessage::Welcome { .. }),
    });
}

/// Account for one `Input` leaving the pending queue: what reached the pty
/// counts as written, the rest as discarded, and its pending cost is freed.
fn resolve(client: &mut Client, segment: InputSegment) {
    let unwritten = segment.data.len() - segment.written;
    client.input_cost -= unwritten + INPUT_COST;
    client.written += segment.written as u64;
    client.discarded += unwritten as u64;
}

/// Queue an `InputAck` with the connection's running totals, or bring the
/// one already last in the queue and not started up to date instead.
///
/// Coalescing loses the client nothing, since each acknowledgement carries
/// the totals so far, and it keeps acknowledgements from piling up: a burst
/// that resolves a million tiny `Input`s at once (the terminal losing its
/// reader with that much pending) queues one. A started acknowledgement is
/// never touched (that would corrupt the frame on the wire), so the one
/// case of two in a row is one partly written at the front with a fresh one
/// behind it; acknowledgements therefore number at most two more than the
/// other frames queued (SPEC.md, "Resource bounds").
fn acknowledge(client: &mut Client) {
    if client.gone {
        return;
    }
    let bytes = encode_host(&HostMessage::InputAck {
        written: client.written,
        discarded: client.discarded,
    });
    if let Some(last) = client.outgoing.back_mut()
        && last.ack
        && last.written == 0
    {
        // Same message kind, same fixed size: `unsent` is unchanged.
        last.bytes = bytes;
        return;
    }
    client.unsent += bytes.len();
    client.outgoing.push_back(Outgoing {
        bytes,
        written: 0,
        output_end: None,
        ack: true,
        welcome: false,
    });
}

/// End the connection with `last` (`Goodbye` or `Error`): nothing more is
/// acted on or streamed, pending input is discarded unacknowledged, the
/// frames not started are dropped, `last` is queued after what is left, and
/// it has `GOODBYE_FLUSH` to go out (SPEC.md, "Ending" and "Host
/// messages").
fn end_connection(client: &mut Client, last: &HostMessage) {
    client.input.clear();
    client.input_cost = 0;
    client.cursor = None;
    drop_unstarted(client);
    queue(client, last);
    client.closing = true;
    client.flush_deadline = Some(Instant::now() + GOODBYE_FLUSH);
}

/// Drop the frames queued for the client that it has not started to
/// receive, before an `Error` or `Goodbye` that must come next. A frame
/// already partly written stays (cutting it would break the framing of
/// what follows), and so does a `Welcome` not written yet: what follows
/// it assumes the version it names.
fn drop_unstarted(client: &mut Client) {
    client.outgoing.retain(|out| out.written > 0 || out.welcome);
    client.unsent = client
        .outgoing
        .iter()
        .map(|out| out.bytes.len() - out.written)
        .sum();
}

fn push(fds: &mut Vec<libc::pollfd>, fd: RawFd, events: libc::c_short) -> usize {
    fds.push(libc::pollfd {
        fd,
        events,
        revents: 0,
    });
    fds.len() - 1
}

/// Write without blocking and without SIGPIPE.
fn send(fd: RawFd, bytes: &[u8]) -> io::Result<usize> {
    #[cfg(target_os = "linux")]
    let flags = libc::MSG_NOSIGNAL;
    #[cfg(not(target_os = "linux"))]
    let flags = 0;
    // SAFETY: sending from a valid buffer on a socket descriptor.
    let n = unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), flags) };
    if n < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(n as usize)
    }
}

/// How a [`receive`] call ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Received {
    /// The socket has nothing more right now.
    Drained,
    /// The buffer reached its limit; the socket may hold more.
    Full,
    /// End of stream, or the connection failed.
    Ended,
}

/// Read what the socket has into `buf` until `buf` holds `limit` bytes.
///
/// The limit is the whole bound on what one call keeps: a peer that refills
/// its socket as fast as the host drains it cannot make one call read more,
/// nor hold the loop here for longer than that takes. A `buf` already at the
/// limit is left as it is, without reading, so that is reported as `Full`
/// and not as the end of stream that might be waiting behind it.
fn receive(fd: RawFd, buf: &mut Vec<u8>, limit: usize) -> Received {
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let room = limit.saturating_sub(buf.len()).min(chunk.len());
        if room == 0 {
            return Received::Full;
        }
        // SAFETY: reading into a valid buffer from a non-blocking socket.
        let n = unsafe { libc::recv(fd, chunk.as_mut_ptr().cast(), room, 0) };
        if n > 0 {
            buf.extend_from_slice(&chunk[..n as usize]);
            continue;
        }
        if n == 0 {
            return Received::Ended;
        }
        match io::Error::last_os_error().kind() {
            io::ErrorKind::WouldBlock => return Received::Drained,
            io::ErrorKind::Interrupted => continue,
            _ => return Received::Ended,
        }
    }
}

/// Whether the peer on `fd` runs as this process's user. Linux and macOS
/// only, the platforms this crate supports, which both report peer
/// credentials.
fn same_user(fd: RawFd) -> bool {
    // SAFETY: geteuid has no preconditions.
    let me = unsafe { libc::geteuid() };
    #[cfg(target_os = "linux")]
    {
        let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: SO_PEERCRED fills a ucred of the given size.
        let result = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut cred as *mut libc::ucred).cast(),
                &mut len,
            )
        };
        result == 0 && cred.uid == me
    }
    #[cfg(target_os = "macos")]
    {
        let (mut uid, mut gid) = (0, 0);
        // SAFETY: getpeereid fills two ids for a connected socket.
        unsafe { libc::getpeereid(fd, &mut uid, &mut gid) == 0 && uid == me }
    }
}

/// Accept one connection from `listener`, close-on-exec and non-blocking:
/// in one call on Linux (`accept4`), in three on macOS, which has no
/// `accept4`. A connection whose flags cannot be set is closed and reported
/// as `EAGAIN`, so the caller simply stops for this turn.
fn accept_connection(listener: RawFd) -> io::Result<OwnedFd> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: accept4 on the listening socket with no address out.
        let fd = unsafe {
            libc::accept4(
                listener,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: accept4 returned a fresh descriptor.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
    #[cfg(not(target_os = "linux"))]
    {
        // SAFETY: accept on the listening socket with no address out.
        let fd = unsafe { libc::accept(listener, std::ptr::null_mut(), std::ptr::null_mut()) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: accept returned a fresh descriptor.
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        if crate::spawn::set_cloexec(fd.as_raw_fd()).is_err()
            || crate::spawn::set_nonblocking(fd.as_raw_fd()).is_err()
        {
            return Err(io::Error::from_raw_os_error(libc::EAGAIN));
        }
        Ok(fd)
    }
}

/// Which file a host's socket is: the device and inode of what `bind`
/// created at the socket path.
///
/// The host removes its socket file when it ends, but a path is only a
/// name: something else (a client that judged the host gone, say) may have
/// removed that file and bound a new socket there since, and removing that
/// one would leave its host unreachable for good. So the host remembers
/// which file it bound and removes only that one (SPEC.md, "Ending").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SocketFile {
    device: u64,
    inode: u64,
}

impl SocketFile {
    /// The file at `path` now (not following a symlink), if there is one.
    fn at(path: &Path) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        let metadata = std::fs::symlink_metadata(path).ok()?;
        Some(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

/// Remove the socket file at `path` if it is still `bound`, and leave
/// whatever else is there alone. (Between the check and the removal
/// something could still replace the file; no unlink call takes an
/// identity, and the window is two system calls wide.)
pub(crate) fn remove_socket(path: &Path, bound: SocketFile) {
    if SocketFile::at(path) == Some(bound) {
        let _ = std::fs::remove_file(path);
    }
}

/// Bind and listen on `socket`, which must not exist, and say which file
/// the bind created there.
pub(crate) fn listen(socket: &Path) -> Result<(OwnedFd, SocketFile), String> {
    let bytes = socket.as_os_str().as_bytes();
    // SAFETY: sockaddr_un is plain data.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.len() >= address.sun_path.len() {
        return Err(format!(
            "{} is too long for a socket path",
            socket.display()
        ));
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, byte) in address.sun_path.iter_mut().zip(bytes) {
        *slot = *byte as libc::c_char;
    }
    // Close-on-exec from the start where the platform can (see
    // `crate::spawn::pipe` for why that matters even in a single-threaded
    // host); macOS sets the flags after.
    #[cfg(target_os = "linux")]
    let kind = libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK;
    #[cfg(not(target_os = "linux"))]
    let kind = libc::SOCK_STREAM;
    // SAFETY: socket with constant arguments.
    let fd = unsafe { libc::socket(libc::AF_UNIX, kind, 0) };
    if fd < 0 {
        return Err(format!("creating a socket: {}", io::Error::last_os_error()));
    }
    // SAFETY: socket returned a fresh descriptor.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    #[cfg(not(target_os = "linux"))]
    {
        crate::spawn::set_cloexec(fd.as_raw_fd())
            .map_err(|error| format!("configuring the socket: {error}"))?;
        crate::spawn::set_nonblocking(fd.as_raw_fd())
            .map_err(|error| format!("configuring the socket: {error}"))?;
    }
    // SAFETY: binding a valid sockaddr_un of the stated size.
    let bound = unsafe {
        libc::bind(
            fd.as_raw_fd(),
            (&address as *const libc::sockaddr_un).cast(),
            std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        )
    };
    if bound != 0 {
        return Err(format!(
            "binding {}: {}",
            socket.display(),
            io::Error::last_os_error()
        ));
    }
    // Right after the bind, which is the only moment the file there is
    // known to be this socket's.
    let file = SocketFile::at(socket)
        .ok_or_else(|| format!("{} vanished as it was bound", socket.display()))?;
    // SAFETY: listen on a bound socket.
    if unsafe { libc::listen(fd.as_raw_fd(), 64) } != 0 {
        let error = io::Error::last_os_error();
        remove_socket(socket, file);
        return Err(format!("listening on {}: {error}", socket.display()));
    }
    Ok((fd, file))
}

/// Ignore SIGHUP, SIGINT and SIGPIPE; route SIGTERM and SIGCHLD to a
/// non-blocking self-pipe, and return its read end.
///
/// Without the pipe there would be no SIGTERM handling and no prompt
/// reaping, and SIGPIPE would stay at its default where `send` cannot
/// suppress it (macOS), so a host that cannot make one does not start.
pub(crate) fn install_signal_handlers() -> Result<OwnedFd, String> {
    let (read, write) =
        crate::spawn::pipe().map_err(|error| format!("creating the signal pipe: {error}"))?;
    for fd in [&read, &write] {
        crate::spawn::set_nonblocking(fd.as_raw_fd())
            .map_err(|error| format!("configuring the signal pipe: {error}"))?;
    }
    SIGNAL_PIPE.store(write.as_raw_fd(), std::sync::atomic::Ordering::SeqCst);
    std::mem::forget(write);
    // SAFETY: installing handlers; on_signal is async-signal-safe.
    unsafe {
        for signal in [libc::SIGHUP, libc::SIGINT, libc::SIGPIPE] {
            libc::signal(signal, libc::SIG_IGN);
        }
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = on_signal as *const () as usize;
        action.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut action.sa_mask);
        libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut());
        action.sa_flags |= libc::SA_NOCLDSTOP;
        libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut());
    }
    Ok(read)
}

/// In-process tests of the event loop: the same `Host` the daemon runs, on a
/// pty and child started here. The first few serve it from a thread while the
/// test talks to its socket, mainly so coverage tools see this file (code in
/// a daemonized host never reports back); the step-driven ones after them
/// pin, one iteration at a time, the bounds and orderings a real-binary test
/// cannot observe exactly. The real-binary tests in `tests/` remain the
/// behavioral spec.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Position, Spawn, decode_host, encode_client};
    use std::io::{Read as _, Write as _};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    /// A host serving `argv` in this process, and its socket.
    struct InProcess {
        _dir: tempfile::TempDir,
        socket: PathBuf,
        thread: Option<std::thread::JoinHandle<Ended>>,
    }

    fn serve(argv: &[&str], cap: u64, initial: Option<Vec<u8>>) -> InProcess {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        let spawn = Spawn {
            argv: argv.iter().map(|a| a.as_bytes().to_vec()).collect(),
            cwd: b"/".to_vec(),
            env: vec![b"PATH=/usr/bin:/bin".to_vec(), b"LC_ALL=C".to_vec()],
            size: Size { cols: 80, rows: 24 },
            journal_cap: cap,
            tag: b"tag".to_vec(),
            initial_snapshot: initial.clone(),
        };
        let (listener, socket_file) = listen(&socket).unwrap();
        let pty = crate::spawn::open_pty(spawn.size).unwrap();
        let child =
            crate::spawn::spawn_child(&pty, crate::spawn::Prepared::new(&spawn).unwrap()).unwrap();
        let host = Host::new(
            Setup {
                listener,
                socket: socket.clone(),
                socket_file,
                master: pty.master,
                child,
                size: spawn.size,
                journal_cap: cap,
                tag: spawn.tag.clone(),
            },
            initial,
            None,
        );
        let thread = std::thread::spawn(move || host.run());
        InProcess {
            _dir: dir,
            socket,
            thread: Some(thread),
        }
    }

    impl InProcess {
        fn connect(&self, take_over: bool) -> Conn {
            let mut conn = Conn::raw(&self.socket);
            conn.send(&ClientMessage::Hello {
                version: 1,
                take_over,
            });
            assert_eq!(conn.recv(), HostMessage::Welcome { version: 1 });
            conn
        }

        /// Shut the host down and return why it ended.
        fn shutdown(mut self) -> Ended {
            let mut conn = self.connect(true);
            conn.send(&ClientMessage::Shutdown);
            conn.until(|m| matches!(m, HostMessage::Goodbye));
            self.thread.take().unwrap().join().unwrap()
        }
    }

    impl Drop for InProcess {
        fn drop(&mut self) {
            if let Some(thread) = self.thread.take() {
                if let Ok(mut stream) = UnixStream::connect(&self.socket) {
                    let _ = stream.write_all(&encode_client(&ClientMessage::Hello {
                        version: 1,
                        take_over: true,
                    }));
                    let _ = stream.write_all(&encode_client(&ClientMessage::Shutdown));
                }
                let _ = thread.join();
            }
        }
    }

    struct Conn {
        stream: UnixStream,
        buf: Vec<u8>,
    }

    impl Conn {
        fn raw(socket: &Path) -> Self {
            let stream = UnixStream::connect(socket).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(50)))
                .unwrap();
            Self {
                stream,
                buf: Vec::new(),
            }
        }
        fn send(&mut self, message: &ClientMessage) {
            self.stream.write_all(&encode_client(message)).unwrap();
        }
        fn try_recv(&mut self) -> Option<HostMessage> {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(frame) = split_frame(&self.buf, MAX_FRAME).unwrap() {
                    let message = decode_host(frame, 1).unwrap();
                    let consumed = frame.consumed;
                    self.buf.drain(..consumed);
                    return Some(message);
                }
                assert!(Instant::now() < deadline, "no message in time");
                let mut chunk = [0u8; 65536];
                match self.stream.read(&mut chunk) {
                    Ok(0) => return None,
                    Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) => {}
                    Err(_) => return None,
                }
            }
        }
        fn recv(&mut self) -> HostMessage {
            self.try_recv().expect("connection closed")
        }
        /// The first message matching `predicate`, skipping others, within
        /// one deadline for the whole wait: a host that keeps sending
        /// something else must fail the test, not hold it.
        fn until(&mut self, mut predicate: impl FnMut(&HostMessage) -> bool) -> HostMessage {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                assert!(Instant::now() < deadline, "no matching message in time");
                let message = self.recv();
                if predicate(&message) {
                    return message;
                }
            }
        }
        /// Output until `pattern` appears in it, within one deadline and at
        /// most [`MAX_RETAINED`] bytes of output.
        fn output_until(&mut self, pattern: &str) -> String {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut text = Vec::new();
            while !String::from_utf8_lossy(&text).contains(pattern) {
                assert!(
                    Instant::now() < deadline && text.len() <= MAX_RETAINED,
                    "{pattern:?} never appeared in {} bytes of output",
                    text.len()
                );
                if let HostMessage::Output { data, .. } = self.recv() {
                    text.extend(data);
                }
            }
            String::from_utf8_lossy(&text).into_owned()
        }
        fn info(&mut self) -> Info {
            self.send(&ClientMessage::Info);
            match self.until(|m| matches!(m, HostMessage::InfoReply(_))) {
                HostMessage::InfoReply(info) => info,
                _ => unreachable!(),
            }
        }
    }

    /// The loop serves a whole session in-process: input and its echo with
    /// acknowledgements, a resize, a snapshot and a fresh read from it on a
    /// new connection, the child's exit, Info, and Shutdown.
    ///
    /// Why: one walk through every message, run where coverage can see it.
    /// Spec: as for the real-binary tests in `tests/host_lifecycle.rs`.
    #[test]
    fn a_whole_session_in_process() {
        let host = serve(
            &[
                "/bin/sh",
                "-c",
                "stty -echo; echo ready; read line; echo got:$line; exit 5",
            ],
            1 << 20,
            None,
        );
        let mut conn = host.connect(false);
        conn.send(&ClientMessage::Read(Position::default()));
        conn.output_until("ready");
        conn.send(&ClientMessage::Resize(Size {
            cols: 100,
            rows: 30,
        }));
        conn.until(|m| matches!(m, HostMessage::Resized { index: 0, .. }));
        conn.send(&ClientMessage::Input(b"hello\r".to_vec()));
        assert_eq!(
            conn.until(|m| matches!(m, HostMessage::InputAck { .. })),
            HostMessage::InputAck {
                written: 6,
                discarded: 0
            }
        );
        conn.output_until("got:hello");
        let exit = conn.until(|m| matches!(m, HostMessage::Exited { .. }));
        assert!(matches!(
            exit,
            HostMessage::Exited {
                exit: ChildExit::Code(5),
                ..
            }
        ));
        let info = conn.info();
        assert_eq!(info.child_exit, Some(ChildExit::Code(5)));
        assert_eq!(info.tag, b"tag");
        assert!(info.delivered_end > 0 && info.delivered_end <= info.end);
        conn.send(&ClientMessage::Snapshot {
            position: Position {
                offset: info.end,
                resizes: 1,
            },
            blob: b"snap".to_vec(),
        });
        assert!(matches!(
            conn.until(|m| matches!(
                m,
                HostMessage::SnapshotAccepted(_) | HostMessage::SnapshotRejected { .. }
            )),
            HostMessage::SnapshotAccepted(_)
        ));
        conn.send(&ClientMessage::Snapshot {
            position: Position::default(),
            blob: vec![],
        });
        assert!(matches!(
            conn.until(|m| matches!(m, HostMessage::SnapshotRejected { .. })),
            HostMessage::SnapshotRejected { .. }
        ));
        // A client re-reads by reconnecting: one Read per connection.
        drop(conn);
        let mut conn = host.connect(true);
        conn.send(&ClientMessage::Read(Position::default()));
        assert!(matches!(
            conn.until(|m| matches!(m, HostMessage::SnapshotRecord { .. })),
            HostMessage::SnapshotRecord { .. }
        ));
        // Input after the terminal closed is acknowledged as discarded; the
        // counts start again on the new connection.
        conn.send(&ClientMessage::Input(b"x".to_vec()));
        assert_eq!(
            conn.until(|m| matches!(m, HostMessage::InputAck { .. })),
            HostMessage::InputAck {
                written: 0,
                discarded: 1
            }
        );
        drop(conn);
        assert_eq!(host.shutdown(), Ended::Shutdown);
    }

    /// Busy, takeover, a version-0 hello, a frame before Hello, a malformed
    /// frame after Welcome and a second Hello are each handled as SPEC.md
    /// says, in-process.
    ///
    /// Why: the connection state machine, where coverage can see it. Spec:
    /// Busy then close without take_over; with it, the old client's stream
    /// ends with nothing more sent to it; version 0 gets UnsupportedVersion;
    /// a non-Hello first frame is closed; garbage and a repeated Hello get
    /// Error and close.
    #[test]
    fn connection_rules_in_process() {
        let host = serve(&["/bin/cat"], 1 << 20, None);
        let mut first = host.connect(false);
        let mut busy = Conn::raw(&host.socket);
        busy.send(&ClientMessage::Hello {
            version: 1,
            take_over: false,
        });
        assert_eq!(busy.recv(), HostMessage::Busy);
        assert_eq!(busy.try_recv(), None);
        let mut zero = Conn::raw(&host.socket);
        zero.send(&ClientMessage::Hello {
            version: 0,
            take_over: true,
        });
        assert!(matches!(
            zero.recv(),
            HostMessage::Error {
                code: ErrorCode::UnsupportedVersion,
                ..
            }
        ));
        let mut not_hello = Conn::raw(&host.socket);
        not_hello.send(&ClientMessage::Info);
        assert_eq!(not_hello.try_recv(), None);
        let mut garbage = Conn::raw(&host.socket);
        garbage.stream.write_all(&[0, 0, 0, 0]).unwrap();
        assert_eq!(garbage.try_recv(), None);
        let mut second = host.connect(true);
        assert_eq!(first.try_recv(), None, "the old client's stream just ends");
        second.send(&ClientMessage::Hello {
            version: 1,
            take_over: false,
        });
        assert!(matches!(
            second.recv(),
            HostMessage::Error {
                code: ErrorCode::NotAllowed,
                ..
            }
        ));
        assert_eq!(second.try_recv(), None);
        let mut third = host.connect(false);
        third.stream.write_all(&[0, 0, 0, 1, 0x7e]).unwrap();
        assert!(matches!(
            third.recv(),
            HostMessage::Error {
                code: ErrorCode::Malformed,
                ..
            }
        ));
        assert_eq!(third.try_recv(), None);
        // Frames that arrive with the Hello in one write are handled too.
        let mut together = Conn::raw(&host.socket);
        let mut bytes = encode_client(&ClientMessage::Hello {
            version: 7,
            take_over: false,
        });
        bytes.extend(encode_client(&ClientMessage::Info));
        together.stream.write_all(&bytes).unwrap();
        assert_eq!(
            together.recv(),
            HostMessage::Welcome { version: 1 },
            "the lower version is used"
        );
        assert!(matches!(together.recv(), HostMessage::InfoReply(_)));
        drop(together);
        assert_eq!(host.shutdown(), Ended::Shutdown);
    }

    /// Trimming and the initial snapshot, in-process: a host spawned with an
    /// initial snapshot and a small cap sends the snapshot, then a gap once
    /// output passed the cap.
    ///
    /// Why: the journal's read paths through the real loop. Spec: a fresh
    /// read starts with the initial snapshot record, then a gap, then the
    /// retained output.
    #[test]
    fn initial_snapshot_and_trimming_in_process() {
        let host = serve(
            &[
                "/bin/sh",
                "-c",
                "i=0; while [ $i -lt 6000 ]; do echo padding-padding-$i; i=$((i+1)); done; echo finished",
            ],
            64 * 1024,
            Some(b"history".to_vec()),
        );
        let mut conn = host.connect(false);
        let deadline = Instant::now() + Duration::from_secs(10);
        while conn.info().child_exit.is_none() {
            assert!(Instant::now() < deadline);
            // sleep-ok: polling interval for the child's exit.
            std::thread::sleep(Duration::from_millis(20));
        }
        conn.send(&ClientMessage::Read(Position::default()));
        assert_eq!(
            conn.recv(),
            HostMessage::SnapshotRecord {
                position: Position::default(),
                blob: b"history".to_vec()
            }
        );
        assert!(matches!(conn.recv(), HostMessage::Gap { from: 0, .. }));
        conn.output_until("finished");
        drop(conn);
        assert_eq!(host.shutdown(), Ended::Shutdown);
    }

    // ---------------------------------------------------------------------
    // Step-driven tests
    //
    // The tests above serve the loop from a thread. The ones below call
    // `Host::step` from the test's own thread instead, between their own
    // reads and writes on the client socket. That makes the order of events
    // exact (a frame is buffered in the host, partly written, or still in the
    // socket when the test says so), and it lets a test look at the host's
    // own state between iterations. Bounds a client cannot see directly (how
    // much one iteration reads, how much is queued, what poll waits for) are
    // pinned that way, without measuring time. Most of them exist because a
    // mutation run showed the bound was not tested (TESTING.md, "Mutation
    // testing").
    // ---------------------------------------------------------------------

    use crate::protocol::{MAX_INPUT, MAX_OUTPUT_CHUNK};
    use std::os::unix::fs::OpenOptionsExt as _;

    /// How long a step-driven test waits for one thing before failing. Long,
    /// because the machine running the tests may be loaded; a correct host
    /// gets there in milliseconds.
    const PATIENCE: Duration = Duration::from_secs(30);

    /// The most output a test helper keeps while it waits for a pattern: far
    /// more than any test's program prints before it, and little enough that
    /// a host streaming without end fails the wait instead of growing it.
    const MAX_RETAINED: usize = 16 << 20;

    /// A host that the test drives one [`Host::step`] at a time. Its socket
    /// is `h` in `dir`, beside whatever else the test keeps there (a FIFO,
    /// an output file).
    struct Stepped {
        dir: tempfile::TempDir,
        host: Host,
    }

    /// A stepped host on a real pty, running `argv` with a minimal
    /// environment.
    fn stepped(dir: tempfile::TempDir, argv: &[&str], cap: u64) -> Stepped {
        let spawn = Spawn {
            argv: argv.iter().map(|a| a.as_bytes().to_vec()).collect(),
            cwd: b"/".to_vec(),
            env: vec![b"PATH=/usr/bin:/bin".to_vec(), b"LC_ALL=C".to_vec()],
            size: Size { cols: 80, rows: 24 },
            journal_cap: cap,
            tag: b"tag".to_vec(),
            initial_snapshot: None,
        };
        let pty = crate::spawn::open_pty(spawn.size).unwrap();
        let child =
            crate::spawn::spawn_child(&pty, crate::spawn::Prepared::new(&spawn).unwrap()).unwrap();
        stepped_on(dir, pty.master, child, cap)
    }

    /// A stepped host whose "pty master" is `master`, which can be any
    /// descriptor the loop can poll, read and write, serving `child`, which
    /// must be a child of this process that nothing else waits for.
    fn stepped_on(dir: tempfile::TempDir, master: OwnedFd, child: u32, cap: u64) -> Stepped {
        let socket = dir.path().join("h");
        let (listener, socket_file) = listen(&socket).unwrap();
        let host = Host::new(
            Setup {
                listener,
                socket,
                socket_file,
                master,
                child,
                size: Size { cols: 80, rows: 24 },
                journal_cap: cap,
                tag: b"tag".to_vec(),
            },
            None,
            None,
        );
        Stepped { dir, host }
    }

    impl Drop for Stepped {
        /// Kill and reap a child the host has not reaped, so that no test
        /// leaves a process or a zombie behind. A child the host reaped is
        /// left alone: its pid may already belong to another test's child.
        fn drop(&mut self) {
            // A pid of 0 or 1 (possible only from a broken spawn, as under
            // mutation testing) would signal this test's own process group
            // or init; leave it alone.
            if self.host.child_exit.is_none() && self.host.setup.child > 1 {
                let pid = self.host.setup.child as libc::pid_t;
                // SAFETY: signalling and reaping this test's own child.
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                    libc::waitpid(pid, std::ptr::null_mut(), 0);
                }
            }
        }
    }

    impl Stepped {
        fn socket(&self) -> PathBuf {
            self.dir.path().join("h")
        }

        /// One iteration, which must not end the host.
        fn step(&mut self) {
            assert_eq!(self.host.step(), None, "the host ended unexpectedly");
        }

        /// Step until `done` holds, or fail after [`PATIENCE`].
        fn step_until(&mut self, what: &str, mut done: impl FnMut(&Host) -> bool) {
            let deadline = Instant::now() + PATIENCE;
            while !done(&self.host) {
                assert!(Instant::now() < deadline, "timed out waiting for {what}");
                self.step();
            }
        }

        fn client(&self) -> &Client {
            self.host.client.as_ref().expect("a client is connected")
        }

        /// Connect a client and step until it is welcomed.
        fn connect(&mut self, take_over: bool) -> Peer {
            let mut peer = Peer::raw(&self.socket());
            peer.send(
                self,
                &ClientMessage::Hello {
                    version: 1,
                    take_over,
                },
            );
            assert_eq!(peer.recv(self), Some(HostMessage::Welcome { version: 1 }));
            peer
        }

        /// Wait, without stepping, until the client's socket shows the
        /// peer's close.
        ///
        /// A test that closes its end and then steps once cannot rely on the
        /// close being visible at that step: tests run in parallel threads
        /// of one process, and a child another test forks holds a copy of
        /// every descriptor (close-on-exec ones included) until it execs,
        /// which keeps the connection open that long.
        fn wait_for_hangup(&self) {
            let deadline = Instant::now() + PATIENCE;
            loop {
                let mut fd = libc::pollfd {
                    fd: self.client().fd.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: one valid pollfd.
                unsafe { libc::poll(&mut fd, 1, 100) };
                if fd.revents & libc::POLLHUP != 0 {
                    return;
                }
                assert!(Instant::now() < deadline, "the close never showed");
            }
        }

        /// The bytes queued for the client and not yet written, checked
        /// against the running total the reply budget uses.
        fn unsent(&self) -> usize {
            let client = self.client();
            let unsent = client
                .outgoing
                .iter()
                .map(|out| out.bytes.len() - out.written)
                .sum();
            assert_eq!(client.unsent, unsent, "the running total of unsent bytes");
            unsent
        }

        /// The messages queued for the client that it has not started to
        /// receive, decoded.
        fn queued(&self) -> Vec<HostMessage> {
            self.client()
                .outgoing
                .iter()
                .filter(|out| out.written == 0)
                .map(|out| decode_host(split_frame(&out.bytes, MAX_FRAME).unwrap().unwrap(), 1))
                .map(Result::unwrap)
                .collect()
        }

        /// Whether the client's stream has nothing more to send right now.
        fn caught_up(&self) -> bool {
            self.client()
                .cursor
                .as_ref()
                .is_none_or(|cursor| self.host.journal.next(&mut cursor.clone()).is_none())
        }
    }

    /// The test's end of a connection to a [`Stepped`] host. It never
    /// blocks: where a blocking client would wait, it steps the host
    /// instead, so that one thread plays both sides.
    struct Peer {
        stream: UnixStream,
        buf: Vec<u8>,
    }

    impl Peer {
        fn raw(socket: &Path) -> Self {
            let stream = UnixStream::connect(socket).unwrap();
            stream.set_nonblocking(true).unwrap();
            Self {
                stream,
                buf: Vec::new(),
            }
        }

        /// Write all of `bytes`, stepping the host whenever the socket is
        /// full.
        fn write(&mut self, host: &mut Stepped, mut bytes: &[u8]) {
            let deadline = Instant::now() + PATIENCE;
            while !bytes.is_empty() {
                match self.stream.write(bytes) {
                    Ok(n) => bytes = &bytes[n..],
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "the host stopped reading");
                        host.step();
                    }
                    Err(error) => panic!("writing to the host: {error}"),
                }
            }
        }

        fn send(&mut self, host: &mut Stepped, message: &ClientMessage) {
            self.write(host, &encode_client(message));
        }

        /// Read whatever has arrived, without stepping. False at end of
        /// stream (a reset counts: the host closes a connection whose
        /// unread frames it no longer wants).
        fn fill(&mut self) -> bool {
            let mut chunk = [0u8; 65536];
            loop {
                match self.stream.read(&mut chunk) {
                    Ok(0) => return false,
                    Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => return true,
                    Err(error) if error.kind() == io::ErrorKind::ConnectionReset => return false,
                    Err(error) => panic!("reading from the host: {error}"),
                }
            }
        }

        /// Wait until the host has closed this connection, without
        /// stepping it: for a connection the host already dropped.
        ///
        /// The end of stream can lag the host's close here, unlike in a real
        /// host, when tests run in parallel threads of one process (nextest
        /// isolates them, and libtest is to be run with one thread; see
        /// TESTING.md): a child another test forks holds a copy of every
        /// descriptor until it execs, and on macOS one it starts inherits
        /// whatever is not close-on-exec yet, which can be a connection the
        /// host has just accepted (macOS has no `accept4`). Waiting covers
        /// that case if a parallel run happens anyway.
        fn wait_closed(&mut self) {
            let deadline = Instant::now() + PATIENCE;
            while self.fill() {
                assert!(
                    Instant::now() < deadline,
                    "the host never closed the connection"
                );
                let mut fd = libc::pollfd {
                    fd: self.stream.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: one valid pollfd; waits for the close (or data).
                unsafe { libc::poll(&mut fd, 1, 100) };
            }
        }

        /// A whole message already received, if there is one. Panics on a
        /// frame that does not decode: the stream's framing is broken.
        fn take(&mut self) -> Option<HostMessage> {
            let frame = split_frame(&self.buf, MAX_FRAME).unwrap()?;
            let consumed = frame.consumed;
            let message = decode_host(frame, 1).unwrap();
            self.buf.drain(..consumed);
            Some(message)
        }

        /// The next message, stepping the host until one arrives; `None` at
        /// end of stream.
        fn recv(&mut self, host: &mut Stepped) -> Option<HostMessage> {
            let deadline = Instant::now() + PATIENCE;
            loop {
                if let Some(message) = self.take() {
                    return Some(message);
                }
                let open = self.fill();
                if let Some(message) = self.take() {
                    return Some(message);
                }
                if !open {
                    assert!(self.buf.is_empty(), "the stream ended inside a frame");
                    return None;
                }
                assert!(Instant::now() < deadline, "no message in time");
                host.step();
            }
        }

        /// The first message matching `predicate`, skipping others, within
        /// [`PATIENCE`] for the whole wait.
        fn until(
            &mut self,
            host: &mut Stepped,
            mut predicate: impl FnMut(&HostMessage) -> bool,
        ) -> HostMessage {
            let deadline = Instant::now() + PATIENCE;
            loop {
                assert!(Instant::now() < deadline, "no matching message in time");
                let message = self.recv(host).expect("connection closed");
                if predicate(&message) {
                    return message;
                }
            }
        }

        /// Step until output with `pattern` arrives, within [`PATIENCE`] and
        /// at most [`MAX_RETAINED`] bytes of output.
        fn output_until(&mut self, host: &mut Stepped, pattern: &str) {
            let deadline = Instant::now() + PATIENCE;
            let mut text = Vec::new();
            while !String::from_utf8_lossy(&text).contains(pattern) {
                assert!(
                    Instant::now() < deadline && text.len() <= MAX_RETAINED,
                    "{pattern:?} never appeared in {} bytes of output",
                    text.len()
                );
                match self.recv(host) {
                    Some(HostMessage::Output { data, .. }) => text.extend(data),
                    Some(_) => {}
                    None => panic!("the stream ended before {pattern:?}"),
                }
            }
        }
    }

    /// A file of `len` bytes, opened read-write, to stand in for a pty
    /// master whose program has more output than the host can take at once.
    /// A regular file always polls readable, a read returns what is left of
    /// it, and a read at its end returns 0, which the host takes as the end
    /// of output (the way macOS reports it; Linux reports EIO). Unlike a pty,
    /// how much is ready does not depend on the kernel's tty buffering or on
    /// when the program runs.
    fn output_file(dir: &Path, len: usize) -> OwnedFd {
        let path = dir.join("output");
        std::fs::write(&path, vec![b'o'; len]).unwrap();
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap()
            .into()
    }

    /// A child of this process that runs until the [`Stepped`] owning it
    /// kills it.
    fn idle_child() -> u32 {
        std::process::Command::new("/bin/sleep")
            .arg("1000")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap()
            .id()
    }

    /// Wait for `pid`, a child of this process, to exit, without reaping
    /// it: the host under test is the one that must reap it.
    fn wait_exited(pid: u32) {
        // SAFETY: siginfo_t is plain data; waitid with WNOWAIT only waits.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        assert_eq!(result, 0, "waitid: {}", io::Error::last_os_error());
    }

    /// A child of this process that has exited with status 0 and is not
    /// reaped yet. (`/bin/sh`, not `/bin/true`: macOS has no `/bin/true`.)
    fn exited_child() -> u32 {
        let pid = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .stdin(std::process::Stdio::null())
            .spawn()
            .unwrap()
            .id();
        wait_exited(pid);
        pid
    }

    /// A FIFO in `dir` that a child's script blocks on until
    /// [`release`] writes a line to it.
    fn make_fifo(dir: &Path) -> PathBuf {
        let path = dir.join("go");
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: mkfifo with a valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        path
    }

    /// Let the child blocked reading `fifo` continue. Steps the host while
    /// the child has not opened the FIFO yet (opening a FIFO without a
    /// reader fails with ENXIO when non-blocking).
    fn release(host: &mut Stepped, fifo: &Path) {
        let deadline = Instant::now() + PATIENCE;
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(fifo)
            {
                Ok(mut file) => {
                    file.write_all(b"go\n").unwrap();
                    return;
                }
                Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
                    assert!(Instant::now() < deadline, "the child never opened the FIFO");
                    host.step();
                }
                Err(error) => panic!("opening the FIFO: {error}"),
            }
        }
    }

    /// A non-blocking pipe standing in for the signal pipe
    /// [`install_signal_handlers`] makes, without installing any handler in
    /// the test process. Returns (read end, write end).
    fn signal_pipe() -> (OwnedFd, OwnedFd) {
        let (read, write) = crate::spawn::pipe().unwrap();
        crate::spawn::set_nonblocking(read.as_raw_fd()).unwrap();
        crate::spawn::set_nonblocking(write.as_raw_fd()).unwrap();
        (read, write)
    }

    impl Client {
        /// Bytes of `Input` this connection's host took in: resolved, or
        /// still pending. What a test compares with what it sent to know
        /// the host has acted on it.
        fn taken_in(&self) -> u64 {
            let pending: usize = self.input.iter().map(|segment| segment.data.len()).sum();
            self.written + self.discarded + pending as u64
        }
    }

    /// Send input to a program that does not read its terminal until the
    /// pty is full and some input waits in the host, and return the pending
    /// cost then. The pty must be in raw mode: in canonical mode the kernel
    /// discards characters beyond a full line buffer instead of refusing
    /// them, and the pty never fills.
    fn fill_the_pty(host: &mut Stepped, peer: &mut Peer) -> usize {
        let deadline = Instant::now() + PATIENCE;
        loop {
            assert!(Instant::now() < deadline, "the pty never filled");
            let sent = host.client().taken_in() + MAX_INPUT as u64;
            peer.send(host, &ClientMessage::Input(vec![b'x'; MAX_INPUT]));
            host.step_until("the input to be taken in", |host| {
                host.client.as_ref().unwrap().taken_in() == sent
            });
            // Stuck means unchanged over several iterations, each of which
            // tried to write.
            let pending = host.client().input_cost;
            for _ in 0..3 {
                host.step();
            }
            if pending > 0 && host.client().input_cost == pending {
                return pending;
            }
        }
    }

    /// Send `Input`s until the pending cost (unwritten bytes plus
    /// `INPUT_COST` per `Input`) is exactly `target`. The pty must be full
    /// already (see [`fill_the_pty`]), and `target` must be the cost now or
    /// more than `INPUT_COST` above it, since no `Input` costs less.
    fn top_up(host: &mut Stepped, peer: &mut Peer, target: usize) {
        let mut missing = target - host.client().input_cost;
        let mut frames = Vec::new();
        while missing > 0 {
            assert!(missing > INPUT_COST, "no Input costs only {missing}");
            let mut len = (missing - INPUT_COST).min(MAX_INPUT);
            // Never leave a remainder that no Input could make up.
            let rest = missing - len - INPUT_COST;
            if (1..=INPUT_COST).contains(&rest) {
                len -= INPUT_COST + 1 - rest;
            }
            frames.extend(encode_client(&ClientMessage::Input(vec![b'x'; len])));
            missing -= len + INPUT_COST;
        }
        peer.write(host, &frames);
        host.step_until("the input to be taken in", |host| {
            host.client.as_ref().unwrap().input_cost == target
        });
        assert!(host.client().buf.is_empty(), "every Input was acted on");
    }

    /// One iteration reads `READ_BUDGET` bytes of output when more is
    /// waiting, no more and no less, and a read of 0 ends the output.
    ///
    /// Why: the budget is what lets the loop serve its client and control
    /// messages while a program prints without pause; without a bound, one
    /// iteration would read for as long as the program keeps up. Too small a
    /// budget is a slowdown nobody would notice. Spec: with 3 MiB + 10 bytes
    /// ready, the first three iterations journal exactly 1 MiB each, and the
    /// fourth journals the last 10 bytes and sees the end of output.
    #[test]
    fn one_iteration_reads_the_read_budget() {
        let dir = tempfile::tempdir().unwrap();
        let len = 3 * READ_BUDGET + 10;
        let master = output_file(dir.path(), len);
        let mut host = stepped_on(dir, master, idle_child(), 8 << 20);
        for iteration in 1..=3 {
            host.step();
            assert_eq!(
                host.host.journal.end(),
                (iteration * READ_BUDGET) as u64,
                "after iteration {iteration}"
            );
            assert!(host.host.pty_open);
        }
        host.step();
        assert_eq!(host.host.journal.end(), len as u64);
        assert!(!host.host.pty_open, "a read of 0 is the end of output");
    }

    /// When the child exits while its output keeps coming, the exit record
    /// waits for up to `EXIT_DRAIN_TIME` and `EXIT_DRAIN_BYTES` of it, and
    /// what follows is journaled after the record; `Info` reports the exit
    /// from the reaping on.
    ///
    /// Why: a program that prints and exits must have its output before its
    /// exit, but a descendant that keeps printing must not keep the exit from
    /// being reported (SPEC.md, "Child exit and end of output"). And a
    /// client uses `Info`'s "running" as leave to treat the child pid as its
    /// process tree's root; once the pid is reaped it can belong to anyone,
    /// so `Info` must not wait for the exit record. Spec: on reaping, the
    /// drain's deadline is `EXIT_DRAIN_TIME` away; an `Info` answered while
    /// the drain still runs reports the exit; with the deadline out of play,
    /// the exit is journaled once at least `EXIT_DRAIN_BYTES` were read
    /// since the reaping (and within one more iteration's reads), not
    /// earlier and not at the end of output; the rest of the output is
    /// still journaled.
    #[test]
    fn the_exit_waits_for_arriving_output_up_to_the_drain_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let len = 2 * EXIT_DRAIN_BYTES;
        let master = output_file(dir.path(), len);
        let mut host = stepped_on(dir, master, exited_child(), 16 << 20);
        // A client whose Hello and Info wait in the socket from the start,
        // so that the Info is answered in the second iteration, mid-drain.
        let mut peer = Peer::raw(&host.socket());
        let bytes = [
            encode_client(&ClientMessage::Hello {
                version: 1,
                take_over: false,
            }),
            encode_client(&ClientMessage::Info),
        ]
        .concat();
        peer.stream.write_all(&bytes).unwrap();
        let before = Instant::now();
        host.step();
        assert_eq!(host.host.child_exit, Some(ChildExit::Code(0)), "reaped");
        // An iteration slower than EXIT_DRAIN_TIME (a loaded machine) ends
        // the drain on time instead; the message says so.
        let (_, deadline, _) = host.host.draining.unwrap_or_else(|| {
            panic!(
                "still draining: more output is ready (the iteration took {:?})",
                before.elapsed()
            )
        });
        assert!(
            deadline >= before + EXIT_DRAIN_TIME,
            "the drain may last EXIT_DRAIN_TIME"
        );
        assert_eq!(host.host.journal.exit(), None);
        // Take the time bound out of play so that the byte bound alone
        // decides, whatever the machine's load does to the iterations.
        if let Some((_, deadline, _)) = &mut host.host.draining {
            *deadline = Instant::now() + Duration::from_secs(3600);
        }
        host.step();
        assert!(
            host.host.draining.is_some() && host.host.journal.exit().is_none(),
            "premise: the drain still runs"
        );
        let replies = host.queued();
        let info = replies
            .iter()
            .find_map(|message| match message {
                HostMessage::InfoReply(info) => Some(info),
                _ => None,
            })
            .expect("the Info was answered");
        assert_eq!(info.child_exit, Some(ChildExit::Code(0)));
        host.step_until("the exit to be journaled", |host| {
            host.journal.exit().is_some()
        });
        let at = host.host.journal.end() as usize;
        assert!(
            (EXIT_DRAIN_BYTES..EXIT_DRAIN_BYTES + 2 * READ_BUDGET).contains(&at),
            "exit journaled at offset {at}"
        );
        host.step_until("the end of output", |host| !host.pty_open);
        assert_eq!(host.host.journal.end(), len as u64);
    }

    /// A child that exits with nothing left to read has its exit journaled
    /// in the iteration that reaps it.
    ///
    /// Why: the drain bounds are for output still arriving; a host that
    /// waited them out anyway would report every exit 200 ms late. Spec:
    /// after `sh -c 'exit 3'` has exited, one iteration both reaps it and
    /// journals the exit with code 3.
    #[test]
    fn an_exit_with_nothing_left_to_read_is_journaled_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sh", "-c", "exit 3"], 1 << 20);
        wait_exited(host.host.setup.child);
        host.step();
        assert_eq!(host.host.child_exit, Some(ChildExit::Code(3)));
        assert_eq!(host.host.journal.exit(), Some(ChildExit::Code(3)));
    }

    /// On Linux, a running child that closes its terminal ends the output,
    /// and input after that is acknowledged.
    ///
    /// Why: end of output is not tied to the child's exit (SPEC.md, "Child
    /// exit and end of output"); here the pty reports only a hangup, with no
    /// data, and the host must still read it to learn that output ended.
    /// Spec: after `exec </dev/null >/dev/null 2>&1` in a child that keeps
    /// running, the host stops polling the pty, and an `Input` is
    /// acknowledged as discarded, none of it written.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_running_child_that_closes_its_terminal_ends_the_output() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(
            dir,
            &[
                "/bin/sh",
                "-c",
                "exec </dev/null >/dev/null 2>&1; exec /bin/sleep 1000",
            ],
            1 << 20,
        );
        host.step_until("the end of output", |host| !host.pty_open);
        assert_eq!(host.host.child_exit, None, "the child still runs");
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Input(b"x".to_vec()));
        assert_eq!(
            peer.until(&mut host, |m| matches!(m, HostMessage::InputAck { .. })),
            HostMessage::InputAck {
                written: 0,
                discarded: 1
            }
        );
    }

    /// On macOS, a running child that closes its terminal descriptors keeps
    /// the output open until it exits.
    ///
    /// Why: macOS counts a session leader's controlling terminal as holding
    /// the slave open, so the master reports no end of output while the
    /// child lives (SPEC.md, "Child exit and end of output" says so). A host
    /// that guessed otherwise would discard input to a program that may
    /// still reopen its terminal. Spec: after `exec </dev/null >/dev/null
    /// 2>&1` in a child that then waits, the pty stays open across several
    /// iterations; once the child exits, the output ends.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_running_child_that_closes_its_terminal_keeps_the_output_until_it_exits() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = make_fifo(dir.path());
        let script = format!(
            "exec </dev/null >/dev/null 2>&1; read go < '{}'; exit 0",
            fifo.display()
        );
        let mut host = stepped(dir, &["/bin/sh", "-c", &script], 1 << 20);
        // The FIFO opens for writing once the child opened it for reading,
        // which it does after closing its terminal descriptors.
        let deadline = Instant::now() + PATIENCE;
        let mut writer = loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&fifo)
            {
                Ok(file) => break file,
                Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
                    assert!(Instant::now() < deadline, "the child never opened the FIFO");
                    host.step();
                }
                Err(error) => panic!("opening the FIFO: {error}"),
            }
        };
        for _ in 0..10 {
            host.step();
            assert!(host.host.pty_open, "the output ended while the child runs");
        }
        writer.write_all(b"go\n").unwrap();
        host.step_until("the child's exit", |host| host.child_exit.is_some());
        host.step_until("the end of output", |host| !host.pty_open);
    }

    /// Input larger than the pty holds is written in full and in order,
    /// across partial writes, and acknowledged in whole `Input`s.
    ///
    /// Why: the program must receive every byte the client was told was
    /// written, an acknowledgement must never count part of an `Input` (a
    /// client resends nothing it saw acknowledged, and must be able to tell
    /// which `Input`s that covers), and the host's pending cost must return
    /// to zero, or the input bound would eventually stop the host reading
    /// its client. Spec: 1 MiB sent as 64 KiB `Input`s to a raw-mode
    /// `head -c 1048576` reaches it in full (it prints "done"); the acks'
    /// written totals rise strictly, each a whole number of `Input`s, the
    /// last the full 1 MiB, none discarded; at least one `Input` was written
    /// in more than one piece; nothing is pending afterwards.
    #[test]
    fn input_beyond_what_the_pty_holds_is_written_in_full() {
        const TOTAL: usize = 1 << 20;
        let script = format!(
            "stty raw -echo; echo ready; head -c {TOTAL} >/dev/null; echo done; exec /bin/sleep 1000"
        );
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sh", "-c", &script], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        peer.output_until(&mut host, "ready");
        let frames: Vec<u8> = (0..TOTAL / MAX_INPUT)
            .flat_map(|_| encode_client(&ClientMessage::Input(vec![b'x'; MAX_INPUT])))
            .collect();
        let mut written = 0;
        let mut acks = Vec::new();
        let mut output = Vec::new();
        let mut split = false;
        let deadline = Instant::now() + PATIENCE;
        while !String::from_utf8_lossy(&output).contains("done") {
            assert!(Instant::now() < deadline, "the program never got all input");
            if written < frames.len() {
                match peer.stream.write(&frames[written..]) {
                    Ok(n) => written += n,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(error) => panic!("{error}"),
                }
            }
            host.step();
            split |= host
                .client()
                .input
                .front()
                .is_some_and(|segment| segment.written > 0);
            peer.fill();
            while let Some(message) = peer.take() {
                match message {
                    HostMessage::InputAck { written, discarded } => acks.push((written, discarded)),
                    HostMessage::Output { data, .. } => output.extend(data),
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
        assert!(split, "premise: some Input was written in pieces");
        host.step_until("the last acknowledgement", |host| {
            host.client.as_ref().unwrap().outgoing.is_empty()
        });
        peer.fill();
        while let Some(message) = peer.take() {
            if let HostMessage::InputAck { written, discarded } = message {
                acks.push((written, discarded));
            }
        }
        assert!(
            acks.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "{acks:?}"
        );
        assert!(
            acks.iter()
                .all(|&(written, discarded)| written % MAX_INPUT as u64 == 0 && discarded == 0),
            "{acks:?}"
        );
        assert_eq!(acks.last(), Some(&(TOTAL as u64, 0)));
        assert_eq!(host.client().input_cost, 0);
        assert!(host.client().input.is_empty());
    }

    /// A raw-mode program that prints "ready", waits for [`release`] on
    /// `fifo` without reading its terminal, then runs `then`.
    fn waits_then(fifo: &Path, then: &str) -> String {
        format!(
            "stty raw -echo; echo ready; read go < '{}'; {then}",
            fifo.display()
        )
    }

    /// An `Input` that would take pending input past `MAX_PENDING_INPUT`
    /// waits in the host's buffer with every frame after it, while the
    /// program's output still arrives and a close is still noticed.
    ///
    /// Why: the bound is the backstop that keeps a misbehaving client from
    /// growing the host without limit (SPEC.md, "Input"), so it must hold
    /// exactly, not up to one `Input` beyond; frames after the held one must
    /// not overtake it, or replies would arrive out of order with input; and
    /// holding must not cost the program its output or the next client its
    /// connection. Each pending `Input` counts `INPUT_COST` beyond its data,
    /// or a client sending tiny `Input`s would hold a million of them under
    /// the bound. Spec: with a raw-mode program that never reads its
    /// terminal and a pending cost of `MAX_PENDING_INPUT - INPUT_COST - 1`,
    /// a 2-byte `Input` (whose data alone would fit, but not with its
    /// `INPUT_COST`), a 1-byte `Input` (which alone would fit exactly) and
    /// an `Info` sent next all stay in the host's buffer, the pending cost
    /// stays where it was, and no `InfoReply` is queued; output the program
    /// prints afterwards still arrives; when the client closes, it is
    /// dropped, and a new client without take-over is welcomed.
    #[test]
    fn an_input_that_does_not_fit_waits_with_everything_after_it() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = make_fifo(dir.path());
        let script = waits_then(&fifo, "echo late; exec /bin/sleep 1000");
        let mut host = stepped(dir, &["/bin/sh", "-c", &script], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        peer.output_until(&mut host, "ready");
        fill_the_pty(&mut host, &mut peer);
        let pending = MAX_PENDING_INPUT - INPUT_COST - 1;
        top_up(&mut host, &mut peer, pending);
        let held = [
            encode_client(&ClientMessage::Input(b"xx".to_vec())),
            encode_client(&ClientMessage::Input(b"y".to_vec())),
            encode_client(&ClientMessage::Info),
        ]
        .concat();
        peer.write(&mut host, &held);
        host.step_until("the frames to be read", |host| {
            host.client.as_ref().unwrap().buf.len() == held.len()
        });
        for _ in 0..3 {
            host.step();
        }
        assert_eq!(host.client().buf.len(), held.len(), "every frame waits");
        assert_eq!(
            host.client().input_cost,
            pending,
            "pending input never passes the bound, and nothing overtook"
        );
        assert!(
            !host
                .queued()
                .iter()
                .any(|m| matches!(m, HostMessage::InfoReply(_))),
            "the Info waits behind the held Input"
        );
        release(&mut host, &fifo);
        peer.output_until(&mut host, "late");
        assert_ne!(
            client_events(host.client()) & libc::POLLIN,
            0,
            "premise: below the buffer limit the socket is polled"
        );
        drop(peer);
        host.step_until("the closed client to be dropped", |host| {
            host.client.is_none()
        });
        let _next = host.connect(false);
    }

    /// While held frames fill the client's buffer and nothing is queued for
    /// it, the host stops reading the client and leaves it out of poll; once
    /// the program takes input again, the held frames are acted on.
    ///
    /// Why: reading on would grow the host without bound, and polling the
    /// socket with no events behaves differently by platform (once the peer
    /// closed, Linux reports a hangup at once, every time; macOS reports
    /// nothing), so the host leaves such a client alone on both (SPEC.md,
    /// "Input"). The
    /// buffer is the host's bound on one client's unacted frames: one
    /// maximum frame, exactly. Spec: with a pending cost of
    /// `MAX_PENDING_INPUT` to a raw-mode program that does not read, and
    /// `MAX_INPUT` `Input`s sent
    /// until the host's buffer is full, it holds exactly `MAX_FRAME + 4`
    /// bytes, the client's poll events are none, and an iteration with
    /// nothing else to do waits most of its 20 ms timeout; once the program
    /// reads its terminal, every `Input` sent is acknowledged.
    #[test]
    fn a_client_whose_held_frames_fill_its_buffer_is_not_polled() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = make_fifo(dir.path());
        let script = waits_then(&fifo, "exec /bin/cat >/dev/null");
        let mut host = stepped(dir, &["/bin/sh", "-c", &script], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        peer.output_until(&mut host, "ready");
        fill_the_pty(&mut host, &mut peer);
        top_up(&mut host, &mut peer, MAX_PENDING_INPUT);
        let frame = encode_client(&ClientMessage::Input(vec![b'x'; MAX_INPUT]));
        let mut backlog: Vec<u8> = Vec::new();
        let mut total = host.client().taken_in();
        let deadline = Instant::now() + PATIENCE;
        while host.client().buf.len() < MAX_FRAME + 4 {
            assert!(Instant::now() < deadline, "the host's buffer never filled");
            if backlog.len() < frame.len() {
                backlog.extend(&frame);
                total += MAX_INPUT as u64;
            }
            match peer.stream.write(&backlog) {
                Ok(n) => {
                    backlog.drain(..n);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("writing to the host: {error}"),
            }
            host.step();
        }
        assert_eq!(host.client().buf.len(), MAX_FRAME + 4, "one maximum frame");
        host.step_until("everything queued to be written", |host| {
            host.client.as_ref().unwrap().outgoing.is_empty()
        });
        assert_eq!(host.client().buf.len(), MAX_FRAME + 4, "not read further");
        assert_eq!(host.client().input_cost, MAX_PENDING_INPUT, "still stuck");
        assert_eq!(client_events(host.client()), 0);
        let started = Instant::now();
        host.step();
        assert!(
            started.elapsed() >= Duration::from_millis(15),
            "an iteration with the client at its limit returned after {:?}",
            started.elapsed()
        );
        release(&mut host, &fifo);
        peer.write(&mut host, &backlog);
        peer.until(
            &mut host,
            |m| matches!(m, HostMessage::InputAck { written, .. } if *written == total),
        );
    }

    /// Input still pending when the client disconnects never reaches the
    /// program, even once the program reads again.
    ///
    /// Why: a client that reconnects cannot know what was written, and
    /// SPEC.md ("Input") promises that the rest is discarded; a host that
    /// wrote it later would type into the program for a client that is
    /// gone. Spec: with a raw-mode program that does not read, the pty full
    /// of `x` and 1000 bytes of `y` pending behind it, the client's close
    /// drops it; a new client's `z` then reaches the program (which echoes
    /// what it reads), and no `y` ever does.
    #[test]
    fn input_pending_at_a_disconnect_never_reaches_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = make_fifo(dir.path());
        let script = waits_then(&fifo, "exec /bin/cat");
        let mut host = stepped(dir, &["/bin/sh", "-c", &script], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        peer.output_until(&mut host, "ready");
        fill_the_pty(&mut host, &mut peer);
        let sent = host.client().taken_in() + 1000;
        peer.send(&mut host, &ClientMessage::Input(vec![b'y'; 1000]));
        host.step_until("the input to be taken in", |host| {
            host.client.as_ref().unwrap().taken_in() == sent
        });
        assert!(
            host.client()
                .input
                .back()
                .is_some_and(|segment| segment.data[0] == b'y' && segment.written == 0),
            "premise: the y input is pending, none of it written"
        );
        drop(peer);
        host.step_until("the closed client to be dropped", |host| {
            host.client.is_none()
        });
        let mut next = host.connect(false);
        let from = host.host.journal.current();
        next.send(&mut host, &ClientMessage::Read(from));
        release(&mut host, &fifo);
        next.send(&mut host, &ClientMessage::Input(b"z".to_vec()));
        let mut output = Vec::new();
        let deadline = Instant::now() + PATIENCE;
        while !output.contains(&b'z') {
            assert!(Instant::now() < deadline, "the new input never arrived");
            if let Some(HostMessage::Output { data, .. }) = next.recv(&mut host) {
                output.extend(data);
            }
        }
        assert!(
            !output.contains(&b'y'),
            "input of the closed client arrived"
        );
    }

    /// A `Shutdown` behind `Input` that the pending-input bound holds back
    /// still ends the host when the client closes.
    ///
    /// Why: the bound holds frames only for a client that stays connected.
    /// A closed connection's input is discarded anyway, and holding its
    /// `Shutdown` behind that input would lose it, leaving a host nobody
    /// will stop (SPEC.md, "Connections and version negotiation"). Spec:
    /// with a raw-mode program not reading and a pending cost of
    /// `MAX_PENDING_INPUT`, a 1-byte `Input` and a `Shutdown` wait in the
    /// host's buffer; once the client closes, the host ends within a few
    /// iterations, with `Shutdown`.
    #[test]
    fn a_shutdown_behind_held_input_still_ends_the_host_at_a_close() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = make_fifo(dir.path());
        let script = waits_then(&fifo, "exec /bin/sleep 1000");
        let mut host = stepped(dir, &["/bin/sh", "-c", &script], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        peer.output_until(&mut host, "ready");
        fill_the_pty(&mut host, &mut peer);
        top_up(&mut host, &mut peer, MAX_PENDING_INPUT);
        let held = [
            encode_client(&ClientMessage::Input(b"x".to_vec())),
            encode_client(&ClientMessage::Shutdown),
        ]
        .concat();
        peer.write(&mut host, &held);
        host.step_until("the frames to be held", |host| {
            host.client.as_ref().unwrap().buf.len() == held.len()
        });
        assert_eq!(host.host.ended, None, "premise: the Shutdown waits");
        drop(peer);
        host.wait_for_hangup();
        let mut iterations = 0;
        let ended = loop {
            iterations += 1;
            assert!(iterations <= 5, "the host did not end at the close");
            if let Some(ended) = host.host.step() {
                break ended;
            }
        };
        assert_eq!(ended, Ended::Shutdown);
    }

    /// A close is acted on in full even when what the client sent before it
    /// would take the host's buffer past its limit.
    ///
    /// Why: a hangup means the peer is gone, so the host acts on what it
    /// buffered and then reads up to one more buffer's worth of what the
    /// socket holds, beyond the buffer limit, and acts on that too; reading
    /// only up to the limit would leave a `Shutdown` in the socket of a
    /// client the host then no longer polls (its held input fills the buffer
    /// and nothing is queued for it), and the host would run on until the
    /// program reads again. Spec: with a raw-mode program not reading, a
    /// pending cost of `MAX_PENDING_INPUT` and held `Input`s filling the
    /// buffer to
    /// 100 000 bytes short of `MAX_FRAME + 4`, two more `Input`s (more than
    /// that) and a `Shutdown` written and the connection closed, the host
    /// ends within a few iterations.
    #[test]
    fn a_close_is_acted_on_beyond_the_buffer_limit() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = make_fifo(dir.path());
        let script = waits_then(&fifo, "exec /bin/sleep 1000");
        let mut host = stepped(dir, &["/bin/sh", "-c", &script], 1 << 20);
        let mut peer = host.connect(false);
        set_buffer(peer.stream.as_raw_fd(), libc::SO_SNDBUF, 1 << 20);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        peer.output_until(&mut host, "ready");
        fill_the_pty(&mut host, &mut peer);
        top_up(&mut host, &mut peer, MAX_PENDING_INPUT);
        let frame = encode_client(&ClientMessage::Input(vec![b'x'; MAX_INPUT]));
        let target = RECEIVE_LIMIT - 100_000;
        let stream: Vec<u8> = frame.repeat(target / frame.len() + 3);
        let (before, after) = stream.split_at(target);
        peer.write(&mut host, before);
        host.step_until("the buffer to hold what was sent", |host| {
            host.client.as_ref().unwrap().buf.len() == target
        });
        host.step_until("everything queued to be written", |host| {
            host.client.as_ref().unwrap().outgoing.is_empty()
        });
        let mut rest = after.to_vec();
        rest.extend(encode_client(&ClientMessage::Shutdown));
        assert!(
            rest.len() > RECEIVE_LIMIT - target,
            "premise: more than fits"
        );
        let written = peer.stream.write(&rest).unwrap();
        assert_eq!(written, rest.len(), "premise: the socket holds all of it");
        drop(peer);
        host.wait_for_hangup();
        let mut iterations = 0;
        let ended = loop {
            iterations += 1;
            assert!(iterations <= 5, "the host did not end at the close");
            if let Some(ended) = host.host.step() {
                break ended;
            }
        };
        assert_eq!(ended, Ended::Shutdown);
    }

    /// Frames the input bound held back in the host's buffer are acted on
    /// once the input drains, without the client sending anything more.
    ///
    /// Why: the host reads everything a socket holds at once, so when the
    /// bound is reached in the middle of a read, the rest of what the client
    /// sent is already in the host and no socket event will announce it
    /// again. A client waiting for those frames' replies would wait forever.
    /// Spec: with a pending cost of `MAX_PENDING_INPUT - INPUT_COST - 1`, a
    /// raw-mode program not reading, and one write holding a 1-byte `Input`
    /// (which fits exactly), a 4-byte `Input` and an `Info`, the last two
    /// wait in the host; once the program reads its terminal, the `Info` is
    /// answered and every `Input` acknowledged.
    #[test]
    fn frames_held_back_by_the_input_bound_are_handled_once_it_drains() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = make_fifo(dir.path());
        let script = format!(
            "stty raw -echo; echo ready; read go < '{}'; exec /bin/cat >/dev/null",
            fifo.display()
        );
        let mut host = stepped(dir, &["/bin/sh", "-c", &script], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        peer.output_until(&mut host, "ready");
        fill_the_pty(&mut host, &mut peer);
        top_up(&mut host, &mut peer, MAX_PENDING_INPUT - INPUT_COST - 1);
        let held = [
            encode_client(&ClientMessage::Input(b"tail".to_vec())),
            encode_client(&ClientMessage::Info),
        ]
        .concat();
        let mut bytes = encode_client(&ClientMessage::Input(b"x".to_vec()));
        bytes.extend(&held);
        peer.write(&mut host, &bytes);
        host.step_until("the frames to be held back", |host| {
            host.client.as_ref().unwrap().buf.len() == held.len()
        });
        let total = host.client().taken_in() + 4;
        release(&mut host, &fifo);
        peer.until(&mut host, |m| matches!(m, HostMessage::InfoReply(_)));
        peer.until(
            &mut host,
            |m| matches!(m, HostMessage::InputAck { written, .. } if *written == total),
        );
    }

    /// Input pending when the terminal closes is acknowledged as discarded,
    /// apart from what already reached the pty, and the host's pending cost
    /// returns to zero.
    ///
    /// Why: input that can no longer go anywhere is discarded and
    /// acknowledged as such (SPEC.md, "Input"): a client must not take a
    /// paste that never arrived for one that did, and must not wait for an
    /// acknowledgement that cannot come. A cost left above zero would make
    /// the input bound stop the host reading its client early, or for good.
    /// Spec: with a raw-mode program that does not read, the pty full and
    /// one partly written `Input` pending, the program's exit yields an ack
    /// whose written total counts the bytes that reached the pty and whose
    /// discarded total is the rest of that `Input`, and nothing is pending.
    /// One `Input`, not several: a miscount by division would come out at
    /// zero again over several.
    #[test]
    fn input_pending_when_the_terminal_closes_is_acknowledged() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = make_fifo(dir.path());
        let script = format!(
            "stty raw -echo; echo ready; read go < '{}'; exit 0",
            fifo.display()
        );
        let mut host = stepped(dir, &["/bin/sh", "-c", &script], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        peer.output_until(&mut host, "ready");
        // One Input at a time until one is stuck partway: then it is the
        // only one pending.
        let deadline = Instant::now() + PATIENCE;
        while host.client().input.is_empty() {
            assert!(Instant::now() < deadline, "the pty never filled");
            let sent = host.client().taken_in() + MAX_INPUT as u64;
            peer.send(&mut host, &ClientMessage::Input(vec![b'x'; MAX_INPUT]));
            host.step_until("the input to be taken in", |host| {
                host.client.as_ref().unwrap().taken_in() == sent
            });
            // Settled: unchanged over several iterations that tried to write.
            loop {
                let pending = host.client().input_cost;
                for _ in 0..3 {
                    host.step();
                }
                if host.client().input_cost == pending {
                    break;
                }
            }
        }
        assert!(
            host.client().input.len() == 1 && host.client().input[0].written > 0,
            "premise: exactly one Input is pending, and it is partly written"
        );
        let segment = &host.client().input[0];
        let unwritten = (segment.data.len() - segment.written) as u64;
        let written = host.client().taken_in() - unwritten;
        release(&mut host, &fifo);
        let ack = peer.until(
            &mut host,
            |m| matches!(m, HostMessage::InputAck { discarded, .. } if *discarded > 0),
        );
        assert_eq!(
            ack,
            HostMessage::InputAck {
                written,
                discarded: unwritten
            }
        );
        assert!(!host.host.pty_open);
        assert_eq!(host.client().input_cost, 0);
    }

    /// Poll waits indefinitely when nothing has a deadline, and otherwise
    /// until the nearest deadline, rounded up to the millisecond.
    ///
    /// Why: an idle host must cost nothing (it lives for weeks), but a
    /// connection waiting for its Hello must still be closed on time, and a
    /// deadline less than a millisecond away rounded down would poll with 0
    /// and spin until it passed. Without a signal pipe (as in these tests)
    /// the loop looks for the child's exit every 20 ms instead. Spec: 20 ms
    /// without a signal pipe; with one, -1 (no timeout) while idle and the
    /// Hello deadline, within `HELLO_TIMEOUT`, while a connection waits; a
    /// deadline about 5.5 ms away gives at least the remaining time rounded
    /// up; a paused listener's resumption counts as a deadline too.
    #[test]
    fn poll_waits_until_the_nearest_deadline_or_indefinitely() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        assert_eq!(host.host.poll_timeout(), 20);
        let (read, _write) = signal_pipe();
        host.host.signals = Some(read);
        assert_eq!(host.host.poll_timeout(), -1);
        let _waiting = UnixStream::connect(host.socket()).unwrap();
        host.step_until("the connection to be accepted", |host| {
            host.pending.len() == 1
        });
        let timeout = host.host.poll_timeout();
        assert!(
            (4000..=HELLO_TIMEOUT.as_millis() as libc::c_int).contains(&timeout),
            "timeout {timeout}"
        );
        let deadline = Instant::now() + Duration::from_micros(5500);
        host.host.pending[0].deadline = deadline;
        let timeout = host.host.poll_timeout();
        // What is left once the call returned, rounded up: the call saw at
        // least that much, so a correct rounding is never below it, and one
        // that truncated would be (unless the machine stalled for half a
        // millisecond, which only makes this check weaker, never wrong).
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(
            timeout as u128 >= left.as_micros().div_ceil(1000),
            "timeout {timeout} with {left:?} left"
        );
        host.host.pending.clear();
        host.host.accept_paused_until = Some(Instant::now() + Duration::from_millis(50));
        assert!((1..=50).contains(&host.host.poll_timeout()));
    }

    /// While accepting is paused (after `accept` ran out of descriptors),
    /// the listener is not polled, and once the pause is over it is again.
    ///
    /// Why: out of descriptors, a connection stays queued and the listener
    /// stays readable, so polling it at once would spin a CPU until a
    /// descriptor frees up (SPEC.md, "Resource bounds"); but the pause must
    /// end, or the host would stop taking connections for good. Running out
    /// of descriptors itself needs a resource limit on the test process, so
    /// the pause is set directly. Spec: with a pause an hour long, a
    /// connection waits unaccepted over several iterations; with the pause
    /// over, the next iteration accepts it and clears the pause.
    #[test]
    fn a_paused_listener_is_left_alone_until_the_pause_ends() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        host.host.accept_paused_until = Some(Instant::now() + Duration::from_secs(3600));
        let _waiting = UnixStream::connect(host.socket()).unwrap();
        for _ in 0..3 {
            host.step();
        }
        assert!(host.host.pending.is_empty(), "accepted while paused");
        host.host.accept_paused_until = Some(Instant::now());
        host.step_until("the connection to be accepted", |host| {
            host.pending.len() == 1
        });
        assert_eq!(host.host.accept_paused_until, None);
    }

    /// An idle iteration waits in poll instead of returning at once.
    ///
    /// Why: a host that polls for something that is always ready (a pty is
    /// almost always writable) spins a CPU for as long as it lives, while
    /// everything else still appears to work. Spec: with a silent program, a
    /// connected client and nothing queued either way, one iteration lasts
    /// at least most of its 20 ms timeout.
    #[test]
    fn an_idle_iteration_waits_in_poll() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let _peer = host.connect(false);
        assert!(host.client().input.is_empty() && host.client().outgoing.is_empty());
        let started = Instant::now();
        host.step();
        assert!(
            started.elapsed() >= Duration::from_millis(15),
            "an idle iteration returned after {:?}",
            started.elapsed()
        );
    }

    /// A wakeup on the signal pipe is drained in full.
    ///
    /// Why: a pipe left readable makes every later poll return at once, so
    /// the host would spin forever after its first signal. Spec: with 100
    /// bytes in the signal pipe (more than one read of the drain takes), one
    /// iteration leaves the pipe empty.
    #[test]
    fn a_signal_wakeup_is_drained_in_full() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let (read, write) = signal_pipe();
        host.host.signals = Some(read);
        // The write end stays open, as the real one does: a closed one would
        // leave the pipe readable at end of file.
        let mut write = std::fs::File::from(write);
        write.write_all(&[0u8; 100]).unwrap();
        host.step();
        let mut fd = libc::pollfd {
            fd: host.host.signals.as_ref().unwrap().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd, no wait.
        assert_eq!(
            unsafe { libc::poll(&mut fd, 1, 0) },
            0,
            "the pipe is still readable"
        );
    }

    /// `Shutdown` ends the host as soon as `Goodbye` is written, and nothing
    /// follows `Goodbye`.
    ///
    /// Why: Shutdown is how a client deletes a terminal, and it waits for the
    /// host to be gone; the one-second flush bound is only for a client that
    /// stops reading. Spec: with `Shutdown` and `Info` sent in one write, the
    /// host ends within two iterations of acting on `Shutdown`, well before
    /// the flush bound, and the client receives `Goodbye` and then end of
    /// stream, no `InfoReply`.
    #[test]
    fn shutdown_ends_the_host_once_goodbye_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        let mut bytes = encode_client(&ClientMessage::Shutdown);
        bytes.extend(encode_client(&ClientMessage::Info));
        peer.write(&mut host, &bytes);
        host.step_until("Shutdown to be acted on", |host| host.ended.is_some());
        let flush_bound = host
            .client()
            .flush_deadline
            .expect("Shutdown sets the flush bound");
        let mut iterations = 0;
        let ended = loop {
            iterations += 1;
            assert!(iterations <= 2, "the host did not end once Goodbye was out");
            if let Some(ended) = host.host.step() {
                break ended;
            }
        };
        assert_eq!(ended, Ended::Shutdown);
        assert!(Instant::now() < flush_bound);
        peer.wait_closed();
        assert_eq!(peer.take(), Some(HostMessage::Goodbye));
        assert_eq!(peer.take(), None, "nothing follows Goodbye");
    }

    /// Connections waiting for their `Hello` are accepted at most
    /// `MAX_PENDING_CONNECTIONS` per iteration, capped at that many by
    /// closing the oldest, kept while silent or partway through a `Hello`,
    /// and closed at their deadline.
    ///
    /// Why: a peer that connects and says nothing must not hold a
    /// descriptor for good, nor crowd out the client (SPEC.md, "Connections
    /// and version negotiation"), while a client whose `Hello` arrives late
    /// or in pieces must still get in. Accepting is bounded per iteration
    /// apart from the cap: peers connecting as fast as the host accepts
    /// could otherwise keep one iteration accepting for good, away from the
    /// pty, the client and a SIGTERM. Spec: of `MAX_PENDING_CONNECTIONS + 3`
    /// silent connections, one iteration accepts `MAX_PENDING_CONNECTIONS`
    /// and closes none; the next accepts the other 3 and closes the 3
    /// oldest; the rest wait across iterations; one that sent 3 bytes of its
    /// `Hello` keeps waiting and is welcomed when the rest arrives; the
    /// others are closed once their deadline has passed.
    #[test]
    fn connections_waiting_for_hello_are_capped_kept_and_expired() {
        const EXTRA: usize = 3;
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peers: Vec<Peer> = (0..MAX_PENDING_CONNECTIONS + EXTRA)
            .map(|_| Peer::raw(&host.socket()))
            .collect();
        host.step();
        assert_eq!(host.host.pending.len(), MAX_PENDING_CONNECTIONS);
        for peer in &mut peers[..EXTRA] {
            assert!(peer.fill(), "closed in the first iteration");
        }
        host.step();
        assert_eq!(host.host.pending.len(), MAX_PENDING_CONNECTIONS);
        for peer in &mut peers[..EXTRA] {
            peer.wait_closed();
        }
        for _ in 0..3 {
            host.step();
        }
        assert_eq!(
            host.host.pending.len(),
            MAX_PENDING_CONNECTIONS,
            "silence alone closes nothing before the deadline"
        );
        let hello = encode_client(&ClientMessage::Hello {
            version: 1,
            take_over: false,
        });
        peers[EXTRA].write(&mut host, &hello[..3]);
        host.step_until("the first piece to be read", |host| {
            host.pending.iter().any(|pending| pending.buf.len() == 3)
        });
        host.step();
        assert_eq!(host.host.pending.len(), MAX_PENDING_CONNECTIONS);
        peers[EXTRA].write(&mut host, &hello[3..]);
        assert_eq!(
            peers[EXTRA].recv(&mut host),
            Some(HostMessage::Welcome { version: 1 })
        );
        for pending in &mut host.host.pending {
            pending.deadline = Instant::now();
        }
        host.step();
        assert!(host.host.pending.is_empty());
        for peer in &mut peers[EXTRA + 1..] {
            peer.wait_closed();
        }
    }

    /// A connection that sends `MAX_BEFORE_HELLO` bytes without a whole
    /// frame is closed at once, before its deadline.
    ///
    /// Why: a `Hello` is 10 bytes in every version, so that much without a
    /// frame is something other than a `Hello`; reading on would let each
    /// waiting connection hold up to a maximum frame of the host's memory
    /// (SPEC.md, "Connections and version negotiation"). Spec: the start of
    /// a frame claiming 1000 bytes, followed by filler up to 64 bytes in
    /// all, gets the connection closed within a few iterations, no reply.
    #[test]
    fn a_connection_without_a_hello_where_one_fits_is_closed() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = Peer::raw(&host.socket());
        let mut bytes = 1000u32.to_be_bytes().to_vec();
        bytes.resize(MAX_BEFORE_HELLO, 0x10);
        peer.write(&mut host, &bytes);
        host.step_until("the connection to be accepted", |host| {
            host.pending.len() == 1
        });
        host.step_until("the connection to be closed", |host| {
            host.pending.is_empty()
        });
        peer.wait_closed();
        assert!(peer.buf.is_empty(), "no reply");
    }

    /// A second `Read` on a connection is refused with `Error` 2 and the
    /// connection closed.
    ///
    /// Why: version 1 has one stream per connection (SPEC.md, "Client
    /// messages"). A second stream could not be told apart from the first
    /// one's records already in the socket, so a client re-reads by
    /// reconnecting. Spec: `Read` and `Read` in one write, to a host whose
    /// program prints nothing, yield exactly `Error` with code `NotAllowed`
    /// and then end of stream; the host keeps serving the next client.
    #[test]
    fn a_second_read_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        let read = encode_client(&ClientMessage::Read(Position::default()));
        peer.write(&mut host, &[read.clone(), read].concat());
        let mut messages = Vec::new();
        while let Some(message) = peer.recv(&mut host) {
            messages.push(message);
        }
        assert!(
            matches!(
                messages.as_slice(),
                [HostMessage::Error {
                    code: ErrorCode::NotAllowed,
                    ..
                }]
            ),
            "{messages:?}"
        );
        let _next = host.connect(false);
    }

    /// The queue for a stream stays between `OUTGOING_HIGH_WATER` and one
    /// frame more while the client does not read, and a client that then
    /// reads still gets every byte, in order.
    ///
    /// Why: the outgoing queue is part of the host's memory bound, which
    /// must not grow with the journal, and a slow client must lose nothing
    /// (SPEC.md, "Reading"). Spec: with 4 MiB journaled in 100-byte pieces
    /// (a resize after each, so that no `Output` frame is longer than 100
    /// bytes) and a client that does not read, the queue's unsent bytes
    /// stay within those bounds over five iterations; reading it all then
    /// yields contiguous output up to the journal's end, each resize at the
    /// end of its piece.
    #[test]
    fn the_stream_queue_stays_at_the_high_water_mark() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 8 << 20);
        // Output the client cannot take at once, journaled directly as if
        // the program had printed it. The journal packs adjacent output
        // into large blocks; a resize between pieces is what keeps each
        // `Output` frame small.
        let size = host.host.journal.size();
        for _ in 0..(4 << 20) / 100 {
            host.host.journal.push_output(&[b'a'; 100]);
            host.host.journal.push_resize(size);
        }
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        for _ in 0..5 {
            host.step();
            let unsent = host.unsent();
            assert!(unsent >= OUTGOING_HIGH_WATER, "only {unsent} queued");
            assert!(
                unsent <= OUTGOING_HIGH_WATER + MAX_OUTPUT_CHUNK + 64,
                "{unsent} queued"
            );
        }
        let end = host.host.journal.end();
        let mut next = 0;
        let deadline = Instant::now() + PATIENCE;
        while next < end {
            assert!(Instant::now() < deadline, "the stream stalled at {next}");
            host.step();
            peer.fill();
            while let Some(message) = peer.take() {
                match message {
                    HostMessage::Output { offset, data } => {
                        assert_eq!(offset, next, "contiguous output");
                        next += data.len() as u64;
                    }
                    HostMessage::Resized { offset, .. } => {
                        assert_eq!(offset, next, "each resize follows its piece");
                    }
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
        assert_eq!(next, end);
    }

    /// Replies count against the same budget as the stream: while more than
    /// `OUTGOING_HIGH_WATER` waits unsent, the host acts on none of the
    /// client's frames and reads them only until its buffer holds one
    /// maximum frame; once the client reads, every one is answered.
    ///
    /// Why: a client that keeps sending `Info` without reading its replies
    /// (a wedged reader beside a working poller is enough) would otherwise
    /// grow the host by one reply per request, without bound (SPEC.md,
    /// "Resource bounds"). Spec: with a client that sends `Info`s and does
    /// not read, the unsent bytes never exceed `OUTGOING_HIGH_WATER` plus one
    /// `InfoReply`; once they exceed the mark, 1 MiB `Snapshot`s sent after
    /// (rejected when acted on) are buffered unanswered until the buffer
    /// holds `MAX_FRAME + 4` bytes and the socket is polled for writing
    /// only; when the client then reads, every `Info` and `Snapshot` sent is
    /// answered.
    #[test]
    fn replies_count_against_the_outgoing_budget() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        let reply = encode_host(&HostMessage::InfoReply(Info {
            host_pid: 0,
            child_pid: 0,
            child_exit: None,
            size: Size { cols: 1, rows: 1 },
            initial_size: Size { cols: 1, rows: 1 },
            snapshot: Position::default(),
            tail_start: 0,
            end: 0,
            resizes: 0,
            gap: false,
            delivered_end: 0,
            tag: host.host.setup.tag.clone(),
        }))
        .len();
        let infos = encode_client(&ClientMessage::Info).repeat(1000);
        let mut sent_infos = 0;
        let deadline = Instant::now() + PATIENCE;
        while host.unsent() <= OUTGOING_HIGH_WATER {
            assert!(Instant::now() < deadline, "the replies never piled up");
            peer.write(&mut host, &infos);
            sent_infos += 1000;
            host.step();
            assert!(
                host.unsent() <= OUTGOING_HIGH_WATER + reply,
                "{} unsent",
                host.unsent()
            );
        }
        let unsent = host.unsent();
        let snapshot = encode_client(&ClientMessage::Snapshot {
            position: Position {
                offset: 1 << 40,
                resizes: 0,
            },
            blob: vec![0; 1 << 20],
        });
        let mut backlog: Vec<u8> = Vec::new();
        let mut sent_snapshots = 0;
        while host.client().buf.len() < MAX_FRAME + 4 {
            assert!(Instant::now() < deadline, "the host's buffer never filled");
            if backlog.len() < snapshot.len() {
                backlog.extend(&snapshot);
                sent_snapshots += 1;
            }
            match peer.stream.write(&backlog) {
                Ok(n) => {
                    backlog.drain(..n);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("writing to the host: {error}"),
            }
            host.step();
            assert!(host.unsent() <= unsent, "a reply was queued while over");
        }
        assert_eq!(host.client().buf.len(), MAX_FRAME + 4, "one maximum frame");
        assert_eq!(client_events(host.client()), libc::POLLOUT);
        let (mut infos_answered, mut snapshots_answered) = (0, 0);
        while infos_answered < sent_infos || snapshots_answered < sent_snapshots {
            assert!(Instant::now() < deadline, "not everything was answered");
            if !backlog.is_empty() {
                match peer.stream.write(&backlog) {
                    Ok(n) => {
                        backlog.drain(..n);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(error) => panic!("writing to the host: {error}"),
                }
            }
            host.step();
            peer.fill();
            while let Some(message) = peer.take() {
                match message {
                    HostMessage::InfoReply(_) => infos_answered += 1,
                    HostMessage::SnapshotRejected { .. } => snapshots_answered += 1,
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
        assert_eq!(
            (infos_answered, snapshots_answered),
            (sent_infos, sent_snapshots)
        );
    }

    /// A stepped host whose client has a 2 MiB snapshot accepted and a small
    /// send buffer on the host's side, so that a snapshot record is written
    /// across many iterations. Returns the host, the client and the blob.
    fn with_a_large_snapshot() -> (Stepped, Peer, Vec<u8>) {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        let blob: Vec<u8> = (0..2 << 20).map(|i: u32| (i % 251) as u8).collect();
        peer.send(
            &mut host,
            &ClientMessage::Snapshot {
                position: Position::default(),
                blob: blob.clone(),
            },
        );
        assert_eq!(
            peer.recv(&mut host),
            Some(HostMessage::SnapshotAccepted(Position::default()))
        );
        // A small send buffer makes the host write in pieces of a few KiB,
        // so its queue is seen at every stage.
        set_buffer(host.client().fd.as_raw_fd(), libc::SO_SNDBUF, 16 * 1024);
        (host, peer, blob)
    }

    /// Set a socket's send or receive buffer size (`option`), as far as the
    /// system allows.
    fn set_buffer(fd: RawFd, option: libc::c_int, size: libc::c_int) {
        // SAFETY: setting an int socket option on a socket the test owns.
        let result = unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                option,
                (&size as *const libc::c_int).cast(),
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            )
        };
        assert_eq!(result, 0, "setsockopt: {}", io::Error::last_os_error());
    }

    /// Start the stream from (0, 0), which begins with the snapshot record,
    /// and step until that record is partly written.
    fn start_the_record(host: &mut Stepped, peer: &mut Peer, blob: &[u8]) {
        peer.send(host, &ClientMessage::Read(Position::default()));
        host.step_until("the record to be partly written", |host| {
            host.client
                .as_ref()
                .unwrap()
                .outgoing
                .front()
                .is_some_and(|out| out.written > 0 && out.bytes.len() > blob.len())
        });
    }

    /// A record too large for the socket is written across iterations, and
    /// while it is written the stream behind it is kept queued up to
    /// `OUTGOING_HIGH_WATER`.
    ///
    /// Why: a snapshot is far larger than a socket buffer, and the stream
    /// behind it must neither stall nor pile up without bound. Spec: with a
    /// 2 MiB snapshot accepted, 300 KB of output after it, and a small
    /// socket buffer on the host's side, a `Read` from (0, 0) yields the
    /// snapshot record intact and then the output, with the queue holding at
    /// least `OUTGOING_HIGH_WATER` unsent bytes whenever the stream has more.
    #[test]
    fn a_record_too_large_for_the_socket_is_written_across_iterations() {
        let (mut host, mut peer, blob) = with_a_large_snapshot();
        host.host.journal.push_output(&vec![b'z'; 300_000]);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        let mut record = None;
        let mut output = 0;
        let deadline = Instant::now() + PATIENCE;
        while output < 300_000 {
            assert!(Instant::now() < deadline, "the stream stalled");
            host.step();
            assert!(
                host.unsent() >= OUTGOING_HIGH_WATER || host.caught_up(),
                "only {} queued while the stream has more",
                host.unsent()
            );
            peer.fill();
            while let Some(message) = peer.take() {
                match message {
                    HostMessage::SnapshotRecord { blob, .. } => record = Some(blob),
                    HostMessage::Output { data, .. } => output += data.len(),
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
        assert!(
            record.as_ref() == Some(&blob),
            "the snapshot record arrived intact"
        );
    }

    /// A refusal finishes a partly written record before its `Error`.
    ///
    /// Why: frames share one byte stream, so a frame cut short breaks the
    /// framing of everything after it, the `Error` included. Spec: while
    /// the snapshot record is partly written, a second `Read` (acted on once
    /// the client has read enough of the record for the reply budget to let
    /// it through, and still mid-record then) yields that record intact,
    /// then `Error` (`NotAllowed`), then end of stream; at the refusal,
    /// exactly the record's rest and the `Error` wait to be sent.
    #[test]
    fn a_refusal_finishes_a_partly_written_record_first() {
        let (mut host, mut peer, blob) = with_a_large_snapshot();
        start_the_record(&mut host, &mut peer, &blob);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        // The second Read waits for the reply budget, so the client has to
        // read for it to be acted on at all.
        let deadline = Instant::now() + PATIENCE;
        while !host.client().closing {
            assert!(Instant::now() < deadline, "the Read was never refused");
            host.step();
            peer.fill();
        }
        // The record's rest and the Error are what is left to send, and the
        // running total says so (`unsent` checks it against the queue).
        assert_eq!(host.client().outgoing.len(), 2);
        host.unsent();
        let mut messages = Vec::new();
        while let Some(message) = peer.recv(&mut host) {
            messages.push(message);
        }
        assert_eq!(messages.len(), 2, "{} messages", messages.len());
        assert_eq!(
            messages[0],
            HostMessage::SnapshotRecord {
                position: Position::default(),
                blob,
            }
        );
        assert!(matches!(
            messages[1],
            HostMessage::Error {
                code: ErrorCode::NotAllowed,
                ..
            }
        ));
    }

    /// A takeover closes an old connection that is in the middle of a frame
    /// where it is, without finishing the frame.
    ///
    /// Why: takeover exists for a stuck old connection, which is the
    /// likeliest to be mid-frame, so the newcomer must not wait for that
    /// frame; the old client takes the end of its stream, even inside a
    /// frame, as possible displacement (SPEC.md, "Connections and version
    /// negotiation"). Spec: while the snapshot record is partly written, a
    /// newcomer with `take_over` is welcomed, and the old connection
    /// receives a strict prefix of the record's frame and then end of
    /// stream.
    #[test]
    fn a_takeover_never_cuts_into_a_partly_written_record() {
        let (mut host, mut old, blob) = with_a_large_snapshot();
        start_the_record(&mut host, &mut old, &blob);
        assert!(
            old.buf.is_empty(),
            "premise: nothing of the record read yet"
        );
        let _newcomer = host.connect(true);
        old.wait_closed();
        let frame = encode_host(&HostMessage::SnapshotRecord {
            position: Position::default(),
            blob,
        });
        assert!(
            old.buf.len() < frame.len() && frame.starts_with(&old.buf),
            "the old connection got {} bytes, not a prefix of the record",
            old.buf.len()
        );
    }

    /// A takeover drops what is queued for the old client and sends it
    /// nothing more: its stream just ends.
    ///
    /// Why: version 1 has no displacement notice (kind 0x82 is reserved and
    /// never sent), so the end of stream is the old client's whole notice,
    /// and nothing queued for it may still reach it after another client
    /// took the terminal over (SPEC.md, "Connections and version
    /// negotiation"). Spec: with a newcomer waiting, the old client's `Info`
    /// and the newcomer's `Hello` with `take_over`, acted on in one
    /// iteration, leave the `InfoReply` queued and unstarted; the old
    /// connection then receives no frame at all before its end of stream,
    /// and the newcomer `Welcome`.
    #[test]
    fn a_takeover_sends_the_old_client_nothing_more() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut old = host.connect(false);
        let mut newcomer = Peer::raw(&host.socket());
        host.step_until("the newcomer to be accepted", |host| {
            host.pending.len() == 1
        });
        newcomer
            .stream
            .write_all(&encode_client(&ClientMessage::Hello {
                version: 1,
                take_over: true,
            }))
            .unwrap();
        old.stream
            .write_all(&encode_client(&ClientMessage::Info))
            .unwrap();
        host.step();
        assert!(
            host.host.pending.is_empty(),
            "premise: the takeover came in the same iteration as the Info"
        );
        old.wait_closed();
        let mut messages = Vec::new();
        while let Some(message) = old.take() {
            messages.push(message);
        }
        assert_eq!(messages, vec![], "the old client got nothing more");
        assert_eq!(
            newcomer.recv(&mut host),
            Some(HostMessage::Welcome { version: 1 })
        );
    }

    /// The last reading of a connection whose write failed stops after
    /// `RECEIVE_LIMIT` bytes, even while its peer keeps writing.
    ///
    /// Why: a peer that shut down only its reading side makes every write
    /// to it fail while it can still send without end; reading it to the
    /// end before dropping it would hold the loop, and with it the program's
    /// output and every other client, for as long as it writes. Spec
    /// (Linux, where that shutdown makes `send` fail with EPIPE): with an
    /// `InfoReply` queued, the peer's reading side shut down and a thread
    /// writing `Info`s to it without pause, one iteration drops the client.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_last_reading_of_a_failed_connection_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Info);
        host.step();
        assert!(
            host.queued()
                .iter()
                .any(|m| matches!(m, HostMessage::InfoReply(_))),
            "premise: the reply is queued, not written"
        );
        peer.stream.shutdown(std::net::Shutdown::Read).unwrap();
        let mut writer = peer.stream.try_clone().unwrap();
        writer.set_nonblocking(false).unwrap();
        let frames = encode_client(&ClientMessage::Info).repeat(10_000);
        // Writes until the host closes its end, which fails the write.
        let thread = std::thread::spawn(move || while writer.write_all(&frames).is_ok() {});
        host.step();
        assert!(host.host.client.is_none(), "the client was dropped");
        drop(peer);
        thread.join().unwrap();
    }

    /// A `Shutdown` the client sent before closing ends the host even when a
    /// reply was still queued for it, so that the write to the closed
    /// connection fails first.
    ///
    /// Why: a client tearing a terminal down may send `Shutdown` and close
    /// without waiting (SPEC.md, "Connections and version negotiation").
    /// Poll then reports the socket writable and hung up at once, and a host
    /// that dropped the client on the failed write would never read the
    /// `Shutdown` and run on for good. Spec: with an `InfoReply` queued and
    /// unwritten, a `Shutdown` written and the connection closed, the next
    /// iteration drops the client, and the host ends in the one after.
    #[test]
    fn a_shutdown_before_a_close_is_acted_on_although_a_reply_waits() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Info);
        host.step();
        assert!(
            host.queued()
                .iter()
                .any(|m| matches!(m, HostMessage::InfoReply(_))),
            "premise: the reply is queued, not written"
        );
        peer.send(&mut host, &ClientMessage::Shutdown);
        drop(peer);
        host.wait_for_hangup();
        host.step();
        assert!(host.host.client.is_none(), "the client was dropped");
        assert_eq!(host.host.step(), Some(Ended::Shutdown));
    }

    /// A failed write closes the client at once, and what it sent before is
    /// still acted on.
    ///
    /// Why: a peer that shut down only its reading side makes every write
    /// fail while poll reports no hangup, so the failed write is the only
    /// sign the connection is gone; a host that kept such a client would
    /// spin on a socket that is always writable, and keep the next client
    /// out with `Busy`. Spec (Linux, where that shutdown makes `send` fail
    /// with EPIPE and poll report the socket writable only): with an
    /// `InfoReply` queued, a `Shutdown` written and the peer's reading side
    /// shut down, one iteration drops the client and acts on the `Shutdown`.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_failed_write_closes_the_client_and_still_acts_on_what_it_sent() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Info);
        host.step();
        assert!(
            host.queued()
                .iter()
                .any(|m| matches!(m, HostMessage::InfoReply(_))),
            "premise: the reply is queued, not written"
        );
        peer.send(&mut host, &ClientMessage::Shutdown);
        peer.stream.shutdown(std::net::Shutdown::Read).unwrap();
        host.step();
        assert!(host.host.client.is_none(), "the client was dropped");
        assert_eq!(host.host.ended, Some(Ended::Shutdown));
    }

    /// A write to the pty that fails for good discards the pending input but
    /// leaves the output open until a read finds its end.
    ///
    /// Why: on Linux a write to the master fails as soon as nothing holds the
    /// slave, while the master can still hold the program's last output;
    /// ending the output on the failed write would lose that output and put
    /// the exit record before it. Spec: with a stand-in master that reads
    /// like a 4 MiB file but refuses every write (it is opened read-only),
    /// an `Input` is acknowledged as discarded, none of it written, while
    /// the output stays open, and every byte of the file is journaled before
    /// the end of output.
    #[test]
    fn a_failed_pty_write_discards_input_but_not_the_output() {
        let dir = tempfile::tempdir().unwrap();
        let len = 4 * READ_BUDGET;
        drop(output_file(dir.path(), len));
        let master: OwnedFd = std::fs::File::open(dir.path().join("output"))
            .unwrap()
            .into();
        let mut host = stepped_on(dir, master, idle_child(), 1 << 20);
        let mut peer = Peer::raw(&host.socket());
        let bytes = [
            encode_client(&ClientMessage::Hello {
                version: 1,
                take_over: false,
            }),
            encode_client(&ClientMessage::Input(b"x".to_vec())),
        ]
        .concat();
        peer.stream.write_all(&bytes).unwrap();
        host.step_until("the input to be taken in", |host| {
            host.client
                .as_ref()
                .is_some_and(|client| client.taken_in() == 1)
        });
        assert!(host.client().input.is_empty(), "the input was discarded");
        assert!(host.host.pty_open, "the output is still open");
        assert!(
            host.host.journal.end() < len as u64,
            "premise: output was left to read"
        );
        assert_eq!(
            peer.until(&mut host, |m| matches!(m, HostMessage::InputAck { .. })),
            HostMessage::InputAck {
                written: 0,
                discarded: 1
            }
        );
        host.step_until("the end of output", |host| !host.pty_open);
        assert_eq!(host.host.journal.end(), len as u64);
    }

    /// During the flush after `Shutdown`, the host takes no new connection
    /// and closes those waiting for their `Hello`, and it ends at the flush
    /// bound even though `Goodbye` never went out.
    ///
    /// Why: a newcomer taking over in that second would be served by a host
    /// about to exit, its input lost unacknowledged; and a client that stops
    /// reading must not keep the host alive (SPEC.md, "Ending"). Spec: with
    /// the client's socket full (it does not read) and a connection waiting
    /// for its `Hello`, once `Shutdown` is acted on the waiting connection is
    /// closed and a newcomer's `Hello` goes unanswered over several
    /// iterations; once the flush bound has passed, the host ends.
    #[test]
    fn the_shutdown_flush_serves_nobody_new() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        set_buffer(host.client().fd.as_raw_fd(), libc::SO_SNDBUF, 16 * 1024);
        let mut waiting = Peer::raw(&host.socket());
        host.step_until("the connection to be accepted", |host| {
            host.pending.len() == 1
        });
        // Less output than the reply budget, so that the Shutdown after it
        // is acted on, but far more than the socket takes.
        host.host.journal.push_output(&vec![b'z'; 200_000]);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        host.step_until("the socket to fill", |host| {
            host.client
                .as_ref()
                .unwrap()
                .outgoing
                .front()
                .is_some_and(|out| out.written > 0)
        });
        assert!(
            host.unsent() <= OUTGOING_HIGH_WATER,
            "premise: within budget"
        );
        peer.send(&mut host, &ClientMessage::Shutdown);
        host.step_until("Shutdown to be acted on", |host| host.ended.is_some());
        waiting.wait_closed();
        let mut newcomer = Peer::raw(&host.socket());
        newcomer.write(
            &mut host,
            &encode_client(&ClientMessage::Hello {
                version: 1,
                take_over: true,
            }),
        );
        for _ in 0..3 {
            assert_eq!(host.host.step(), None, "premise: Goodbye is not out");
        }
        assert!(
            newcomer.fill() && newcomer.buf.is_empty(),
            "the newcomer was answered"
        );
        host.host.client.as_mut().unwrap().flush_deadline = Some(Instant::now());
        assert_eq!(host.host.step(), Some(Ended::Shutdown));
    }

    /// One `receive` call reads until the buffer holds its limit and no
    /// further, however much the socket has, and reads nothing into a
    /// buffer already at its limit.
    ///
    /// Why: the limit is the host's bound on a client's unacted frames and
    /// on how long one call may read from a peer that keeps writing; a call
    /// that stepped over it would let a client grow the host past one
    /// frame, and one that stopped short would stall a frame that fits.
    /// Spec: with more than one read's worth waiting (64 KiB), a call with a
    /// limit 100 bytes short of it reads exactly to the limit and reports
    /// `Full`; a call with room reads the remaining 100 and reports
    /// `Drained`; with the peer closed, a call at the limit still reports
    /// `Full` without reading, and one with room reports `Ended`; and with
    /// the buffer prefilled to `RECEIVE_LIMIT - 100`, a call reads exactly
    /// 100 bytes.
    #[test]
    fn one_receive_call_reads_up_to_its_limit_and_no_further() {
        // A pair with room for more than one read: macOS gives a unix
        // socket only a few KiB by default.
        let pair = || {
            let (ours, theirs) = UnixStream::pair().unwrap();
            for stream in [&ours, &theirs] {
                stream.set_nonblocking(true).unwrap();
                set_buffer(stream.as_raw_fd(), libc::SO_SNDBUF, 1 << 20);
                set_buffer(stream.as_raw_fd(), libc::SO_RCVBUF, 1 << 20);
            }
            (ours, theirs)
        };
        let (ours, theirs) = pair();
        let fill = |mut writer: &UnixStream| {
            let mut total = 0;
            loop {
                match writer.write(&[7u8; 4096]) {
                    Ok(n) => total += n,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => return total,
                    Err(error) => panic!("{error}"),
                }
            }
        };
        let available = fill(&theirs);
        assert!(available > 64 * 1024, "premise: only {available} bytes fit");
        let mut buf = Vec::new();
        let limit = available - 100;
        assert_eq!(receive(ours.as_raw_fd(), &mut buf, limit), Received::Full);
        assert_eq!(buf.len(), limit);
        assert_eq!(
            receive(ours.as_raw_fd(), &mut buf, 2 * available),
            Received::Drained
        );
        assert_eq!(buf.len(), available);
        fill(&theirs);
        // A shutdown, not a drop, ends the stream: a child another test
        // forks holds a copy of every descriptor until it execs.
        theirs.shutdown(std::net::Shutdown::Write).unwrap();
        let at = buf.len();
        assert_eq!(receive(ours.as_raw_fd(), &mut buf, at), Received::Full);
        assert_eq!(buf.len(), at, "nothing read at the limit");
        assert_eq!(
            receive(ours.as_raw_fd(), &mut buf, usize::MAX),
            Received::Ended
        );

        let (ours, theirs) = pair();
        fill(&theirs);
        let mut buf = vec![0u8; RECEIVE_LIMIT - 100];
        assert_eq!(
            receive(ours.as_raw_fd(), &mut buf, RECEIVE_LIMIT),
            Received::Full
        );
        assert_eq!(buf.len(), RECEIVE_LIMIT);
    }

    /// The signal handler wakes the loop through the signal pipe, and
    /// leaves `errno` as it found it, even when its write fails.
    ///
    /// Why: that byte is what wakes a loop that checked for signals and is
    /// about to block in poll; a SIGCHLD or SIGTERM landing in that window
    /// would otherwise wait for an unrelated event, which for an idle host
    /// may be never. And the handler can run between any system call of the
    /// loop and its look at `errno`, which a failed write of its own would
    /// change under it. Spec: with a pipe's write end stored as the signal
    /// pipe, calling the handler for SIGCHLD leaves one byte in the pipe;
    /// with a full pipe stored instead (a write to it fails with EAGAIN),
    /// `errno` set to ENOENT before the call is still ENOENT after it. (The
    /// handler is called directly: no handler is installed in the test
    /// process, and nothing else here touches the signal pipe.)
    #[test]
    fn the_signal_handler_wakes_the_loop_and_keeps_errno() {
        let (read, write) = signal_pipe();
        SIGNAL_PIPE.store(write.as_raw_fd(), std::sync::atomic::Ordering::SeqCst);
        on_signal(libc::SIGCHLD);
        SIGNAL_PIPE.store(-1, std::sync::atomic::Ordering::SeqCst);
        let mut buf = [0u8; 8];
        // SAFETY: reading into a valid buffer from a non-blocking pipe.
        let n = unsafe { libc::read(read.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
        assert_eq!(n, 1);

        let (_full_read, full) = signal_pipe();
        let chunk = [0u8; 4096];
        // SAFETY: writing from a valid buffer to a non-blocking pipe, and
        // reading this thread's errno.
        let failure = loop {
            if unsafe { libc::write(full.as_raw_fd(), chunk.as_ptr().cast(), chunk.len()) } < 0 {
                break unsafe { *crate::spawn::errno_location() };
            }
        };
        assert_eq!(failure, libc::EAGAIN, "premise: the pipe is full");
        SIGNAL_PIPE.store(full.as_raw_fd(), std::sync::atomic::Ordering::SeqCst);
        // SAFETY: setting and reading this thread's errno.
        let after = unsafe {
            *crate::spawn::errno_location() = libc::ENOENT;
            on_signal(libc::SIGCHLD);
            *crate::spawn::errno_location()
        };
        SIGNAL_PIPE.store(-1, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(after, libc::ENOENT, "the handler changed errno");
    }

    /// A closed connection's `Input`s are dropped as they are decoded, while
    /// the control frames after them still act.
    ///
    /// Why: a closed connection's frames bypass the pending-input bound, so
    /// that a `Shutdown` behind held input is never lost; queueing its
    /// `Input`s on the way would let a client that filled its buffer and
    /// then closed make the host hold far more than `MAX_PENDING_INPUT`
    /// (a buffer of one-byte `Input`s is over a million segments), for
    /// input that can never be written (SPEC.md, "Input"). The check comes
    /// before any `Shutdown`, which discards pending input itself and would
    /// hide a queue built up on the way. Spec: with the client marked gone
    /// and its buffer holding 20 `MAX_INPUT` `Input`s (well beyond the
    /// bound) and an `Info`, acting on the buffer consumes all of it, leaves
    /// no input pending, at no cost, and queues no reply; a `Shutdown` after
    /// that still ends the host.
    #[test]
    fn a_closed_clients_input_is_dropped_as_it_is_decoded() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let _peer = host.connect(false);
        let mut frames: Vec<u8> =
            encode_client(&ClientMessage::Input(vec![b'x'; MAX_INPUT])).repeat(20);
        frames.extend(encode_client(&ClientMessage::Info));
        let client = host.host.client.as_mut().unwrap();
        client.gone = true;
        client.buf = frames;
        host.host.act_on_buffered();
        let client = host.client();
        assert!(client.buf.is_empty(), "every frame was acted on");
        assert!(
            client.input.is_empty(),
            "a closed client's input was queued"
        );
        assert_eq!(client.input_cost, 0);
        assert!(client.outgoing.is_empty(), "a reply was queued");
        host.host.client.as_mut().unwrap().buf = encode_client(&ClientMessage::Shutdown);
        host.host.act_on_buffered();
        assert_eq!(host.host.ended, Some(Ended::Shutdown));
    }

    /// `Shutdown` drops the frames queued and not started, so `Goodbye` is
    /// the next frame the client gets.
    ///
    /// Why: the host ends once `Goodbye` is written, or after
    /// `GOODBYE_FLUSH` without it; up to `OUTGOING_HIGH_WATER` of stream
    /// records and replies ahead of it would use that second up behind a
    /// slow client, and the client is tearing the terminal down anyway
    /// (SPEC.md, "Ending"). A refusal drops them the same way before its
    /// `Error`. Spec: with `Info` and `Shutdown` sent in one write, so that
    /// the `InfoReply` is queued and unstarted when the `Shutdown` is acted
    /// on, exactly `Goodbye` is left queued, and the client receives
    /// `Goodbye` and then end of stream.
    #[test]
    fn shutdown_drops_what_was_not_started() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        let bytes = [
            encode_client(&ClientMessage::Info),
            encode_client(&ClientMessage::Shutdown),
        ]
        .concat();
        peer.write(&mut host, &bytes);
        host.step_until("Shutdown to be acted on", |host| host.ended.is_some());
        assert_eq!(host.queued(), vec![HostMessage::Goodbye]);
        host.unsent();
        let deadline = Instant::now() + PATIENCE;
        while host.host.step().is_none() {
            assert!(Instant::now() < deadline, "the host never ended");
        }
        peer.wait_closed();
        let mut messages = Vec::new();
        while let Some(message) = peer.take() {
            messages.push(message);
        }
        assert_eq!(messages, vec![HostMessage::Goodbye]);
    }

    /// A client that shuts down only its writing side counts as closed:
    /// what it sent is acted on, and nothing more is sent to it.
    ///
    /// Why: half-closing after the last request is a common client idiom,
    /// and SPEC.md ("Connections and version negotiation") says what it
    /// gets, so a client does not wait for a `Goodbye` that is never sent.
    /// On Linux the host sees this as a read of 0, not a hangup. Spec: with
    /// an `Info` and a `Shutdown` sent and the client's writing side shut
    /// down, the host drops the client and ends with `Shutdown`, and the
    /// client receives no frame at all before its end of stream.
    #[test]
    fn a_half_closed_client_counts_as_closed() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        let bytes = [
            encode_client(&ClientMessage::Info),
            encode_client(&ClientMessage::Shutdown),
        ]
        .concat();
        peer.write(&mut host, &bytes);
        peer.stream.shutdown(std::net::Shutdown::Write).unwrap();
        let deadline = Instant::now() + PATIENCE;
        let ended = loop {
            assert!(Instant::now() < deadline, "the host never ended");
            if let Some(ended) = host.host.step() {
                break ended;
            }
        };
        assert_eq!(ended, Ended::Shutdown);
        peer.wait_closed();
        assert!(
            peer.buf.is_empty(),
            "a half-closed client was sent {:?}",
            peer.take()
        );
    }

    /// A burst of `Input`s written together earns one acknowledgement, not
    /// one per `Input`.
    ///
    /// Why: acknowledgements are not budgeted like replies, so one per
    /// `Input` would let a client pipelining tiny `Input`s queue one frame
    /// per `Input` at once (SPEC.md, "Input"). This pins that a write of
    /// many resolved `Input`s acknowledges them once;
    /// `an_acknowledgement_updates_one_waiting_at_the_back` pins how a
    /// later acknowledgement updates one still waiting. Spec: 1000 one-byte
    /// `Input`s sent in one write to `sleep` (which never reads; the pty, in
    /// its default canonical mode, takes all 1000 bytes) are written, and the
    /// acknowledgements left unsent then are exactly one, with 1000 written
    /// and none discarded.
    #[test]
    fn a_burst_of_input_is_acknowledged_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        peer.write(
            &mut host,
            &encode_client(&ClientMessage::Input(b"x".to_vec())).repeat(1000),
        );
        host.step_until("the input to be written", |host| {
            host.client.as_ref().unwrap().written == 1000
        });
        assert_eq!(
            host.queued(),
            vec![HostMessage::InputAck {
                written: 1000,
                discarded: 0
            }]
        );
    }

    /// A client with no host around it, for tests of the queueing helpers.
    fn bare_client() -> Client {
        let (ours, _theirs) = UnixStream::pair().unwrap();
        Client {
            fd: ours.into(),
            version: 1,
            buf: Vec::new(),
            outgoing: VecDeque::new(),
            unsent: 0,
            cursor: None,
            input: VecDeque::new(),
            input_cost: 0,
            written: 0,
            discarded: 0,
            closing: false,
            flush_deadline: None,
            gone: false,
        }
    }

    /// An acknowledgement updates the one last in the queue while that one
    /// has not started, and otherwise queues a new one.
    ///
    /// Why: this is what bounds queued acknowledgements by the other frames
    /// between them (at most two more than those: SPEC.md, "Resource
    /// bounds"); updating one that has started would corrupt the frame on
    /// the wire, and updating one with a frame behind it would reorder the
    /// stream of replies. Spec: on an empty queue, an acknowledgement is
    /// queued; a second, with new totals, replaces it (one frame, the new
    /// totals, the unsent count unchanged); once it is partly written, the
    /// next is queued right behind it (the one case of two in a row), and
    /// once a reply is queued behind that, the next goes after the reply.
    #[test]
    fn an_acknowledgement_updates_one_waiting_at_the_back() {
        let mut client = bare_client();
        let acks = |client: &Client| -> Vec<(u64, u64)> {
            client
                .outgoing
                .iter()
                .filter_map(|out| {
                    match decode_host(split_frame(&out.bytes, MAX_FRAME).unwrap().unwrap(), 1) {
                        Ok(HostMessage::InputAck { written, discarded }) => {
                            Some((written, discarded))
                        }
                        _ => None,
                    }
                })
                .collect()
        };
        client.written = 1;
        acknowledge(&mut client);
        let one = client.unsent;
        client.written = 2;
        client.discarded = 3;
        acknowledge(&mut client);
        assert_eq!(acks(&client), vec![(2, 3)]);
        assert_eq!(client.unsent, one);
        client.outgoing[0].written = 1;
        client.unsent -= 1;
        client.written = 4;
        acknowledge(&mut client);
        assert_eq!(acks(&client), vec![(2, 3), (4, 3)]);
        queue(&mut client, &HostMessage::Goodbye);
        client.written = 5;
        acknowledge(&mut client);
        assert_eq!(acks(&client), vec![(2, 3), (4, 3), (5, 3)]);
        assert_eq!(client.outgoing.len(), 4);
    }

    /// The `Welcome` a connection is owed comes first, even when the frame
    /// sent right behind its `Hello` ends the connection.
    ///
    /// Why: every frame after `Welcome` is in the version it names, so a
    /// client given `Goodbye` or `Error` without it could not know how to
    /// read them; and a client may pipeline frames behind its `Hello`
    /// (SPEC.md, "Ending" and "Host messages"). `Shutdown` and a refusal
    /// drop the frames not started, so they must keep a `Welcome` not written
    /// yet. Spec: with the peer kept open, `Hello` followed in the same write
    /// by `Shutdown`, by a second `Hello`, or by a frame of unknown kind
    /// leaves, at the iteration that takes them in, `Welcome` and then
    /// `Goodbye`, `Error` 2 or `Error` 1 queued, both unstarted; the peer
    /// then receives exactly those two frames, in that order, and end of
    /// stream.
    #[test]
    fn welcome_comes_first_whatever_ends_the_connection() {
        let hello = encode_client(&ClientMessage::Hello {
            version: 1,
            take_over: false,
        });
        for (after, code) in [
            (encode_client(&ClientMessage::Shutdown), None),
            (hello.clone(), Some(ErrorCode::NotAllowed)),
            (vec![0, 0, 0, 1, 0x7e], Some(ErrorCode::Malformed)),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
            let mut peer = Peer::raw(&host.socket());
            peer.stream
                .write_all(&[hello.clone(), after].concat())
                .unwrap();
            host.step_until("the connection to be welcomed", |host| {
                host.client.is_some()
            });
            let last_matches = |message: &HostMessage| match code {
                None => *message == HostMessage::Goodbye,
                Some(code) => matches!(message, HostMessage::Error { code: c, .. } if *c == code),
            };
            let queued = host.queued();
            assert!(
                queued.len() == 2
                    && queued[0] == HostMessage::Welcome { version: 1 }
                    && last_matches(&queued[1]),
                "{code:?}: queued {queued:?}"
            );
            let deadline = Instant::now() + PATIENCE;
            while peer.fill() {
                assert!(
                    Instant::now() < deadline,
                    "{code:?}: the connection never ended"
                );
                // After `Shutdown` this is the host ending, which is fine.
                let _ = host.host.step();
            }
            let messages: Vec<HostMessage> = std::iter::from_fn(|| peer.take()).collect();
            assert!(
                messages.len() == 2
                    && messages[0] == HostMessage::Welcome { version: 1 }
                    && last_matches(&messages[1]),
                "{code:?}: received {messages:?}"
            );
        }
    }

    /// A refused client's pending input is discarded unacknowledged at the
    /// refusal, and its `Error` is the last frame it gets.
    ///
    /// Why: an `Error` ends the connection (SPEC.md, "Host messages"):
    /// input written after it would type into the program for a connection
    /// that is going away, and an `InputAck` for it would be a frame after
    /// the last. Spec: with a raw-mode program that does not read, the pty
    /// full and input pending, a frame of unknown kind leaves, once acted
    /// on, nothing pending at no cost; once the program reads its terminal
    /// again, the client receives `Error` (`Malformed`) as its last frame,
    /// with no `InputAck` after it, and then end of stream.
    #[test]
    fn a_refusal_discards_pending_input_and_nothing_follows_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = make_fifo(dir.path());
        let script = waits_then(&fifo, "exec /bin/cat >/dev/null");
        let mut host = stepped(dir, &["/bin/sh", "-c", &script], 1 << 20);
        let mut peer = host.connect(false);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        peer.output_until(&mut host, "ready");
        fill_the_pty(&mut host, &mut peer);
        peer.write(&mut host, &[0, 0, 0, 1, 0x7e]);
        host.step_until("the refusal", |host| {
            host.client.as_ref().is_none_or(|client| client.closing)
        });
        let client = host
            .host
            .client
            .as_ref()
            .expect("premise: the refused client is still flushing its Error");
        assert!(
            client.input.is_empty() && client.input_cost == 0,
            "input still pending after the refusal"
        );
        release(&mut host, &fifo);
        let mut messages = Vec::new();
        while let Some(message) = peer.recv(&mut host) {
            messages.push(message);
        }
        assert!(
            matches!(
                messages.last(),
                Some(HostMessage::Error {
                    code: ErrorCode::Malformed,
                    ..
                })
            ),
            "the last frame: {:?}",
            messages.last()
        );
    }

    /// A refused client that does not read is dropped at its flush
    /// deadline, and the next client is welcomed.
    ///
    /// Why: a refused connection is closed once its `Error` is out, and a
    /// peer that never reads would otherwise stay the client for good, with
    /// every newcomer without take-over told `Busy` (SPEC.md, "Host
    /// messages"). Spec: with the client's socket full (it does not read its
    /// stream), a frame of unknown kind is refused: the `Error` waits behind
    /// a partly written frame, the flush deadline is `GOODBYE_FLUSH` away,
    /// and a newcomer gets `Busy`; with the deadline passed, the next
    /// iteration drops the client, and a newcomer is welcomed.
    #[test]
    fn a_refused_client_that_does_not_read_is_dropped_at_its_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        set_buffer(host.client().fd.as_raw_fd(), libc::SO_SNDBUF, 16 * 1024);
        // Less output than the reply budget, so that the frame after it is
        // acted on, but far more than the socket takes.
        host.host.journal.push_output(&vec![b'z'; 200_000]);
        peer.send(&mut host, &ClientMessage::Read(Position::default()));
        host.step_until("the socket to fill", |host| {
            host.client
                .as_ref()
                .unwrap()
                .outgoing
                .front()
                .is_some_and(|out| out.written > 0)
        });
        assert!(
            host.unsent() <= OUTGOING_HIGH_WATER,
            "premise: within budget"
        );
        let before = Instant::now();
        peer.write(&mut host, &[0, 0, 0, 1, 0x7e]);
        host.step_until("the refusal", |host| host.client.as_ref().unwrap().closing);
        let deadline = host.client().flush_deadline.expect("a flush deadline");
        assert!(
            deadline >= before + GOODBYE_FLUSH && deadline <= Instant::now() + GOODBYE_FLUSH,
            "the deadline is GOODBYE_FLUSH after the refusal"
        );
        assert!(
            host.client()
                .outgoing
                .front()
                .is_some_and(|out| out.written > 0),
            "premise: the Error waits behind a partly written frame"
        );
        let mut busy = Peer::raw(&host.socket());
        busy.send(
            &mut host,
            &ClientMessage::Hello {
                version: 1,
                take_over: false,
            },
        );
        assert_eq!(busy.recv(&mut host), Some(HostMessage::Busy));
        host.host.client.as_mut().unwrap().flush_deadline = Some(Instant::now());
        host.step();
        assert!(host.host.client.is_none(), "the refused client was dropped");
        let _next = host.connect(false);
        drop(peer);
    }

    /// Nothing the client sent after a frame that drew an `Error` is acted
    /// on, even when the host sees its close.
    ///
    /// Why: frames before an observed close are acted on whatever the
    /// bounds, but only up to one that draws an `Error` (SPEC.md,
    /// "Connections and version negotiation"); a host that went on would
    /// end on a `Shutdown` that another implementation ignores. Spec: a
    /// frame of unknown kind and a `Shutdown` sent in one write, and the
    /// connection then closed, get the client dropped, and the host keeps
    /// running and takes the next client.
    #[test]
    fn nothing_after_a_refused_frame_is_acted_on() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = stepped(dir, &["/bin/sleep", "1000"], 1 << 20);
        let mut peer = host.connect(false);
        let bytes = [
            vec![0, 0, 0, 1, 0x7e],
            encode_client(&ClientMessage::Shutdown),
        ]
        .concat();
        peer.write(&mut host, &bytes);
        drop(peer);
        host.step_until("the client to be dropped", |host| host.client.is_none());
        for _ in 0..3 {
            host.step();
        }
        assert_eq!(host.host.ended, None, "the Shutdown was acted on");
        let _next = host.connect(false);
    }

    /// A small `Input` is offered to the pty whole, in one `write` call.
    ///
    /// Why: the host must never split a key that encodes to several bytes
    /// (Shift+Enter's ESC CR) itself (SPEC.md, "Input"). A real pty cannot
    /// show where one write ended (the program's reads merge or split them
    /// as they like), so a datagram socket stands in for the master here:
    /// each write is one datagram. Spec: an `Input` of ESC CR and one of
    /// exactly `ATOMIC_INPUT` bytes, sent together, arrive as exactly two
    /// datagrams holding those two `Input`s' bytes.
    #[test]
    fn a_small_input_is_offered_in_one_write() {
        let dir = tempfile::tempdir().unwrap();
        let (master, program) = std::os::unix::net::UnixDatagram::pair().unwrap();
        program.set_nonblocking(true).unwrap();
        master.set_nonblocking(true).unwrap();
        let mut host = stepped_on(dir, master.into(), idle_child(), 1 << 20);
        let mut peer = host.connect(false);
        let full = vec![b'k'; crate::protocol::ATOMIC_INPUT];
        let bytes = [
            encode_client(&ClientMessage::Input(b"\x1b\r".to_vec())),
            encode_client(&ClientMessage::Input(full.clone())),
        ]
        .concat();
        peer.write(&mut host, &bytes);
        let total = 2 + full.len() as u64;
        host.step_until("both Inputs to be written", |host| {
            host.client.as_ref().unwrap().written == total
        });
        let mut datagrams = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n) = program.recv(&mut buf) {
            datagrams.push(buf[..n].to_vec());
        }
        assert_eq!(datagrams, vec![b"\x1b\r".to_vec(), full]);
    }

    /// A host's socket file is removed only while it is still the file the
    /// host bound.
    ///
    /// Why: a path is only a name. Something else (a client that judged the
    /// host gone, say) may have removed the file and bound another socket
    /// there, and a host removing that one on its way out would leave its
    /// host unreachable for good (SPEC.md, "Ending"). Spec: a socket bound
    /// by `listen` and still in place is removed; one replaced since by
    /// another socket at the same path (the first still open, as a running
    /// host's is) is left alone.
    #[test]
    fn a_socket_file_is_removed_only_while_it_is_the_one_bound() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s");
        let (_listener, file) = listen(&path).unwrap();
        remove_socket(&path, file);
        assert!(!path.exists(), "the host's own socket file stays");
        let (_listener, file) = listen(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let _other = std::os::unix::net::UnixListener::bind(&path).unwrap();
        remove_socket(&path, file);
        assert!(path.exists(), "another socket's file was removed");
    }
}
