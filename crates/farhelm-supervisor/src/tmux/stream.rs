//! Replay cutover and live output for one pane's tmux control client.

#[cfg(test)]
use super::control_codec::read_control_line_before;
use super::control_codec::{
    ControlLine, ControlMarker, PassthroughDecoder, classify_control_line, decode_output_payload,
    parse_control_marker, read_command_block, read_control_line, strip_command_output_terminator,
    warn_once_about_missing_bracket_paste,
};
use super::query_strip::QueryStripper;
use super::{
    HISTORY_LIMIT, PANE_MODE_FORMAT, PaneModes, TMUX_PAUSE_AFTER_SECS, TmuxDriver,
    control_cleanup_retry_delay, normalize_capture, pane_in_session,
    shutdown_output_control_client, strip_line_ending,
};
use anyhow::Context as _;
#[cfg(test)]
use anyhow::bail;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tracing::warn;

/// The largest number of ` -A <pane>:off` pairs allowed to ride the
/// attach cutover; the rest follow it as their own commands.
///
/// tmux refuses a command line carrying on the order of a thousand
/// arguments ("command too long" — the same ceiling
/// `InputClient::MAX_CHUNK` is sized against), and each pane here costs
/// two argv entries on top of the cutover's own flags. A session with
/// hundreds of tabs is unusual but not forbidden, and the failure mode
/// would be the worst kind: attach refused outright, for a session whose
/// only sin was having many terminals. Chunking keeps the cutover a fixed,
/// safe size and pushes the remainder onto the post-cutover path, which
/// has no length pressure because it can use as many commands as it likes.
const MAX_CUTOVER_PANE_FILTERS: usize = 200;

/// A short pause lets a split query complete without delaying ordinary live
/// output indefinitely when the pane has stopped writing.
const QUERY_STRIP_IDLE_FLUSH: std::time::Duration = std::time::Duration::from_millis(50);

/// Upper bound on [`OutputStream::silenced`], the per-attachment memo of
/// panes already filtered.
///
/// The memo exists only to keep a foreign pane's OUTPUT RATE from driving
/// a command rate (see that field), so losing it costs at most one extra
/// idempotent `refresh-client` per pane that speaks again — which is why a
/// long-lived attachment on a session churning through tabs may simply
/// forget everything and start over rather than grow without bound. 512 is
/// far above any real session's simultaneous pane count and still a
/// trivial amount of memory.
const MAX_SILENCED_PANES: usize = 512;

/// The command that lists every pane of `session`, one pane id per line,
/// asked over a control client rather than as a separate `tmux` process.
///
/// Used only by [`OutputStream::foreign_panes`], which needs the
/// answer on the very client it is about to configure — a separate
/// invocation would cost a process spawn to learn something the client's
/// own connection can answer inside the attach exchange it is already
/// having.
fn list_session_panes_command(session: &str) -> String {
    format!("list-panes -s -t \"={session}\" -F \"#{{pane_id}}\"")
}

/// `refresh-client` arguments that turn off a control client's delivery
/// for panes it does not speak for — one ` -A <pane>:off` per pane, tmux's
/// documented per-client pane filter.
///
/// Arguments rather than a whole command because at attach time the first
/// [`MAX_CUTOVER_PANE_FILTERS`] of them MUST share an invocation with the
/// `no-output` flip (see [`attach_cutover_command`]). Once output is live,
/// [`silence_live_pane_commands`] adds a preceding `pause` instead of using
/// these off-only arguments.
///
/// # Why this is safe only alongside [`super::SessionSink`]
///
/// tmux stops READING a pane whose every attached client has turned it
/// off — documented in `refresh-client`'s own man page, and confirmed
/// empirically (2026-08-02, 3.4 and 3.7b): with this as the only client,
/// turning a busy pane off froze that pane's producer for the entire
/// 10-second observation window. It is the sink — an attached client that
/// never turns any pane off and never falls behind — that keeps this from
/// being a way to freeze the terminal nobody happens to be looking at.
/// The same audit confirmed the other half: with the sink attached, the
/// filtered client received exactly zero notifications for the silenced
/// pane while its own pane's output kept arriving, and the silenced
/// pane's producer never stalled.
fn silence_pane_args(panes: &[String]) -> String {
    let mut args = String::new();
    for pane in panes {
        // Quoted for `continue_pane_command`'s reason: the argument
        // contains a `:`.
        let _ = write!(args, " -A \"{pane}:off\"");
    }
    args
}

/// One command that safely filters panes after output is live.
///
/// tmux 3.3 through 3.6 must see `pause` before `off` once a control client
/// can have pane output queued. Keeping both transitions in one command
/// prevents more output from interleaving between them. This renderer is
/// shared by attach-time overflow and panes discovered later so those two
/// live paths cannot drift on the compatibility-sensitive ordering.
fn silence_live_pane_command(panes: &[String]) -> String {
    let mut command = String::from("refresh-client");
    for pane in panes {
        let _ = write!(command, " -A \"{pane}:pause\" -A \"{pane}:off\"");
    }
    command
}

/// Chunk live pane filters below tmux's argument ceiling.
///
/// Each pane costs twice as many arguments as it does in the cutover, so a
/// live command carries half as many panes to preserve the same argv budget.
fn silence_live_pane_commands(panes: &[String]) -> Vec<String> {
    panes
        .chunks(MAX_CUTOVER_PANE_FILTERS / 2)
        .map(silence_live_pane_command)
        .collect()
}

/// The command that ends the attach-time replay command group and hands
/// the client over to live output.
///
/// Two client flags in one comma-separated `-f` list, and both halves are
/// load-bearing. `!no-output` (the leading `!` is tmux's own
/// clear-this-flag form) is the cutover itself: the client attached with
/// `no-output` set, so this is the instant pane bytes start flowing.
/// `pause-after=N` is what makes those bytes safe to fall behind on — see
/// [`TMUX_PAUSE_AFTER_SECS`]. Setting `pause-after` here rather than on
/// the original `attach` is deliberate: it belongs to the same atomic
/// hand-over as the cutover, so there is no window in which output flows
/// without the backstop.
///
/// Enabling `pause-after` also switches this client's output dialect —
/// pane bytes arrive as `%extended-output` instead of `%output`, and
/// `%pause`/`%continue` notifications appear (see [`OutputEvent`]).
/// Verified against tmux 3.7b that the combined flag list is accepted and
/// does exactly this.
///
/// # Why the pane filter rides HERE rather than being sent beforehand
///
/// `silenced` is the session's other panes — at most
/// [`MAX_CUTOVER_PANE_FILTERS`] of them, the caller having split off any
/// overflow — and folding them into this one invocation is not tidiness;
/// it is the only spelling that works.
/// Clearing `no-output` makes tmux discard the client's per-pane state, so
/// a `refresh-client -A <pane>:off` sent as its own command during the
/// attach handshake is wiped by the very cutover that starts the output it
/// was meant to filter. Audited 2026-08-02 on tmux 3.4 and 3.7b: the
/// separate-command form let 1840 and 1847 foreign notifications through
/// in six seconds respectively, while this combined form let through
/// exactly zero on both, with the client's own pane unaffected. That the
/// filter is silently lost rather than refused is what makes this worth a
/// paragraph — the failure mode is a performance regression nobody would
/// look for.
///
/// The overflow does NOT share this hazard, which is why splitting it off
/// is safe: it is sent after the cutover, when nothing changes the client's
/// `no-output` flag again and so nothing resets its per-pane state.
fn attach_cutover_command(silenced: &[String]) -> String {
    debug_assert!(
        silenced.len() <= MAX_CUTOVER_PANE_FILTERS,
        "the caller must split the overflow off; see MAX_CUTOVER_PANE_FILTERS"
    );
    format!(
        "refresh-client{} -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}",
        silence_pane_args(silenced)
    )
}

/// The command that lifts a tmux-side pause on `pane` — `refresh-client
/// -A <pane-id>:continue`, tmux's documented pane-state form.
///
/// Used as the FINAL command of the catch-up replay group (see
/// [`OutputStream::resume_paused_with_replay`]), which is why it is a
/// command rather than a flag: it is the resume path's cutover, exactly
/// as [`attach_cutover_command`] is the attach path's. tmux acknowledges
/// it with a `%continue <pane-id>` notification and resumes the stream
/// immediately after the command group's own `%end` (verified against
/// tmux 3.7b). The argument is quoted because it contains a `:`; the
/// pane id's leading `%` is required by tmux.
fn continue_pane_command(pane: &str) -> String {
    format!("refresh-client -A \"{pane}:continue\"")
}

/// The one command group a replay cutover sends: pane modes, history
/// snapshot, visible snapshot, then `cutover` as the final command whose
/// `%end` is the replay/live boundary.
///
/// Shared rather than duplicated because the four commands and their
/// ORDER are the contract [`OutputStream::snapshot_then_cutover`] reads
/// replies against: modes, history snapshot, visible snapshot, cutover.
/// A second copy of this string for the resume path would be a second
/// place for that block-count contract to drift.
fn replay_command_group(session: &str, pane: &str, cutover: &str) -> String {
    // Every pane target is paired with its session ([`pane_in_session`]),
    // quoted because the pairing contains a `:`. A control client attaches
    // to a session but its COMMANDS resolve server-wide, so a bare `%N`
    // here would let a pane id that went stale across a tmux-server
    // restart capture — and replay — a completely different session's
    // terminal into this one.
    //
    // The captures and the cutover travel as ONE command group on
    // purpose, and the adjacency is a correctness property, not styling:
    // output the pane emits between the visible capture and the cutover
    // is delivered to nobody, so any command inserted between them — or
    // any retry scheme that re-runs the captures before a separately
    // written cutover — widens that loss window from effectively nothing
    // into something the cutover tests catch losing real records through.
    // (An issue-4 attempt at bracketing the captures with a second mode
    // sample and retrying on mismatch failed exactly that way; the fresh-
    // tab tear it chased is prevented at its source instead, by pre-sizing
    // the tab window at open so the attach-time resize stops provoking a
    // mid-capture repaint.)
    let target = format!("\"{}\"", pane_in_session(session, pane));
    format!(
        "display-message -p -t {target} '{PANE_MODE_FORMAT}' ; \
         capture-pane -p -e -N -t {target} -S -{HISTORY_LIMIT} ; \
         capture-pane -p -e -N -t {target} ; \
         {cutover}\n"
    )
}

impl TmuxDriver {
    /// Open one control client, capture replay, then turn on its live
    /// output without leaving a gap between the two.
    ///
    /// The client attaches with tmux's `no-output` flag. Mode query, two
    /// snapshots, and [`attach_cutover_command`] are then submitted as one
    /// command group through that same client. tmux runs a command group
    /// synchronously before returning to pane reads, so the final
    /// command's `%end` is the cutover: pane bytes before it are already
    /// represented by the selected snapshot, while bytes after it arrive
    /// as `%extended-output` on this stream (the cutover also sets
    /// `pause-after`, which switches the dialect — see [`OutputEvent`]).
    ///
    /// Both history and visible-only snapshots are taken because modes
    /// decide which one is valid. Normal-screen replay includes
    /// scrollback; alternate-screen replay must not mix in the normal
    /// screen's history. Keeping both captures in the same command group
    /// avoids a second mode/capture race without depending on nested
    /// `if-shell` reply blocks, whose shape changed across supported tmux
    /// releases.
    ///
    /// `session` and `pane` are BOTH needed and mean different things: a
    /// control client can only attach to a session, while everything this
    /// stream then carries is filtered down to the one pane — see
    /// [`OutputStream`] for why that filter exists and what it costs.
    pub async fn open_replay_stream(
        &self,
        session: &str,
        pane: &str,
    ) -> anyhow::Result<(PaneModes, Vec<u8>, OutputStream)> {
        Ok(self
            .open_replay_stream_candidate(session, pane)
            .await?
            .install())
    }

    /// Build a replay stream behind a cancellation-safe ownership guard.
    ///
    /// The supervisor keeps this guard until the input client is also ready.
    /// Abandoning it at any point safely reaps the output-bearing client rather
    /// than exercising the raw stream's emergency `kill_on_drop` fallback.
    pub(crate) async fn open_replay_stream_candidate(
        &self,
        session: &str,
        pane: &str,
    ) -> anyhow::Result<ReplayStreamCandidate> {
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        let mut child = self
            .command()
            .arg("-C")
            .arg("attach")
            .arg("-f")
            .arg("no-output")
            .arg("-t")
            .arg(session)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("spawning tmux control-mode client")?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stream = OutputStream {
            driver: self.clone(),
            client_target: format!(
                "client-{}",
                child
                    .id()
                    .context("tmux control-mode client has no process id")?
            ),
            output_disabled: false,
            child,
            // Exclusively owned: this client carries its own one-shot
            // replay-cutover command group (above), the attach-time pane
            // filter, and the occasional live `pause`-then-`off` filter —
            // all written from the single task that owns this stream. Input
            // travels on a wholly separate client — see
            // `open_input_client` — so there is no sharing concern here.
            stdin: Some(stdin),
            reader: BufReader::new(stdout),
            line: Vec::with_capacity(8192),
            passthrough: PassthroughDecoder::default(),
            query_strip: QueryStripper::default(),
            query_strip_deadline: None,
            pane: pane.to_string(),
            session: session.to_string(),
            silenced: HashSet::new(),
            pending_filter_replies: 0,
            foreign_dropped: 0,
            exchange_timeout: self.exchange_timeout,
            pane_list_timeout: self.pane_list_timeout,
            exit_reason: None,
        };
        let mut candidate = OutputStreamCandidate::new(stream);
        let opened = async {
            let stream = candidate.stream_mut();
            read_command_block(
                &mut stream.reader,
                &mut stream.line,
                deadline,
                "control-mode attach",
                pane,
            )
            .await?;
            // The session's other panes are learned while output is still off
            // and then silenced BY the cutover itself — see
            // `attach_cutover_command` for why they cannot be silenced by an
            // earlier command, and `OutputStream` for what the filter buys.
            // Any failure here propagates: this exchange shares the client's
            // stdout with the positionally-read replay group that follows, so
            // an exchange that did not complete cleanly leaves a stream nobody
            // can safely keep reading (see `foreign_panes`).
            let foreign = stream.foreign_panes(deadline).await?;
            // Only the first chunk can ride the cutover; the rest follow it
            // (see `MAX_CUTOVER_PANE_FILTERS`). Splitting BEFORE the cutover
            // rather than trimming inside it keeps the overflow addressed
            // rather than silently unfiltered.
            let split = foreign.len().min(MAX_CUTOVER_PANE_FILTERS);
            let (riding, overflow) = foreign.split_at(split);
            let (modes, prefill) = stream
                .snapshot_then_cutover(deadline, &attach_cutover_command(riding))
                .await?;
            for command in silence_live_pane_commands(overflow) {
                stream.send_filter_command(&command).await?;
            }
            stream.silenced.extend(foreign.iter().cloned());
            Ok::<_, anyhow::Error>((modes, prefill))
        }
        .await;
        match opened {
            Ok((modes, prefill)) => Ok(ReplayStreamCandidate::new(modes, prefill, candidate)),
            Err(error) => {
                candidate.shutdown().await;
                Err(error)
            }
        }
    }
}

/// A control-mode client streaming ONE PANE's output.
///
/// It starts with output disabled and is only returned after replay
/// capture and the live cutover have completed. The client counts as
/// attached in tmux's eyes but never declares a size (no
/// `refresh-client -C`), so tmux ignores it for sizing entirely —
/// geometry comes only from explicit `resize-window` calls. Orderly teardown
/// closes the command pipe and drains the client before reaping it.
/// `kill_on_drop` remains only for cancellation and unwinding, where an async
/// shutdown cannot run.
///
/// # One pane, out of a whole session's stream
///
/// tmux control clients attach to a SESSION, not to a window or a pane,
/// and a control client receives `%output`/`%extended-output` for every
/// pane in every window of that session — audited empirically against
/// tmux 3.7b with a second window present, which is the shape terminal
/// tabs introduce (PLAN_M4.md item 2). Narrowing that down takes BOTH of
/// the mechanisms below, and neither is redundant.
///
/// **tmux-side, and the one that matters.** Every pane of the session
/// except this stream's own is turned off for this client with
/// `refresh-client -A <pane>:off` — at attach for the panes that exist
/// then ([`Self::foreign_panes`], folded into the cutover), and after a
/// protective `pause` when the filter must be installed after live output
/// begins. That second path covers both attach-time overflow and a pane
/// first discovered later ([`Self::silence_late_pane`]). This is
/// what makes a session's control-mode read work O(session traffic)
/// instead of O(attached terminals × session traffic), and — more
/// importantly than the arithmetic — it is what stops a busy tab's bytes
/// from queueing in front of the agent's bytes inside that viewer's own
/// client. That head-of-line delay was measured at 3.3–3.9s of
/// agent-output latency under a flooding neighbour, against 0.36–0.38s
/// with the filter on (2026-08-02, tmux 3.4 and 3.7b).
///
/// This is only safe because [`super::SessionSink`] exists: tmux stops READING a
/// pane whose every attached client has turned it off, and the sink is the
/// client that never does. Turning foreign panes off WITHOUT a sink froze
/// the silenced pane's producer for an entire 10-second observation window
/// in the same audit. The two mechanisms ship together or not at all.
///
/// **Local, as belt and braces.** A notification naming another pane is
/// still dropped on the way through. tmux's filter is a per-client
/// setting, so there is always a window — an attach in flight, a pane
/// created a moment ago — in which a foreign notification can legitimately
/// arrive, and a stray one must be harmless rather than merely unlikely.
/// The drop runs BEFORE any decoding: [`classify_control_line`] splits the
/// pane id out of each notification, and a line naming another pane is
/// dropped there — never fed to [`PassthroughDecoder`], which carries
/// wrapper state across notifications and would be corrupted by a
/// stranger's bytes landing inside a wrapper of ours.
///
/// # Stall isolation, and what it now rests on
///
/// A stalled viewer on one terminal costs that terminal alone. Three
/// separate properties combine to that, and dropping any one of them
/// weakens it:
///
/// - `pause-after` is per (client, pane), so tmux cuts the stalled
///   client's stream rather than anybody else's (see
///   [`TMUX_PAUSE_AFTER_SECS`]).
/// - The pane filter above means a stalled terminal is only ever behind on
///   its own pane in the first place.
/// - [`super::SessionSink`] guarantees tmux always has a client it can deliver
///   every pane to, so tmux's OTHER answer to a lagging client — stop
///   reading the pane, which blocks the pane's process on its next write
///   and is not bounded by `pause-after` — has no client to take against.
///
/// Before the sink this last branch was a real, measured residual, and the
/// honest qualification that used to live here said so. It is now closed
/// rather than merely narrowed: with the sink attached, no trial across
/// tmux 3.4 and 3.7b reproduced a pane stall that stalled-client trials
/// without it reproduced 4 times in 5.
pub struct OutputStream {
    /// Independent access to the private server for the shutdown boundary.
    driver: TmuxDriver,
    /// tmux names non-terminal control clients `client-<pid>`.
    client_target: String,
    /// Whether the server has acknowledged client-wide `no-output`.
    output_disabled: bool,
    /// Exposed only so the shared real-tmux test harness can distinguish a
    /// dead client process from a live process whose output pipe failed.
    pub(super) child: Child,
    /// Held open for the lifetime of the client. Carries the attach-time
    /// exchanges (the pane list, then the replay-cutover command group)
    /// and, afterwards, the occasional fire-and-forget pane filter for a
    /// pane that appeared late — never terminal input, which travels on a
    /// wholly separate [`super::InputClient`]. So this is a plain,
    /// exclusively-owned handle rather than a shared, lockable one: only
    /// the one task driving this stream ever writes to it.
    stdin: Option<ChildStdin>,
    reader: BufReader<tokio::process::ChildStdout>,
    line: Vec<u8>,
    /// Stateful because tmux may split one passthrough wrapper across
    /// several output notifications.
    passthrough: PassthroughDecoder,
    /// Stateful because tmux may split a terminal query across notifications;
    /// this applies only after live payload decoding, never to replay bytes.
    query_strip: QueryStripper,
    /// Absolute own-pane idle deadline for a retained query candidate.
    ///
    /// The control client also carries command replies and other panes' output,
    /// neither of which says this pane is still writing. Keeping the deadline
    /// here prevents that unrelated traffic from extending a retained prefix.
    query_strip_deadline: Option<tokio::time::Instant>,
    /// The one pane this stream speaks for — see the type's own docs for
    /// why a session-wide client needs to know that at all.
    ///
    /// Held as the raw tmux pane id (`%N`) so every comparison is a plain
    /// byte equality against what a notification carries, with no
    /// re-parsing per line.
    pane: String,
    /// The tmux session that pane belongs to, kept so the catch-up replay
    /// can address it the same session-paired way the attach did — and so
    /// no caller has to carry a second copy of either handle (see
    /// [`Self::pane`]).
    session: String,
    /// Panes this client has already told tmux to stop delivering (see the
    /// type's own docs).
    ///
    /// Purely a memo against re-sending: the state that matters lives in
    /// the tmux server, and `refresh-client -A <pane>:off` is idempotent
    /// there. What this prevents is a pane that manages to speak once more
    /// before the off takes effect turning into a command per notification
    /// — an unbounded write rate driven by a foreign pane's output rate,
    /// which is exactly the coupling the filter exists to remove.
    ///
    /// Bounded by [`MAX_SILENCED_PANES`] rather than pruned precisely.
    /// tmux's control protocol has no pane-close notification to prune on
    /// (`%window-close` names a window, and a pane can also go without
    /// one), and reconciling against a live `list-panes` would mean an
    /// exchange on a client whose replies nothing is positioned to read.
    /// Forgetting wholesale is sound because every entry is only a memo:
    /// the worst a cleared set can cost is one redundant, idempotent
    /// command per pane that speaks again.
    silenced: HashSet<String>,
    /// Filter-command reply blocks tmux still owes this client.
    ///
    /// Every command written outside a synchronous exchange
    /// ([`Self::send_filter_command`]) is answered by a `%begin`/`%end`
    /// pair that arrives on the same stdout the CATCH-UP path reads
    /// positionally. Left unaccounted, one such reply landing just before
    /// [`Self::resume_paused_with_replay`] would be mistaken for the
    /// modes reply and shift the whole four-block group by one — a replay
    /// that silently returns the wrong capture, which is exactly the class
    /// of bug positional reads exist to be careful about.
    ///
    /// So the count goes up when a command is written and down when
    /// [`Self::next_output`] passes a block terminator, and the catch-up
    /// path drains whatever is still outstanding before it begins. Nothing
    /// else ever writes to this client, so every terminator this stream
    /// sees while live belongs to one of these.
    pending_filter_replies: usize,
    /// How many foreign-pane notifications this stream dropped locally.
    ///
    /// Not used in production — it exists so a test can assert the tmux
    /// filter is doing the work rather than the local drop quietly
    /// covering for it. "The right bytes came out" is true either way; only
    /// this number distinguishes a filter that was installed from one that
    /// was not. Read directly by the tests in this module; deliberately
    /// without an accessor, so nothing outside can grow a dependency on a
    /// diagnostic counter.
    foreign_dropped: u64,
    /// This stream's copy of [`TmuxDriver::exchange_timeout`], carried
    /// here because [`Self::send_filter_command`] and
    /// [`Self::resume_paused_with_replay`] compute their own fresh
    /// deadlines well after the attach that created this stream — they
    /// have no deadline handed to them by a caller, so the budget has to
    /// live on the stream itself.
    exchange_timeout: std::time::Duration,
    /// This stream's copy of [`TmuxDriver::pane_list_timeout`], consulted
    /// by [`Self::foreign_panes`] for the same reason as
    /// `exchange_timeout`.
    pane_list_timeout: std::time::Duration,
    /// Why tmux said this control client was going away, if it said so at
    /// all: `None` means no `%exit` was ever seen — the stream simply hit
    /// EOF, which is what a tmux process killed outright looks like from
    /// here — while `Some("")` means tmux announced a bare `%exit` with no
    /// reason.
    ///
    /// Purely diagnostic. Nothing in production branches on it:
    /// `Ok(None)` from [`Self::next_output`] already means everything the
    /// forwarder needs to act on, and the reason is logged when it is set.
    /// Parent visibility exists only so the shared real-tmux test harness can
    /// name the cause when a control client vanishes mid-test; three earlier
    /// CI failures yielded no evidence about what happened to the client.
    pub(super) exit_reason: Option<String>,
}

/// The optional supervisor barrier completed by a provisional replay stream.
type ReplayCompletionSender = tokio::sync::watch::Sender<Option<Result<(), std::sync::Arc<str>>>>;

/// A replay stream that has not yet been committed to an attachment.
///
/// The supervisor can hold this across input-client setup. Cancellation or an
/// input setup failure then reaps the already-output-bearing client before the
/// optional handoff barrier completes; installing it transfers ownership to
/// the forwarder and completes that barrier immediately.
pub(crate) struct ReplayStreamCandidate {
    modes: Option<PaneModes>,
    prefill: Option<Vec<u8>>,
    stream: Option<OutputStreamCandidate>,
    completion: Option<ReplayCompletionSender>,
    runtime: tokio::runtime::Handle,
}

impl ReplayStreamCandidate {
    fn new(modes: PaneModes, prefill: Vec<u8>, stream: OutputStreamCandidate) -> Self {
        Self {
            modes: Some(modes),
            prefill: Some(prefill),
            stream: Some(stream),
            completion: None,
            runtime: tokio::runtime::Handle::current(),
        }
    }

    /// Tie abandonment to a supervisor-published terminal handoff barrier.
    pub(crate) fn set_completion(&mut self, completion: ReplayCompletionSender) {
        self.completion = Some(completion);
    }

    /// Commit the client to a forwarder and release its provisional barrier.
    pub(crate) fn install(mut self) -> (PaneModes, Vec<u8>, OutputStream) {
        if let Some(completion) = self.completion.take() {
            completion.send_replace(Some(Ok(())));
        }
        (
            self.modes.take().expect("replay candidate has modes"),
            self.prefill.take().expect("replay candidate has prefill"),
            self.stream
                .take()
                .expect("replay candidate has a stream")
                .install(),
        )
    }
}

impl Drop for ReplayStreamCandidate {
    fn drop(&mut self) {
        let Some(stream) = self.stream.take() else {
            return;
        };
        let completion = self.completion.take();
        self.runtime.spawn(async move {
            stream.shutdown().await;
            if let Some(completion) = completion {
                completion.send_replace(Some(Ok(())));
            }
        });
    }
}

/// Own an output client until attach either commits it or reaps it safely.
///
/// The attach exchange can be cancelled after tmux has enabled live output.
/// Dropping the raw stream there would invoke `kill_on_drop` while pane blocks
/// may still be queued, which is the tmux 3.7b server-abort trigger. This guard
/// transfers every abandoned client to the runtime and keeps retrying the
/// acknowledged `no-output` boundary instead.
struct OutputStreamCandidate {
    stream: Option<OutputStream>,
    runtime: tokio::runtime::Handle,
}

impl OutputStreamCandidate {
    fn new(stream: OutputStream) -> Self {
        Self {
            stream: Some(stream),
            runtime: tokio::runtime::Handle::current(),
        }
    }

    fn stream_mut(&mut self) -> &mut OutputStream {
        self.stream.as_mut().expect("output candidate is live")
    }

    fn install(mut self) -> OutputStream {
        self.stream.take().expect("output candidate is live")
    }

    async fn shutdown(mut self) {
        if let Some(stream) = self.stream.take()
            && let Err(reaper) = stream.shutdown().await
        {
            let _ = reaper.run().await;
        }
    }
}

impl Drop for OutputStreamCandidate {
    fn drop(&mut self) {
        let Some(stream) = self.stream.take() else {
            return;
        };
        self.runtime.spawn(async move {
            if let Err(reaper) = stream.shutdown().await {
                let _ = reaper.run().await;
            }
        });
    }
}

impl OutputStream {
    /// The pane this stream carries.
    ///
    /// Exists so the forwarder driving this stream does not keep its own
    /// copy of the pane id: two fields that must agree, in two structs
    /// with different lifetimes, is exactly the kind of duplication that
    /// goes wrong quietly — a forwarder replaying the wrong pane's
    /// history would look like a plain terminal glitch.
    pub fn pane(&self) -> &str {
        &self.pane
    }

    /// Every pane of this stream's session except its own, asked while
    /// output is still off so the answer can be folded into the cutover.
    ///
    /// # Why a failure here fails the whole attach
    ///
    /// What this produces is only an optimization's input — the local
    /// filter in [`Self::next_output`] drops whatever arrives anyway, and
    /// [`super::SessionSink`] keeps every pane readable regardless — so it is
    /// tempting to swallow errors and attach unfiltered. That would be
    /// wrong for a reason that has nothing to do with the filter: this
    /// exchange shares one stdout with the replay group that follows, and
    /// that group reads its four reply blocks POSITIONALLY. An exchange
    /// that timed out or hit EOF mid-block may have left an unconsumed
    /// `%end` behind, and continuing would then pair the modes reply with
    /// the history capture — a replay that returns the wrong bytes and
    /// says nothing. There is no way to tell a cleanly-refused command
    /// from a half-read one at this layer, so every failure is treated as
    /// the dangerous one and the caller discards the client.
    ///
    /// The deadline is [`super::PANE_LIST_TIMEOUT`], clamped to the attach's own:
    /// a tmux slow to answer an optimization's question must not eat the
    /// budget the replay still needs, and must not extend the attach
    /// either.
    ///
    /// The obvious race — a pane closing between this listing and the
    /// cutover that names it — is benign, and that was worth confirming
    /// rather than assuming, because the alternative would have been an
    /// attach that fails whenever a tab happens to close at the wrong
    /// moment. tmux accepts `-A` for a pane id it cannot find and replies
    /// `%end`, not `%error` (audited 2026-08-02 on 3.4 and 3.7b), so a
    /// stale id in the cutover costs nothing at all.
    async fn foreign_panes(
        &mut self,
        attach_deadline: tokio::time::Instant,
    ) -> anyhow::Result<Vec<String>> {
        let deadline = attach_deadline.min(tokio::time::Instant::now() + self.pane_list_timeout);
        let command = list_session_panes_command(&self.session);
        let listing = self
            .exchange(deadline, &command, "session pane list")
            .await
            .context("listing a session's panes for this terminal's pane filter")?;
        Ok(listing
            .split(|&byte| byte == b'\n')
            .map(strip_line_ending)
            .filter(|pane| is_pane_id_shaped(pane) && *pane != self.pane.as_bytes())
            .map(|pane| String::from_utf8_lossy(pane).into_owned())
            .collect())
    }

    /// Write one pane-filter command and record that tmux owes a reply for
    /// it — the only way this type writes once output is live.
    ///
    /// Fire-and-forget in the sense that nothing waits for the reply HERE:
    /// the stream's own reader passes it as chatter (and decrements the
    /// debt), or the catch-up path drains it before its positional reads
    /// begin. See [`Self::pending_filter_replies`] for why the debt is
    /// counted rather than assumed to have cleared.
    ///
    /// The write is bounded by [`super::CONTROL_EXCHANGE_TIMEOUT`] and a failure
    /// is returned rather than logged. Both matter and neither is
    /// theoretical: this runs from inside [`Self::next_output`], so an
    /// unbounded `write_all` on a pipe whose reader has wedged would park
    /// the forwarder — a terminal that stops updating with nothing
    /// anywhere reporting a fault — and a swallowed error would leave the
    /// stream believing a filter is installed that never was.
    async fn send_filter_command(&mut self, command: &str) -> anyhow::Result<()> {
        let line = format!("{command}\n");
        let stdin = self
            .stdin
            .as_mut()
            .context("the tmux pane-filter client is already shutting down")?;
        tokio::time::timeout(self.exchange_timeout, async {
            stdin
                .write_all(line.as_bytes())
                .await
                .context("writing a tmux pane-filter command")?;
            stdin
                .flush()
                .await
                .context("flushing a tmux pane-filter command")
        })
        .await
        .context("timed out writing a tmux pane-filter command")??;
        self.pending_filter_replies += 1;
        Ok(())
    }

    /// Consume every filter-command reply block tmux still owes, so a
    /// positional read can start from a known boundary.
    ///
    /// Called only by [`Self::resume_paused_with_replay`], and only ever
    /// with this stream's own pane already paused — which is what makes
    /// [`read_command_block`]'s own-pane guard the right one to pass:
    /// live output for our pane arriving here really would mean the
    /// ordering assumption had broken.
    async fn settle_filter_replies(
        &mut self,
        deadline: tokio::time::Instant,
    ) -> anyhow::Result<()> {
        while self.pending_filter_replies > 0 {
            let pane = self.pane.clone();
            read_command_block(
                &mut self.reader,
                &mut self.line,
                deadline,
                "pane filter reply",
                &pane,
            )
            .await?;
            self.pending_filter_replies -= 1;
        }
        Ok(())
    }

    /// Stop delivery of a pane this stream had never heard from before —
    /// the tab opened, or the pane split, after this terminal attached.
    ///
    /// tmux's pane filter names panes one at a time and has no wildcard
    /// (`refresh-client -A` takes a pane id; only `-B` accepts `%*`), and a
    /// pane that did not exist at attach time is delivered ON by default.
    /// Reacting to the pane's first notification is what covers that
    /// without any plumbing from the tab-open path down into every
    /// attached terminal's forwarder: the notification IS the discovery.
    /// The cost is at most one command per pane per attachment, since
    /// [`Self::silenced`] remembers.
    ///
    /// The command goes out through [`Self::send_filter_command`], so its
    /// reply is accounted for rather than merely expected to be harmless,
    /// and a write failure ends the stream instead of being logged: a
    /// terminal whose control client cannot be written to is not a
    /// terminal that should keep pretending.
    ///
    /// Unlike the attach-time filter this CAN be its own command: nothing
    /// changes the client's `no-output` flag after the cutover, so there is
    /// no per-pane state reset to lose it to (see
    /// [`attach_cutover_command`]).
    ///
    /// A pane id that is not shaped like one, or one already remembered,
    /// is a no-op — the first because nothing unvalidated reaches a
    /// command line, the second because [`Self::silenced`] exists exactly
    /// to keep a chatty foreign pane from driving a command per line.
    async fn silence_late_pane(&mut self, pane: &[u8]) -> anyhow::Result<()> {
        if !is_pane_id_shaped(pane) {
            return Ok(());
        }
        let pane = String::from_utf8_lossy(pane).into_owned();
        if self.silenced.contains(&pane) {
            return Ok(());
        }
        // Forget everything rather than grow without bound; see
        // `MAX_SILENCED_PANES` for why that is sound and what it costs.
        if self.silenced.len() >= MAX_SILENCED_PANES {
            self.silenced.clear();
        }
        self.silenced.insert(pane.clone());
        let command = silence_live_pane_command(std::slice::from_ref(&pane));
        self.send_filter_command(&command).await
    }

    /// Send one command on this client and read back its reply block.
    ///
    /// Only usable while output is off (the attach exchange): once pane
    /// bytes flow, replies have to be read by whatever is draining the
    /// stream, which is why every later command goes through
    /// [`Self::send_filter_command`] instead.
    async fn exchange(
        &mut self,
        deadline: tokio::time::Instant,
        command: &str,
        purpose: &str,
    ) -> anyhow::Result<Vec<u8>> {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let pane = self.pane.clone();
        let stdin = self
            .stdin
            .as_mut()
            .context("the tmux exchange client is already shutting down")?;
        tokio::time::timeout(remaining, async {
            stdin
                .write_all(format!("{command}\n").as_bytes())
                .await
                .with_context(|| format!("writing the tmux {purpose} command"))?;
            stdin
                .flush()
                .await
                .with_context(|| format!("flushing the tmux {purpose} command"))
        })
        .await
        .with_context(|| format!("timed out writing the tmux {purpose} command"))??;
        read_command_block(&mut self.reader, &mut self.line, deadline, purpose, &pane).await
    }
}

/// What one classified control line means to [`OutputStream::next_output`],
/// in a form that outlives the borrow of the line buffer it was read from.
///
/// Exists purely so the read loop can act on a line after releasing that
/// borrow — the foreign-pane arm writes a command through `&mut self`, and
/// the terminator arm touches the reply-debt counter, neither of which can
/// happen while a classified slice of `self.line` is still alive.
enum Decision {
    /// Hand this to the caller and stop reading.
    Event(OutputEvent),
    /// The client is gone, carrying whatever reason tmux gave for it
    /// (empty for a bare `%exit`) — see [`ControlLine::Exit`].
    Exit(String),
    /// Our pane's bytes ended mid-passthrough-wrapper: nothing to hand
    /// back yet, keep reading.
    Incomplete,
    /// A foreign pane already filtered — count it, do nothing else.
    ForeignKnown,
    /// A foreign pane heard from for the first time, carrying its id.
    ForeignNew(Vec<u8>),
    /// Anything else tmux says. Only a block terminator is acted on; see
    /// the read loop.
    Chatter,
}

/// Whether `candidate` has the shape of a tmux pane id (`%` followed by at
/// least one digit, nothing else).
///
/// Every id this module puts into a `refresh-client -A` argument passes
/// through here first. Pane ids reach this process from tmux itself, but
/// they arrive as bytes on a stream shared with pane CONTENT — a
/// notification's shape is the only thing separating the two — so a value
/// that is about to be interpolated into a command line is shape-checked
/// rather than trusted, the same defensive reading `scope::is_uuid_shaped`
/// applies to tmux user options.
fn is_pane_id_shaped(candidate: &[u8]) -> bool {
    matches!(candidate.split_first(), Some((b'%', digits))
        if !digits.is_empty() && digits.iter().all(u8::is_ascii_digit))
}

/// One thing read off a control client's stream that the forwarder cares
/// about. Everything else tmux says (command replies, layout changes,
/// window renames) is consumed and discarded inside
/// [`OutputStream::next_output`].
///
/// The two non-byte variants exist because `pause-after` (see
/// [`TMUX_PAUSE_AFTER_SECS`]) makes tmux's flow-control state visible on
/// this stream, and the supervisor must act on it rather than treat it as
/// chatter: a `Paused` means tmux CUT this client's pane stream and the
/// bytes it dropped are gone from the live path for good, recoverable
/// only by replaying history (PLAN_M2_5.md's reset-then-replay catch-up).
/// Swallowing it would leave the client silently missing a chunk of its
/// terminal with nothing anywhere noticing.
#[derive(Debug, PartialEq, Eq)]
pub enum OutputEvent {
    /// Decoded pane bytes, ready for the wire. Never empty.
    Bytes(Vec<u8>),
    /// `%pause`: tmux stopped sending this pane's output to this client
    /// because it fell further behind than `pause-after` allows, and
    /// discarded what it had queued. The stream stays quiet until
    /// [`OutputStream::resume_paused_with_replay`].
    ///
    /// Its ABSENCE is not evidence of anything: tmux answers a lagging
    /// client either this way or by throttling the pane instead, and
    /// which one is timing-dependent rather than version-dependent (see
    /// [`TMUX_PAUSE_AFTER_SECS`]). Code may act on this event but must
    /// never wait for one.
    Paused,
}

impl OutputStream {
    /// Capture modes and content, then hand the client over to live
    /// output at the final command block boundary.
    ///
    /// `cutover` is that final command — [`attach_cutover_command`] on the
    /// attach path, [`continue_pane_command`] on the catch-up path. The
    /// four expected blocks are deliberately explicit. Treating the group
    /// as one opaque reply makes it too easy to enable output after only
    /// the first `%end`, which reintroduces the replay/live overlap this
    /// method exists to remove.
    async fn snapshot_then_cutover(
        &mut self,
        deadline: tokio::time::Instant,
        cutover: &str,
    ) -> anyhow::Result<(PaneModes, Vec<u8>)> {
        let command = replay_command_group(&self.session, &self.pane, cutover);
        let pane = self.pane.clone();
        let pane = pane.as_str();
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let stdin = self
            .stdin
            .as_mut()
            .context("the tmux replay client is already shutting down")?;
        tokio::time::timeout(remaining, async {
            stdin
                .write_all(command.as_bytes())
                .await
                .context("writing tmux replay cutover commands")?;
            stdin
                .flush()
                .await
                .context("flushing tmux replay cutover commands")
        })
        .await
        .context("timed out writing tmux replay cutover commands")??;

        let modes_output = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "pane modes",
            pane,
        )
        .await?;
        let history_output = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "history snapshot",
            pane,
        )
        .await?;
        let visible_output = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "visible snapshot",
            pane,
        )
        .await?;
        let _cutover = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "live-output cutover",
            pane,
        )
        .await?;

        let modes_output = strip_command_output_terminator(&modes_output);
        let modes_text = String::from_utf8_lossy(modes_output);
        warn_once_about_missing_bracket_paste(&modes_text);
        let modes = PaneModes::parse(&modes_text);
        let snapshot = if modes.alternate_on {
            visible_output
        } else {
            history_output
        };
        let snapshot = strip_command_output_terminator(&snapshot);
        Ok((modes, normalize_capture(snapshot)))
    }

    /// Next event of interest, or `None` when the client exits (session
    /// killed, server gone). Notifications this stream has no use for —
    /// command replies, layout changes, window renames — are consumed and
    /// ignored; they are not terminal content and not flow control.
    ///
    /// BOTH output dialects are accepted unconditionally, in every call,
    /// rather than switching on whether `pause-after` was set: `%output`
    /// is what tmux emits without the flag (and what an older or
    /// differently-configured client would still send), `%extended-output`
    /// is what it emits with it. The two carry identical payloads and go
    /// through identical decoding — see [`decode_output_payload`] — so a
    /// stream that changes dialect mid-life, or interleaves the two, loses
    /// nothing.
    ///
    /// Reads BYTES, never lines-as-`String`: tmux's control-mode escaping
    /// only octal-escapes bytes below 0x20 (and backslash), so anything
    /// ≥ 0x80 crosses this stream raw. Decoding as UTF-8 would fail on
    /// any pane emitting binary or non-UTF-8 output — `cat` of a binary
    /// file — and one such byte would kill the terminal for good.
    ///
    /// NOT cancel-safe: a cancelled call can leave a partially-read line
    /// behind, which the next call discards — and, since the foreign-pane
    /// path below writes, a half-written filter command on the client's
    /// stdin, which would desynchronize any later command on it. Callers
    /// may only abandon this future on a path that tears the whole stream
    /// down (the stall detach does exactly that), never to resume reading
    /// afterwards. That was already the rule; the write only widens what
    /// breaking it would cost.
    ///
    /// Notifications about ANOTHER pane are chatter here, discarded with
    /// the same indifference as a `%layout-change`. That drop is the
    /// belt-and-braces half of the pane filter (see the type's own docs):
    /// without it, opening a tab would start spraying the tab's shell
    /// output into every already-attached terminal of that session, and a
    /// foreign pane's `%pause` would send this terminal through a
    /// reset-and-replay catch-up for bytes it was never carrying. Seeing
    /// one is also the trigger for asking tmux to stop sending that pane
    /// at all ([`Self::silence_late_pane`]), which is how a tab opened
    /// after this terminal attached ends up filtered server-side like
    /// every other.
    pub async fn next_output(&mut self) -> anyhow::Result<Option<OutputEvent>> {
        loop {
            if let Some(deadline) = self.query_strip_deadline {
                match tokio::time::timeout_at(deadline, self.reader.fill_buf()).await {
                    Ok(result) => {
                        result?;
                    }
                    Err(_) => {
                        return Ok(Some(OutputEvent::Bytes(self.flush_query_strip())));
                    }
                }
            }
            self.line.clear();
            let n = read_control_line(&mut self.reader, &mut self.line).await?;
            if n == 0 {
                let bytes = self.flush_query_strip();
                return if bytes.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(OutputEvent::Bytes(bytes)))
                };
            }
            // Classify under a borrow of the line buffer, then ACT on an
            // owned decision. The split exists because acting can need
            // `&mut self` (writing a filter command), which cannot coexist
            // with a classified borrow of one of self's own fields.
            let decision = {
                let OutputStream {
                    line,
                    pane,
                    passthrough,
                    query_strip,
                    query_strip_deadline,
                    silenced,
                    ..
                } = &mut *self;
                let own_pane = pane.as_bytes();
                match classify_control_line(strip_line_ending(line)) {
                    Some(ControlLine::Payload { pane, escaped }) if pane == own_pane => {
                        let segments = decode_output_payload(passthrough, escaped);
                        let mut bytes = Vec::new();
                        let mut ordinary_bytes = false;
                        for segment in segments {
                            if segment.passthrough {
                                // A wrapper bypassed tmux's terminal parser.
                                // Flush an ordinary candidate first so wrapper
                                // bytes cannot complete and be mistaken for it.
                                bytes.extend(query_strip.flush());
                                bytes.extend(segment.bytes);
                            } else {
                                ordinary_bytes |= !segment.bytes.is_empty();
                                bytes.extend(query_strip.feed(&segment.bytes));
                            }
                        }
                        if ordinary_bytes {
                            *query_strip_deadline = query_strip
                                .has_pending()
                                .then(|| tokio::time::Instant::now() + QUERY_STRIP_IDLE_FLUSH);
                        } else if !query_strip.has_pending() {
                            *query_strip_deadline = None;
                        }
                        if bytes.is_empty() {
                            // The decoder may await a wrapper continuation, or
                            // filtering may have consumed a query or retained
                            // its prefix. None has terminal bytes to emit yet.
                            Decision::Incomplete
                        } else {
                            Decision::Event(OutputEvent::Bytes(bytes))
                        }
                    }
                    Some(ControlLine::Paused { pane }) if pane == own_pane => {
                        Decision::Event(OutputEvent::Paused)
                    }
                    // Another window's pane, on the same session-attached
                    // control client. Deliberately NOT fed to the
                    // passthrough decoder: that decoder carries wrapper
                    // state across notifications, and mixing a stranger's
                    // bytes into it would corrupt a wrapper of ours that
                    // happened to be open at the time.
                    Some(
                        ControlLine::Payload { pane: other, .. }
                        | ControlLine::Paused { pane: other },
                    ) => {
                        // Copied only when this pane is new. A pane already
                        // filtered can still be mid-flight for a moment, and
                        // allocating per notification for one we have
                        // nothing left to do about would put the allocation
                        // rate back under a foreign pane's control.
                        if std::str::from_utf8(other).is_ok_and(|id| silenced.contains(id)) {
                            Decision::ForeignKnown
                        } else {
                            Decision::ForeignNew(other.to_vec())
                        }
                    }
                    Some(ControlLine::Exit { reason }) => {
                        Decision::Exit(String::from_utf8_lossy(reason).into_owned())
                    }
                    None => Decision::Chatter,
                }
            };
            match decision {
                Decision::Event(event) => return Ok(Some(event)),
                Decision::Exit(reason) => {
                    // Diagnostics only: the `Ok(None)` below is unchanged,
                    // and every caller still reads it as "this stream
                    // ended". What the reason buys is the difference
                    // between a tmux server that was killed and one that
                    // died on its own, which is otherwise erased here and
                    // unrecoverable afterwards.
                    self.exit_reason = Some(reason);
                    warn!(
                        pane = %self.pane,
                        // The stored copy rather than the local, so this
                        // field has a production reader and cannot drift
                        // from what the tests later report.
                        // Debug-formatted, and that is a security choice
                        // rather than a style one: tmux names the SESSION
                        // in several of its exit reasons, a pane inherits
                        // `$TMUX` and can rename its own session to
                        // anything at all — control characters included —
                        // so this string is pane-influenced text arriving
                        // at a log an operator reads in a terminal.
                        // `Display` would replay those bytes verbatim;
                        // `Debug` escapes them. `session` beside it is
                        // safe under `Display` because it is the
                        // supervisor's own stored name, never read back
                        // from tmux.
                        session = %self.session,
                        reason = ?self.exit_reason,
                        "tmux control client exited"
                    );
                    let bytes = self.flush_query_strip();
                    return if bytes.is_empty() {
                        Ok(None)
                    } else {
                        Ok(Some(OutputEvent::Bytes(bytes)))
                    };
                }
                Decision::Incomplete => {}
                Decision::ForeignKnown => self.foreign_dropped += 1,
                Decision::ForeignNew(pane) => {
                    self.foreign_dropped += 1;
                    self.silence_late_pane(&pane).await?;
                }
                Decision::Chatter => {
                    // The one piece of chatter that is not ignorable: a
                    // block terminator settles part of the filter-reply
                    // debt the catch-up path would otherwise mistake for
                    // its own first reply (see `pending_filter_replies`).
                    if self.pending_filter_replies > 0
                        && matches!(
                            parse_control_marker(strip_line_ending(&self.line)),
                            Some(ControlMarker::End(_) | ControlMarker::Error(_))
                        )
                    {
                        self.pending_filter_replies -= 1;
                    }
                }
            }
        }
    }

    /// Return a retained ordinary prefix at a stream boundary or idle limit.
    ///
    /// A prefix is only withheld while a later own-pane payload might complete
    /// a table entry. Once that wait ends, clearing both pieces of state keeps
    /// a later fresh payload from inheriting an obsolete deadline.
    fn flush_query_strip(&mut self) -> Vec<u8> {
        self.query_strip_deadline = None;
        self.query_strip.flush()
    }

    /// Lift a tmux-side pause and re-establish this client's stream as a
    /// reattach: snapshot the pane's modes and history, then continue it.
    ///
    /// The catch-up contract (PLAN_M2_5.md): the bytes tmux dropped while
    /// paused are exactly the bytes history can replay, so the resume path
    /// reuses the attach path's snapshot machinery rather than growing a
    /// second replay implementation — same command group, same block
    /// accounting, only the cutover command differs.
    ///
    /// Ordering is load-bearing and empirically verified (tmux 3.7b): the
    /// snapshot is taken while the pane is STILL paused, so no live output
    /// can interleave with the command group's reply blocks, and the
    /// continue is the group's LAST command, so the first live byte after
    /// its `%end` is the first byte past the snapshot. Continuing first and
    /// snapshotting after would race resumed output into
    /// [`read_command_block`], which would silently fold it into the
    /// captured history.
    ///
    /// The caller must reset the client's terminal before writing the
    /// returned prefill: the replay assumes an empty terminal, and this
    /// one lands in a terminal already showing the pre-pause bytes.
    pub async fn resume_paused_with_replay(&mut self) -> anyhow::Result<(PaneModes, Vec<u8>)> {
        // Discard any half-decoded passthrough wrapper left over from the
        // stream tmux just cut. `%pause` can land in the middle of a
        // wrapper split across notifications, and that partial state
        // belongs to bytes the client will never receive — carrying it
        // into the post-reset stream would make the decoder treat the
        // first fresh bytes as a wrapper continuation and swallow or
        // mangle them. The replay itself is capture-pane output, which
        // never contains a wrapper, so nothing is lost by clearing here.
        self.passthrough = PassthroughDecoder::default();
        // The query suffix belongs to the abandoned pre-pause live stream.
        // The terminal is reset and replayed before fresh output begins, so
        // retaining it could consume bytes from that new stream instead.
        self.query_strip = QueryStripper::default();
        self.query_strip_deadline = None;
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        // Start the positional reads from a known boundary. A pane filter
        // written moments before the `%pause` can still have its reply in
        // flight, and this group would otherwise read that reply as its
        // modes block and shift every block after it (see
        // `pending_filter_replies`). The pause is what makes this safe to
        // wait for: our own pane is quiet, so the only thing that can
        // arrive is the debt being settled.
        self.settle_filter_replies(deadline).await?;
        let cutover = continue_pane_command(&self.pane);
        self.snapshot_then_cutover(deadline, &cutover).await
    }

    /// Clear this client's queued pane blocks through an independent command.
    ///
    /// The output stream may be between any two bytes of an in-band command or
    /// reply when teardown wins its race. Targeting the client externally makes
    /// that partial protocol state irrelevant: the separate tmux process exits
    /// only after the server has applied client-wide `no-output`.
    async fn disable_output_before_shutdown(&mut self) -> anyhow::Result<()> {
        if self.output_disabled
            || self
                .child
                .try_wait()
                .context("checking whether the terminal-output client already exited")?
                .is_some()
        {
            return Ok(());
        }
        if let Err(error) = self
            .driver
            .disable_control_client_output(&self.client_target)
            .await
        {
            if self
                .child
                .try_wait()
                .context("rechecking whether the terminal-output client exited")?
                .is_some()
            {
                return Ok(());
            }
            return Err(error).context("disabling terminal output before shutdown");
        }
        self.output_disabled = true;
        Ok(())
    }

    /// Attempt the complete safe-boundary transition and process reap once.
    async fn try_shutdown(&mut self) -> anyhow::Result<()> {
        self.disable_output_before_shutdown().await?;
        shutdown_output_control_client(
            &mut self.child,
            self.stdin.take(),
            &mut self.reader,
            "terminal-output control client",
        )
        .await
    }

    /// Submit a raw command line on this client's stdin, for tests only.
    ///
    /// Exists so a test can force tmux into a state that is otherwise
    /// reachable only by timing — specifically `refresh-client -A
    /// <pane>:pause`, tmux's on-demand form of the same pane pause
    /// `pause-after` produces after a delay. Provoking that delay-driven
    /// pause deterministically is not possible from a test (it depends on
    /// how far tmux happens to have read ahead of a client that then
    /// stalls), so the on-demand form is what makes the catch-up path
    /// testable at all.
    ///
    /// Deliberately does NOT read the reply: the caller is expected to go
    /// on consuming the stream through [`Self::next_output`], which
    /// discards command-reply blocks like any other chatter. Production
    /// code must not use this — every real command this client sends is
    /// part of a command GROUP whose replies are read back in lockstep by
    /// [`Self::snapshot_then_cutover`].
    #[cfg(test)]
    async fn send_raw_command(&mut self, command: &str) -> anyhow::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .context("the test stream is shut down")?;
        stdin.write_all(command.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Read and discard control lines until this client's in-flight
    /// command reply closes with `%end`/`%error`. Tests only.
    ///
    /// The complement to [`Self::send_raw_command`]: tmux nests the
    /// `%pause`/`%continue` notification INSIDE the reply block of the
    /// command that caused it, so a test that stops reading the moment it
    /// sees the notification leaves the block's closing marker unread —
    /// and the replay group that follows reads reply blocks positionally,
    /// so one stray marker desynchronizes it. Waiting for the marker
    /// explicitly is what makes that deterministic; the obvious
    /// alternative (a short timeout on [`Self::next_output`], hoping it
    /// consumes the line) both races and cancels a read this type
    /// documents as not cancel-safe.
    #[cfg(test)]
    async fn drain_command_reply(&mut self) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        loop {
            self.line.clear();
            let read = read_control_line_before(
                &mut self.reader,
                &mut self.line,
                deadline,
                "test command reply",
            )
            .await?;
            if read == 0 {
                bail!("control client exited before its command reply closed");
            }
            if matches!(
                parse_control_marker(strip_line_ending(&self.line)),
                Some(ControlMarker::End(_) | ControlMarker::Error(_))
            ) {
                return Ok(());
            }
        }
    }

    /// Detach the control-mode client and wait for it to actually be gone.
    ///
    /// The external `no-output` acknowledgement is load-bearing. Killing or
    /// closing a client while tmux has a pane block queued for it can abort
    /// tmux 3.7b with `fatal: not enough data`; [`Self::try_shutdown`] first
    /// makes tmux discard that queue, then closes stdin and reaps the client.
    /// A bounded kill is safe only after that boundary and exists for a client
    /// that refuses the normal EOF exit.
    ///
    /// The client must be dead before another attaches: overlapping control
    /// clients reproducibly froze the newcomer's stream after replay. The
    /// mechanism was never pinned down (in isolation two attached control
    /// clients do both receive pane output), so callers depend on the ordering,
    /// not that explanation.
    #[allow(
        clippy::result_large_err,
        reason = "the error transfers the intact stream to caller-managed safe teardown retries"
    )]
    pub async fn shutdown(mut self) -> Result<(), OutputReaper> {
        match self.try_shutdown().await {
            Ok(()) => Ok(()),
            Err(error) => Err(OutputReaper {
                stream: self,
                last_error: format!("{error:#}"),
            }),
        }
    }
}

/// A runtime-owned retry of an output client's safe shutdown boundary.
///
/// Returning an ordinary error would drop `OutputStream`, whose
/// `kill_on_drop` child can invalidate a pane block before tmux has discarded
/// it. This value retains every process handle instead. The supervisor publishes
/// a per-terminal barrier before letting a replacement attach, while this task
/// retries until the server acknowledges `no-output` and the client is reaped.
pub struct OutputReaper {
    stream: OutputStream,
    last_error: String,
}

impl OutputReaper {
    /// Retry forever rather than turn an unconfirmed client into a safe one by
    /// assertion. The registry waiting on this task makes the failure visible
    /// and keeps only the affected terminal closed to replacement.
    pub async fn run(mut self) -> Result<(), std::sync::Arc<str>> {
        let mut failures = 1u32;
        loop {
            if failures.is_power_of_two() {
                warn!(
                    pane = %self.stream.pane,
                    error = %self.last_error,
                    failures,
                    "terminal-output client cleanup is not yet safe; retrying"
                );
            }
            tokio::time::sleep(control_cleanup_retry_delay(failures)).await;
            match self.stream.try_shutdown().await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    failures = failures.saturating_add(1);
                    self.last_error = format!("{error:#}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::super::{CONTROL_EXCHANGE_TIMEOUT, DisableOutputGate, PANE_LIST_TIMEOUT};
    use super::*;
    use std::sync::Arc;
    use tokio::process::Command;

    /// Run a scripted control-mode transcript through the exact
    /// classification and decoding `OutputStream::read_next_event` uses,
    /// collecting the events a forwarder would have observed.
    ///
    /// Line-per-line rather than through a reader on purpose: framing is
    /// already pinned by `control_codec`'s `read_control_line` tests, and
    /// keeping this helper free of any I/O is what lets the notification
    /// GRAMMAR be tested without a tmux server. One shared
    /// [`PassthroughDecoder`] across the whole transcript is load-bearing,
    /// not incidental — it is what makes a wrapper split across
    /// notifications (or across dialects) observable here at all.
    fn events_from_transcript(transcript: &[&[u8]]) -> Vec<OutputEvent> {
        let mut decoder = PassthroughDecoder::default();
        let mut events = Vec::new();
        for line in transcript {
            match classify_control_line(line) {
                Some(ControlLine::Payload { escaped, .. }) => {
                    let bytes = decode_output_payload(&mut decoder, escaped)
                        .into_iter()
                        .flat_map(|segment| segment.bytes)
                        .collect::<Vec<_>>();
                    if !bytes.is_empty() {
                        events.push(OutputEvent::Bytes(bytes));
                    }
                }
                Some(ControlLine::Paused { .. }) => events.push(OutputEvent::Paused),
                Some(ControlLine::Exit { .. }) => break,
                None => {}
            }
        }
        events
    }

    /// `%extended-output` — the dialect `pause-after` switches tmux into,
    /// and therefore the ONLY dialect a production attachment sees after
    /// this milestone. Pins the documented shape (`pane-id age ... :
    /// value`), the extensibility rule (unknown fields between the age
    /// and the `:` are ignored rather than counted, so a future tmux
    /// argument cannot shift the payload), and that a payload containing
    /// colons and spaces of its own is not mistaken for the separator.
    #[farhelm_testtrace::test]
    fn extended_output_is_decoded_with_and_without_extra_fields() {
        let events = events_from_transcript(&[
            br"%extended-output %0 0 : plain\015\012",
            b"%extended-output %0 12 future args here : with:colons and spaces",
            b"%extended-output %0 0 :",
        ]);
        assert_eq!(
            events,
            vec![
                OutputEvent::Bytes(b"plain\r\n".to_vec()),
                OutputEvent::Bytes(b"with:colons and spaces".to_vec()),
            ],
            "an empty payload yields no event at all, and neither extra fields nor colons in \
             the payload may shift the split"
        );
    }

    /// Plain `%output` must keep working unchanged: it is what tmux emits
    /// with no `pause-after` set, and the parser accepts both dialects
    /// unconditionally rather than switching on client state. Interleaving
    /// the two in one transcript is deliberate — passthrough-wrapper state
    /// is carried ACROSS notifications, so a dialect boundary landing
    /// inside a wrapper must decode exactly as if it never happened.
    #[farhelm_testtrace::test]
    fn both_output_dialects_share_one_decoder() {
        let events = events_from_transcript(&[
            br"%output %0 before\033Ptmux;\033\033]52;c;",
            br"%extended-output %0 0 : aGk=\007\033\134after",
        ]);
        assert_eq!(
            events,
            vec![
                // The wrapper opens in the `%output` notification and its
                // payload so far (an un-doubled ESC, then the OSC opener)
                // comes out with it; the rest, including the `ESC \` that
                // CLOSES the wrapper, arrives in the `%extended-output`
                // one and must decode as a continuation rather than as a
                // fresh, unwrapped chunk.
                OutputEvent::Bytes(b"before\x1b]52;c;".to_vec()),
                OutputEvent::Bytes(b"aGk=\x07after".to_vec()),
            ],
            "a passthrough wrapper opened in one dialect must close correctly in the other"
        );
    }

    /// `%pause` must surface as an event; `%continue` must NOT, and must
    /// not disturb the output around it.
    ///
    /// The asymmetry is the point. A swallowed `%pause` is the worst
    /// outcome in this file — tmux has cut the stream, the bytes it drops
    /// are gone from the live path forever, and nothing downstream would
    /// know to replay them, so the terminal would just silently miss a
    /// chunk. `%continue`, by contrast, is pure acknowledgement: it
    /// arrives inside the reply block of the very command that requested
    /// it, so nothing ever waits on it, and surfacing it would only give
    /// callers a variant they must remember to ignore. This pins that it
    /// is discarded like any other chatter AND that discarding it does not
    /// swallow or reorder the adjacent pane output.
    #[farhelm_testtrace::test]
    fn pause_surfaces_as_an_event_while_continue_is_discarded_intact() {
        let events = events_from_transcript(&[
            b"%extended-output %0 0 : a",
            b"%pause %0",
            b"%window-renamed @0 sh",
            b"%continue %0",
            b"%extended-output %0 0 : b",
        ]);
        assert_eq!(
            events,
            vec![
                OutputEvent::Bytes(b"a".to_vec()),
                OutputEvent::Paused,
                OutputEvent::Bytes(b"b".to_vec()),
            ],
            "`%continue` and unrelated notifications alike must be discarded without \
             disturbing the pane bytes on either side of them"
        );
    }

    /// The pane filter driven through the PRODUCTION path —
    /// [`OutputStream::next_output`] itself, over a real reader — rather
    /// than through `classify_control_line` alone.
    ///
    /// The grammar test above pins which lines carry which pane; this
    /// pins what the method built on it actually EMITS, which is a
    /// different claim and the one that matters. Two properties only this
    /// shape can show:
    ///
    /// - A passthrough wrapper of OUR pane, split across notifications,
    ///   survives foreign payloads interleaved into the split. The
    ///   decoder carries wrapper state across calls, so a filter that
    ///   dropped foreign lines only AFTER decoding would feed a
    ///   stranger's bytes into our half-open wrapper and corrupt it.
    /// - A foreign `%pause` produces no event at all. Surfacing it would
    ///   send this terminal through a full reset-and-replay catch-up for
    ///   bytes it never carried.
    ///
    /// An [`OutputStream`] over a canned transcript, plus the process
    /// standing in for tmux on its WRITE half.
    ///
    /// Both halves are real child processes rather than in-memory pipes,
    /// and both for reasons: the read half must be a `ChildStdout` because
    /// that is what the production type reads and what EOFs the way a
    /// control client's exit does, and the write half must be a pipe
    /// somebody is emptying because `next_output` now WRITES (the late
    /// pane filter). A write half nothing drained would fill at 64 KiB and
    /// park the stream under test — a hang, not an assertion failure.
    ///
    /// The returned `Child` is the write half's holder: dropping it kills
    /// that process and closes the pipe, so callers must keep it alive for
    /// as long as they drive the stream.
    fn stream_over_transcript(transcript: &'static [u8]) -> (OutputStream, Child) {
        let mut feeder = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawning the transcript feeder");
        let mut feeder_stdin = feeder.stdin.take().expect("piped stdin");
        let feeder_stdout = feeder.stdout.take().expect("piped stdout");
        tokio::spawn(async move {
            let _ = feeder_stdin.write_all(transcript).await;
        });
        let mut command_sink = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawning the command sink");
        let stdin = command_sink.stdin.take().expect("piped stdin");
        let stream = OutputStream {
            // This fixture exercises parsing over process-backed pipes, not
            // tmux teardown. Marking its synthetic client already disabled
            // keeps shutdown-only server state out of the fixture's contract.
            driver: TmuxDriver::new(std::path::Path::new("")),
            client_target: String::new(),
            output_disabled: true,
            child: feeder,
            stdin: Some(stdin),
            reader: BufReader::new(feeder_stdout),
            line: Vec::new(),
            passthrough: PassthroughDecoder::default(),
            query_strip: QueryStripper::default(),
            query_strip_deadline: None,
            pane: "%0".to_string(),
            session: "fh-s".to_string(),
            silenced: HashSet::new(),
            pending_filter_replies: 0,
            foreign_dropped: 0,
            // This helper drives `next_output` directly against a canned
            // transcript, never a real tmux, so neither budget is ever
            // consulted — production defaults are just the least
            // surprising filler.
            exchange_timeout: CONTROL_EXCHANGE_TIMEOUT,
            pane_list_timeout: PANE_LIST_TIMEOUT,
            exit_reason: None,
        };
        (stream, command_sink)
    }

    /// Build the production reader around a pipe whose writer the test owns.
    ///
    /// EOF and idle behavior depend on whether the input stays open, which a
    /// prewritten transcript cannot express. The retained writer lets callers
    /// stage control lines around a pending terminal prefix without replacing
    /// the `ChildStdout` production code reads from.
    fn stream_over_open_pipe() -> (OutputStream, Child, tokio::process::ChildStdin) {
        let mut feeder = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawning the open transcript feeder");
        let feeder_stdin = feeder.stdin.take().expect("piped feeder stdin");
        let feeder_stdout = feeder.stdout.take().expect("piped feeder stdout");
        let mut command_sink = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawning the command sink");
        let stdin = command_sink.stdin.take().expect("piped command sink stdin");
        let stream = OutputStream {
            driver: TmuxDriver::new(std::path::Path::new("")),
            client_target: String::new(),
            output_disabled: true,
            child: feeder,
            stdin: Some(stdin),
            reader: BufReader::new(feeder_stdout),
            line: Vec::new(),
            passthrough: PassthroughDecoder::default(),
            query_strip: QueryStripper::default(),
            query_strip_deadline: None,
            pane: "%0".to_string(),
            session: "fh-s".to_string(),
            silenced: HashSet::new(),
            pending_filter_replies: 0,
            foreign_dropped: 0,
            exchange_timeout: CONTROL_EXCHANGE_TIMEOUT,
            pane_list_timeout: PANE_LIST_TIMEOUT,
            exit_reason: None,
        };
        (stream, command_sink, feeder_stdin)
    }

    /// Write one raw own-pane payload as a complete control-mode line.
    ///
    /// The control codec accepts raw ESC bytes as well as tmux's octal form;
    /// using the raw form keeps split-boundary tests aligned with the bytes the
    /// passthrough decoder actually receives.
    async fn feed_own_payload(writer: &mut tokio::process::ChildStdin, payload: &[u8]) {
        writer
            .write_all(b"%output %0 ")
            .await
            .expect("writing output marker");
        writer
            .write_all(payload)
            .await
            .expect("writing pane payload");
        writer
            .write_all(b"\n")
            .await
            .expect("terminating output line");
        writer.flush().await.expect("flushing pane payload");
    }

    #[farhelm_testtrace::test]
    async fn next_output_emits_only_its_own_panes_events_through_a_split_wrapper() {
        let transcript: &[u8] = b"%output %0 before\\033Ptmux;\\033\\033]52;c;\n\
                                  %output %7 foreign-one\n\
                                  %extended-output %7 0 : foreign-two\n\
                                  %pause %7\n\
                                  %window-renamed @3 something\n\
                                  %extended-output %0 0 : aGk=\\007\\033\\134after\n\
                                  %pause %0\n\
                                  %exit\n";
        let (mut stream, _command_sink) = stream_over_transcript(transcript);
        let mut events = Vec::new();
        while let Some(event) = stream.next_output().await.expect("stream must not fail") {
            events.push(event);
        }
        assert_eq!(
            events,
            vec![
                // The wrapper opened in the first notification; its
                // payload so far comes out with it.
                OutputEvent::Bytes(b"before\x1b]52;c;".to_vec()),
                // ...and CLOSES correctly in ours, three foreign
                // notifications later, proving none of them reached the
                // decoder.
                OutputEvent::Bytes(b"aGk=\x07after".to_vec()),
                // Only OUR pane's pause surfaces.
                OutputEvent::Paused,
            ],
            "a foreign pane's payloads and pauses must be dropped before decoding, and this \
             pane's split wrapper must close intact across them"
        );
    }

    /// A passthrough query must reach the browser because tmux forwarded it.
    ///
    /// The identical bare query is stripped because tmux parsed and answered
    /// it. Testing every wrapper split pins both the provenance boundary and
    /// the decoder's existing ability to carry that boundary across lines.
    #[farhelm_testtrace::test]
    async fn passthrough_queries_bypass_stripping_across_every_wrapper_split() {
        let wrapped = b"\x1bPtmux;\x1b\x1b[6n\x1b\\";
        for split in 0..=wrapped.len() {
            let (mut stream, _command_sink, mut writer) = stream_over_open_pipe();
            feed_own_payload(&mut writer, &wrapped[..split]).await;
            feed_own_payload(&mut writer, &wrapped[split..]).await;
            writer
                .write_all(b"%exit\n")
                .await
                .expect("writing stream exit");
            drop(writer);

            let mut observed = Vec::new();
            while let Some(OutputEvent::Bytes(bytes)) =
                stream.next_output().await.expect("reading wrapped query")
            {
                observed.extend(bytes);
            }
            assert_eq!(
                observed, b"\x1b[6n",
                "wrapper split {split} must preserve a tmux-forwarded query"
            );
        }

        let (mut stream, _command_sink) = stream_over_transcript(b"%output %0 \x1b[6n\n%exit\n");
        assert!(
            stream
                .next_output()
                .await
                .expect("reading bare query")
                .is_none(),
            "the ordinary query must be removed because tmux answers it"
        );
    }

    /// `%exit` is a stream boundary, so a retained prefix must be emitted
    /// once before callers learn that the control client ended.
    #[farhelm_testtrace::test]
    async fn exit_flushes_a_retained_query_prefix_before_end_of_stream() {
        let (mut stream, _command_sink) = stream_over_transcript(b"%output %0 \x1b[\n%exit\n");
        assert_eq!(
            stream.next_output().await.expect("reading exit flush"),
            Some(OutputEvent::Bytes(b"\x1b[".to_vec()))
        );
        assert_eq!(
            stream.next_output().await.expect("reading stream end"),
            None
        );
    }

    /// An open pipe with no further own-pane bytes must release a pending
    /// prefix at the idle deadline instead of waiting forever for EOF.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn idle_flushes_a_retained_query_prefix_from_an_open_stream() {
        let (mut stream, _command_sink, mut writer) = stream_over_open_pipe();
        feed_own_payload(&mut writer, b"\x1b[").await;
        let read = tokio::spawn(async move {
            let event = stream.next_output().await.expect("reading idle flush");
            (stream, event)
        });
        tokio::task::yield_now().await;
        tokio::time::advance(QUERY_STRIP_IDLE_FLUSH).await;
        let (stream, event) = read.await.expect("joining idle reader");
        assert_eq!(event, Some(OutputEvent::Bytes(b"\x1b[".to_vec())));
        drop(writer);
        shutdown_test_stream(stream).await;
    }

    /// EOF settles a retained prefix as ordinary output before `next_output`
    /// reports the end, matching the `%exit` boundary contract.
    #[farhelm_testtrace::test]
    async fn eof_flushes_a_retained_query_prefix_before_end_of_stream() {
        let (mut stream, _command_sink, mut writer) = stream_over_open_pipe();
        feed_own_payload(&mut writer, b"\x1b[").await;
        drop(writer);
        assert_eq!(
            stream.next_output().await.expect("reading EOF flush"),
            Some(OutputEvent::Bytes(b"\x1b[".to_vec()))
        );
        assert_eq!(stream.next_output().await.expect("reading EOF"), None);
    }

    /// Shared-client chatter cannot extend a prefix's own-pane idle deadline.
    ///
    /// This uses paused time so four unrelated notifications are deliberately
    /// spaced inside the 50 ms window without making the regression wall-clock
    /// sensitive. A deadline restarted by each line would remain pending at
    /// the final advance; only the original own-pane deadline may govern it.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn unrelated_control_traffic_does_not_starve_the_idle_flush() {
        let (mut stream, _command_sink, mut writer) = stream_over_open_pipe();
        feed_own_payload(&mut writer, b"\x1b[").await;
        let read = tokio::spawn(async move {
            let event = stream.next_output().await.expect("reading idle flush");
            (stream, event)
        });
        tokio::task::yield_now().await;
        for _ in 0..4 {
            tokio::time::advance(std::time::Duration::from_millis(10)).await;
            writer
                .write_all(b"%window-renamed @0 unrelated\n")
                .await
                .expect("writing unrelated control line");
            writer
                .flush()
                .await
                .expect("flushing unrelated control line");
            tokio::task::yield_now().await;
        }
        tokio::time::advance(std::time::Duration::from_millis(10)).await;
        let (stream, event) = read.await.expect("joining idle reader");
        assert_eq!(event, Some(OutputEvent::Bytes(b"\x1b[".to_vec())));
        drop(writer);
        shutdown_test_stream(stream).await;
    }

    /// The exact attach-cutover text, pinned like every other generated
    /// tmux command in this module. Both flags ride one comma-separated
    /// `-f` list, and the `!` prefix is what CLEARS `no-output` rather
    /// than setting a flag by that name — get either detail wrong and the
    /// client silently either never receives output or never gets the
    /// `pause-after` backstop, neither of which fails loudly.
    ///
    /// The filtered form is pinned in the SAME test because the two must
    /// stay one invocation: splitting the `-A` arguments into a command of
    /// their own is exactly the mistake that loses the filter to tmux's
    /// per-pane state reset (see [`attach_cutover_command`]), and it would
    /// still read perfectly well at the call site.
    #[farhelm_testtrace::test]
    fn attach_cutover_sets_pause_after_while_clearing_no_output() {
        assert_eq!(
            attach_cutover_command(&[]),
            format!("refresh-client -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}")
        );
        assert_eq!(
            attach_cutover_command(&["%1".to_string(), "%12".to_string()]),
            format!(
                "refresh-client -A \"%1:off\" -A \"%12:off\" \
                 -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}"
            )
        );
    }

    /// The pane filter's exact arguments: one ` -A <pane>:off` per pane,
    /// quoted for the `:`, and nothing at all for an empty list.
    ///
    /// The empty case is not pedantry — it is what keeps a session with no
    /// other panes from turning its cutover into a differently-spelled
    /// command than the one the test above pins.
    #[farhelm_testtrace::test]
    fn the_pane_filter_names_every_foreign_pane_once() {
        assert_eq!(
            silence_pane_args(&["%1".to_string(), "%12".to_string()]),
            " -A \"%1:off\" -A \"%12:off\""
        );
        assert_eq!(silence_pane_args(&[]), "");
    }

    /// Live pane filters stay below tmux's argument ceiling and preserve order.
    ///
    /// tmux refuses a command line of roughly a thousand arguments, and
    /// each post-cutover pane costs four: `-A <pane>:pause` followed by
    /// `-A <pane>:off`. Without chunking, a session with enough tabs would
    /// make every attach on it fail outright. The boundary is checked
    /// exactly at the live cap and one past it, and every pane must retain
    /// both transitions in the required order.
    #[farhelm_testtrace::test]
    fn live_pane_filters_are_ordered_and_chunked_below_tmuxs_argument_ceiling() {
        let panes = |n: usize| (0..n).map(|i| format!("%{i}")).collect::<Vec<_>>();
        let live_cap = MAX_CUTOVER_PANE_FILTERS / 2;

        let at_cap = panes(live_cap);
        let commands = silence_live_pane_commands(&at_cap);
        assert_eq!(commands.len(), 1, "the cap itself must be one command");

        let over_cap = panes(live_cap + 1);
        let commands = silence_live_pane_commands(&over_cap);
        assert_eq!(commands.len(), 2, "one past the cap must split");
        for command in &commands {
            assert!(
                command.starts_with("refresh-client "),
                "a live pane filter must be a refresh-client command: {command}"
            );
            assert!(
                command.matches(" -A ").count() <= MAX_CUTOVER_PANE_FILTERS,
                "a command exceeded the established argument budget: {command}"
            );
        }
        // A split that drops its remainder or reverses either transition
        // silently restores the old crash path for that pane.
        let rendered = commands.join(" ");
        for pane in &over_cap {
            assert_eq!(
                rendered
                    .matches(&format!("-A \"{pane}:pause\" -A \"{pane}:off\""))
                    .count(),
                1,
                "{pane} must be paused and filtered exactly once, in order"
            );
        }
        assert!(silence_live_pane_commands(&[]).is_empty());
    }

    /// The per-attachment memo of already-filtered panes stays bounded
    /// however many distinct panes speak to one terminal.
    ///
    /// A session can churn through tabs indefinitely — open, close, open —
    /// and every one of them that ever emits a byte lands in this set.
    /// Unbounded, that is a slow leak keyed by a number the user controls,
    /// on a struct that lives as long as an attachment does. The memo is
    /// only an optimization (see [`MAX_SILENCED_PANES`]), so forgetting is
    /// free; what this pins is that it forgets rather than grows.
    #[farhelm_testtrace::test]
    async fn the_filtered_pane_memo_stays_bounded_under_pane_churn() {
        // One line per pane, more of them than the cap, so a set that
        // never forgot would end up strictly larger than it.
        static TRANSCRIPT: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
            let mut transcript = Vec::new();
            for pane in 1..=(MAX_SILENCED_PANES + 50) {
                transcript.extend_from_slice(format!("%output %{pane} x\n").as_bytes());
            }
            transcript.extend_from_slice(b"%exit\n");
            transcript
        });
        let (mut stream, _command_sink) = stream_over_transcript(&TRANSCRIPT);
        while stream
            .next_output()
            .await
            .expect("the stream must not fail")
            .is_some()
        {}
        assert!(
            stream.silenced.len() <= MAX_SILENCED_PANES,
            "the filtered-pane memo grew to {} entries",
            stream.silenced.len()
        );
        assert_eq!(
            stream.foreign_dropped,
            (MAX_SILENCED_PANES + 50) as u64,
            "every foreign notification must still have been counted and dropped"
        );
    }

    /// Pane ids are shape-checked before they are interpolated into a
    /// `refresh-client` command line.
    ///
    /// They arrive on the same stream as pane CONTENT — the notification
    /// grammar is the only thing separating the two — so this is a trust
    /// boundary, not a formatting nicety: without it a crafted
    /// notification could put arbitrary text into a command this process
    /// sends to its own tmux server.
    #[farhelm_testtrace::test]
    fn only_real_pane_ids_reach_the_pane_filter() {
        for good in ["%0", "%7", "%1234"] {
            assert!(is_pane_id_shaped(good.as_bytes()), "{good} is a pane id");
        }
        for bad in ["", "%", "0", "@1", "%1a", "%1 -A", "%1\"", " %1"] {
            assert!(
                !is_pane_id_shaped(bad.as_bytes()),
                "{bad:?} must not reach a command line"
            );
        }
    }

    /// The exact continue text. `-A <pane>:continue` is tmux's pane-state
    /// form; the pane id keeps its leading `%` and the argument is quoted
    /// because of the `:`. A wrong spelling here would surface only as a
    /// pane that never resumes after a stall.
    #[farhelm_testtrace::test]
    fn continue_command_uses_the_pane_state_form() {
        assert_eq!(
            continue_pane_command("%7"),
            "refresh-client -A \"%7:continue\""
        );
    }

    /// Late filtering discards queued output before turning the pane off.
    ///
    /// The order is the tmux 3.3–3.6 crash workaround: direct `off` can
    /// leave a queued block pointing at bytes tmux has already freed, while
    /// `pause` clears that queue first. Keeping both states in one command
    /// prevents pane output from reopening the vulnerable gap.
    #[farhelm_testtrace::test]
    fn late_pane_filter_pauses_before_turning_output_off() {
        assert_eq!(
            silence_live_pane_command(&["%7".to_string()]),
            "refresh-client -A \"%7:pause\" -A \"%7:off\""
        );
    }

    /// Abandoning a provisional replay client performs the safe transition.
    ///
    /// Attach can be cancelled after the replay cutover enabled output but
    /// before an input client or forwarder exists. The provisional guard must
    /// keep that client alive through the external `no-output` acknowledgement,
    /// then reap it without taking the private tmux server down.
    #[farhelm_testtrace::test]
    async fn an_abandoned_replay_candidate_is_reaped_after_no_output() {
        let mut server = ScratchServer::start().await;
        let pane = server
            .driver
            .create_session(
                "fh-abandoned-replay",
                "/",
                80,
                24,
                &[],
                &ticking_pane("ABANDONED"),
            )
            .await
            .expect("session");
        let gate = Arc::new(DisableOutputGate::default());
        server.driver.disable_output_gate = Some(Arc::clone(&gate));
        let candidate = server
            .driver
            .open_replay_stream_candidate("fh-abandoned-replay", &pane)
            .await
            .expect("replay candidate");
        let target = candidate
            .stream
            .as_ref()
            .expect("candidate stream")
            .stream
            .as_ref()
            .expect("output stream")
            .client_target
            .clone();

        drop(candidate);
        tokio::time::timeout(std::time::Duration::from_secs(10), gate.entered.notified())
            .await
            .expect("candidate cleanup reaches the external boundary");
        assert!(
            server
                .driver
                .run(&["list-clients", "-F", "#{client_name}"])
                .await
                .expect("listing clients before acknowledgement")
                .lines()
                .any(|name| name == target),
            "the candidate client closed before no-output was acknowledged"
        );

        gate.release.notify_one();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            gate.acknowledged.notified(),
        )
        .await
        .expect("candidate cleanup acknowledges no-output");
        gate.finish.notify_one();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let clients = server
                .driver
                .run(&["list-clients", "-F", "#{client_name}"])
                .await
                .expect("listing clients after acknowledgement");
            if !clients.lines().any(|name| name == target) {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the abandoned output client was not reaped"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(
            server
                .driver
                .has_session("fh-abandoned-replay")
                .await
                .expect("checking the private server after candidate cleanup"),
            "candidate cleanup must not abort the private tmux server"
        );
    }

    /// Shutdown establishes `no-output` outside a dirty positional exchange.
    ///
    /// A forwarder can be cancelled after writing a multi-command replay group
    /// but before reading any reply. An in-band shutdown command would then
    /// mistake the first old `%end` for its own acknowledgement and close the
    /// output client while tmux still owns queued pane blocks. Holding the
    /// independent command immediately before and after its acknowledgement
    /// proves both sides of the boundary: the output client remains alive
    /// until `no-output` is visible, then it is reaped without taking down the
    /// private server.
    #[farhelm_testtrace::test]
    async fn shutdown_acks_no_output_despite_unread_positional_replies() {
        let mut server = ScratchServer::start().await;
        let pane = server
            .driver
            .create_session(
                "fh-shutdown-boundary",
                "/",
                80,
                24,
                &[],
                &ticking_pane("BOUNDARY"),
            )
            .await
            .expect("session");
        let gate = Arc::new(DisableOutputGate::default());
        server.driver.disable_output_gate = Some(Arc::clone(&gate));
        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-shutdown-boundary", &pane)
            .await
            .expect("replay stream");
        let target = stream.client_target.clone();

        // Queue four replies and prove the group ran without consuming any of
        // them on this client's stdout. This is the interrupted replay shape
        // that makes an in-band shutdown acknowledgement ambiguous.
        let waiter_driver = server.driver.clone();
        let waiter = tokio::spawn(async move {
            waiter_driver
                .run(&["wait-for", "fh-shutdown-replies-ready"])
                .await
        });
        stream
            .send_raw_command(
                "display-message -p '#{pane_id}' ; \
                 capture-pane -p -t : ; \
                 display-message -p '#{session_name}' ; \
                 wait-for -S fh-shutdown-replies-ready",
            )
            .await
            .expect("queueing an unread four-block exchange");
        waiter
            .await
            .expect("joining the group sentinel")
            .expect("the group sentinel must run");

        let shutdown = tokio::spawn(async move { shutdown_test_stream(stream).await });
        tokio::time::timeout(std::time::Duration::from_secs(10), gate.entered.notified())
            .await
            .expect("shutdown must reach the external boundary");
        assert!(
            server
                .driver
                .run(&["list-clients", "-F", "#{client_name}"])
                .await
                .expect("listing clients before the boundary")
                .lines()
                .any(|name| name == target),
            "the output client closed before the safe transition began"
        );

        gate.release.notify_one();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            gate.acknowledged.notified(),
        )
        .await
        .expect("the external command must acknowledge no-output");
        let clients = server
            .driver
            .run(&["list-clients", "-F", "#{client_name}|#{client_flags}"])
            .await
            .expect("inspecting the acknowledged client");
        let target_line = clients
            .lines()
            .find(|line| line.starts_with(&format!("{target}|")))
            .expect("the client must remain alive until shutdown receives the acknowledgement");
        assert!(
            target_line
                .split_once('|')
                .is_some_and(|(_, flags)| { flags.split(',').any(|flag| flag == "no-output") }),
            "the external command returned without applying no-output: {target_line}"
        );
        assert!(
            !shutdown.is_finished(),
            "the output client was reaped before the acknowledgement returned"
        );

        gate.finish.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(10), shutdown)
            .await
            .expect("shutdown must finish after acknowledgement")
            .expect("joining output shutdown");
        let clients = server
            .driver
            .run(&["list-clients", "-F", "#{client_name}"])
            .await
            .expect("listing clients after shutdown");
        assert!(
            clients.lines().all(|name| name != target),
            "the acknowledged output client was not reaped"
        );
        assert!(
            server
                .driver
                .run(&["has-session", "-t", "fh-shutdown-boundary"])
                .await
                .is_ok(),
            "safe output-client shutdown must not abort the private server"
        );
    }

    /// Every completed reply boundary of a four-command exchange is safe to stop at.
    ///
    /// Pause recovery reads its replies positionally, so cooperative teardown
    /// can win before the first block or after any one of the four. Each case
    /// leaves a different amount of old protocol state on the output client's
    /// stdout. Reopening a fresh client per boundary proves shutdown never
    /// depends on how many of those old replies the interrupted reader happened
    /// to consume.
    #[farhelm_testtrace::test]
    async fn shutdown_survives_every_four_block_reply_boundary() {
        let server = ScratchServer::start().await;
        let pane = server
            .driver
            .create_session(
                "fh-shutdown-boundaries",
                "/",
                80,
                24,
                &[],
                &["sh".to_string(), "-c".to_string(), "sleep 300".to_string()],
            )
            .await
            .expect("session");

        for replies_read in 0..=4 {
            let (_modes, _prefill, mut stream) = server
                .driver
                .open_replay_stream("fh-shutdown-boundaries", &pane)
                .await
                .unwrap_or_else(|error| {
                    panic!("opening boundary-{replies_read} stream: {error:#}")
                });
            let sentinel = format!("fh-shutdown-boundary-{replies_read}");
            let waiter_driver = server.driver.clone();
            let waiter_sentinel = sentinel.clone();
            let waiter =
                tokio::spawn(
                    async move { waiter_driver.run(&["wait-for", &waiter_sentinel]).await },
                );
            stream
                .send_raw_command(&format!(
                    "display-message -p '#{{pane_id}}' ; \
                     capture-pane -p -t : ; \
                     display-message -p '#{{session_name}}' ; \
                     wait-for -S {sentinel}"
                ))
                .await
                .unwrap_or_else(|error| {
                    panic!("queueing boundary-{replies_read} exchange: {error:#}")
                });
            waiter
                .await
                .expect("joining the boundary sentinel")
                .expect("the boundary exchange must run");

            let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
            for block in 0..replies_read {
                read_command_block(
                    &mut stream.reader,
                    &mut stream.line,
                    deadline,
                    "shutdown-boundary test exchange",
                    &pane,
                )
                .await
                .unwrap_or_else(|error| {
                    panic!(
                        "reading block {block} before boundary {replies_read} shutdown: {error:#}"
                    )
                });
            }

            shutdown_test_stream(stream).await;
            assert!(
                server
                    .driver
                    .run(&["has-session", "-t", "fh-shutdown-boundaries"])
                    .await
                    .is_ok(),
                "shutdown after {replies_read} replies aborted the private server"
            );
        }
    }

    /// The replay group's command list and ORDER are the contract
    /// `snapshot_then_cutover` reads reply blocks against, and both the
    /// attach and catch-up paths depend on it being identical apart from
    /// the final cutover. Pinned against the complete string so a
    /// reordering — which would silently pair the modes reply with the
    /// history capture — cannot pass, and so nothing can be inserted
    /// between the visible capture and the cutover: their adjacency is
    /// what makes the cutover lossless (see the function's own comment
    /// for the issue-4 attempt this refuses to readmit).
    #[farhelm_testtrace::test]
    fn replay_command_group_differs_only_in_its_cutover() {
        let attach = replay_command_group("fh-s", "%1", &attach_cutover_command(&[]));
        let resume = replay_command_group("fh-s", "%1", &continue_pane_command("%1"));
        // Every pane target is the QUOTED session-paired form: a bare
        // `%1` would resolve server-wide, and a pane id that went stale
        // across a tmux-server restart would then capture another
        // session's terminal into this one.
        assert_eq!(
            attach,
            format!(
                "display-message -p -t \"=fh-s:.%1\" '{PANE_MODE_FORMAT}' ; \
                 capture-pane -p -e -N -t \"=fh-s:.%1\" -S -{HISTORY_LIMIT} ; \
                 capture-pane -p -e -N -t \"=fh-s:.%1\" ; \
                 refresh-client -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}\n"
            )
        );
        let common = attach
            .strip_suffix(&format!("{}\n", attach_cutover_command(&[])))
            .expect("attach group ends with its cutover");
        assert_eq!(
            resume,
            format!("{common}{}\n", continue_pane_command("%1")),
            "the two groups must share every snapshot command verbatim"
        );
    }
    /// The catch-up path against a REAL tmux: a paused pane must surface
    /// as [`OutputEvent::Paused`], and
    /// [`OutputStream::resume_paused_with_replay`] must both hand back a
    /// usable replay and get the live stream flowing again.
    ///
    /// This is the one place the whole tmux-facing half of PLAN_M2_5.md's
    /// deep-stall recovery is exercised end to end, and it earns a real
    /// tmux because every interesting part is tmux's behavior, not ours:
    /// that a paused pane goes quiet, that a snapshot command group can
    /// still run cleanly on that same client while it is paused (the
    /// ordering the resume path depends on — continuing first would race
    /// resumed output into the snapshot), and that the continue command's
    /// `%end` is where live output picks back up.
    ///
    /// The pause is forced with tmux's on-demand `refresh-client -A
    /// <pane>:pause` rather than by starving the client until
    /// `pause-after` fires. That is not a shortcut, it is the only
    /// DETERMINISTIC option: whether the delay-driven pause triggers
    /// depends on how far tmux happens to have read ahead of a client that
    /// then stalls, and a client that was caught up when it stopped
    /// reading leaves tmux nothing buffered to age — tmux then throttles
    /// the pane instead and never pauses at all. Both outcomes were
    /// observed on every supported tmux generation (3.3a, 3.4, 3.7b) in
    /// repeated identical trials, so starving the client would make this a
    /// coin flip. The pane state reached is identical either way, which is
    /// what this test is about; the e2e suite covers the delay-driven path
    /// end to end, tolerating both outcomes.
    #[farhelm_testtrace::test]
    async fn a_paused_pane_surfaces_and_recovers_through_the_replay_path() {
        let server = ScratchServer::start().await;
        let pane = server
            .driver
            .create_session(
                "fh-pause-test",
                "/",
                80,
                24,
                &[],
                // A producer fast enough that "the stream went quiet"
                // genuinely means paused, and slow enough not to bury the
                // test in output.
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "i=0; while :; do echo PAUSETEST-$i; i=$((i+1)); done".to_string(),
                ],
            )
            .await
            .expect("session");
        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-pause-test", &pane)
            .await
            .expect("replay stream");

        // Prove output is flowing before pausing, so the quiet below is
        // attributable to the pause rather than to a pane that never
        // started.
        pump_until(&mut stream, 10, |event| {
            matches!(event, OutputEvent::Bytes(bytes) if bytes.windows(10).any(|w| w == b"PAUSETEST-"))
        })
        .await;

        stream
            .send_raw_command(&format!("refresh-client -A \"{pane}:pause\""))
            .await
            .expect("pause command");
        pump_until(&mut stream, 10, |event| *event == OutputEvent::Paused).await;
        // Consume the pause command's own closing marker before running
        // the replay group, which reads reply blocks positionally — see
        // `drain_command_reply` for why the notification arrives inside
        // that block, and why this is awaited explicitly rather than
        // approximated with a timeout on a read that is not cancel-safe.
        stream
            .drain_command_reply()
            .await
            .expect("pause command reply");

        // Model an own-pane query prefix received immediately before the
        // pause. It cannot cross the replay boundary: those live bytes belong
        // to the abandoned stream, while the bytes after replay begin a fresh
        // terminal transcript.
        assert!(stream.query_strip.feed(b"\x1b[").is_empty());
        assert!(stream.query_strip.has_pending());

        let (_, content) = stream
            .resume_paused_with_replay()
            .await
            .expect("catch-up replay");
        assert!(
            content.windows(10).any(|window| window == b"PAUSETEST-"),
            "the catch-up replay must carry the pane's history, not an empty capture"
        );
        assert_eq!(
            stream.query_strip.feed(b"6n"),
            b"6n",
            "post-replay bytes must not complete a prefix from the abandoned live stream"
        );

        // Live output resuming is the other half of the contract: a
        // continue that returned a snapshot but left the pane paused
        // would look identical up to here and leave the terminal dead.
        pump_until(&mut stream, 10, |event| {
            matches!(event, OutputEvent::Bytes(bytes) if bytes.windows(10).any(|w| w == b"PAUSETEST-"))
        })
        .await;
        shutdown_test_stream(stream).await;
    }

    /// Each query in the stripping table must be answered by the pinned tmux,
    /// while the query bytes themselves stay out of live output. The explicit
    /// catalog is independent of the production table so deleting an entry
    /// cannot quietly delete the guard case that would catch it.
    #[farhelm_testtrace::test]
    async fn pinned_tmux_answers_every_stripped_query() {
        const REQUIRED_QUERY_ENTRIES: &[&[u8]] = &[
            b"\x1b[6n",
            b"\x1b[5n",
            b"\x1b[c",
            b"\x1b[0c",
            b"\x1b[>c",
            b"\x1b[>0c",
            b"\x1b]10;?\x07",
            b"\x1b]10;?\x1b\\",
            b"\x1b]11;?\x07",
            b"\x1b]11;?\x1b\\",
        ];

        assert_eq!(
            super::super::query_strip::all_entries(),
            REQUIRED_QUERY_ENTRIES,
            "the production filter must contain exactly the pinned-tmux query catalog"
        );
        let server = ScratchServer::start().await;
        for (index, &query) in REQUIRED_QUERY_ENTRIES.iter().enumerate() {
            let name = format!("fh-query-{index}");
            let ready = server.dir.path().join(format!("query-ready-{index}"));
            let pane = server
                .driver
                .create_session(
                    &name,
                    "/",
                    80,
                    24,
                    &[("TERM".to_string(), "xterm-256color".to_string())],
                    &[
                        "sh".to_string(),
                        "-c".to_string(),
                        "while [ ! -f \"$1\" ]; do sleep 0.01; done; \
                         stty raw -echo; printf '%s' \"$2\"; cat"
                            .to_string(),
                        "farhelm-query".to_string(),
                        ready.to_string_lossy().into_owned(),
                        String::from_utf8(query.to_vec()).expect("query table is ASCII"),
                    ],
                )
                .await
                .unwrap_or_else(|error| panic!("creating session for query {query:?}: {error:#}"));
            let (_modes, _prefill, mut stream) = server
                .driver
                .open_replay_stream(&name, &pane)
                .await
                .unwrap_or_else(|error| panic!("opening stream for query {query:?}: {error:#}"));
            std::fs::write(&ready, b"ready").expect("releasing query pane after live stream opens");
            let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let observed_by_pump = std::sync::Arc::clone(&observed);
            let query_for_pump = query;
            pump_until(&mut stream, 10, move |event| {
                if let OutputEvent::Bytes(bytes) = event {
                    let mut observed = observed_by_pump.lock().expect("observation lock");
                    observed.extend_from_slice(bytes);
                    reply_matches(query_for_pump, &observed)
                } else {
                    false
                }
            })
            .await;
            let observed = observed.lock().expect("observation lock").clone();
            assert!(
                !observed.windows(query.len()).any(|window| window == query),
                "query leaked into live output: {query:?}; observed {observed:?}"
            );
            shutdown_test_stream(stream).await;
        }
    }

    /// Recognize the response family tmux emits for one guarded query.
    ///
    /// The guard needs signatures rather than exact replies because cursor
    /// coordinates and terminal identity vary with the scratch pane.
    fn reply_matches(query: &[u8], observed: &[u8]) -> bool {
        match query {
            b"\x1b[6n" => {
                observed.windows(2).any(|window| window == b"\x1b[") && observed.contains(&b'R')
            }
            b"\x1b[5n" => observed.windows(4).any(|window| window == b"\x1b[0n"),
            b"\x1b[c" | b"\x1b[0c" => {
                observed.windows(3).any(|window| window == b"\x1b[?") && observed.contains(&b'c')
            }
            b"\x1b[>c" | b"\x1b[>0c" => {
                observed.windows(3).any(|window| window == b"\x1b[>") && observed.contains(&b'c')
            }
            b"\x1b]10;?\x07" | b"\x1b]10;?\x1b\\" => {
                observed.windows(9).any(|window| window == b"\x1b]10;rgb:")
            }
            b"\x1b]11;?\x07" | b"\x1b]11;?\x1b\\" => {
                observed.windows(9).any(|window| window == b"\x1b]11;rgb:")
            }
            _ => false,
        }
    }

    /// A session whose OTHER window is flooding must deliver none of that
    /// flood to a terminal's control client — the tmux-side pane filter
    /// doing its job, against a real tmux.
    ///
    /// The local drop in `next_output` makes the terminal's BYTES correct
    /// whether or not the filter was ever installed, so "the right output
    /// arrived" proves nothing here. `foreign_dropped` is the only
    /// observable that distinguishes the two: a filter that silently
    /// stopped being applied would show up as a number climbing with the
    /// neighbour's output rate, and as nothing else at all until a busy
    /// tab started delaying somebody's agent.
    ///
    /// Two things beyond the count are asserted, because without them a
    /// PASS would be ambiguous. The sink's drain task must still be
    /// running: if it had ended, the filter's safety net would be gone and
    /// the reason no foreign notifications arrived might be that tmux had
    /// stopped reading the neighbour entirely. And the neighbour itself
    /// must still be making progress, observed outside tmux through a file
    /// it writes — the difference between "filtered" and "frozen", which
    /// is invisible from this client's stream by construction.
    #[farhelm_testtrace::test]
    async fn a_flooding_neighbour_reaches_a_terminals_control_client_not_at_all() {
        let server = ScratchServer::start().await;
        let progress = server.dir.path().join("neighbour-progress");
        let agent = server
            .driver
            .create_session("fh-filter-test", "/", 80, 24, &[], &ticking_pane("AGENT"))
            .await
            .expect("session");
        let (_window, flooder) = server
            .driver
            .new_window(
                "fh-filter-test",
                "/",
                &[],
                &bursting_pane("NEIGHBOUR", &progress),
            )
            .await
            .expect("a second window to flood from");
        assert_ne!(agent, flooder, "test premise: two distinct panes");

        // The sink first, exactly as the attach handler orders it: without
        // one, the filter this test is about would stop tmux reading the
        // neighbour's pane instead of merely not forwarding it.
        let mut sink = server
            .driver
            .open_session_sink("fh-filter-test")
            .await
            .expect("session sink");
        let sink_task = tokio::spawn(async move { sink.drain().await });

        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-filter-test", &agent)
            .await
            .expect("replay stream");
        // Settle first: the neighbour can legitimately speak between the
        // pane listing and the cutover that filters it, so the count taken
        // over a FRESH window afterwards is the one that means anything.
        pump_own_pane_ticks(&server, &mut stream, 5, 30).await;
        let settled = stream.foreign_dropped;
        let before = read_progress(&progress);

        // Long enough to span several of the neighbour's bursts.
        pump_own_pane_ticks(&server, &mut stream, 15, 30).await;

        assert_eq!(
            stream.foreign_dropped,
            settled,
            "a flooding neighbour reached this terminal's control client {} more times after \
             the filter should have been in force; tmux's own pane filter is not being applied",
            stream.foreign_dropped - settled
        );
        assert!(
            !sink_task.is_finished(),
            "the sink stopped draining during the test, so a quiet stream proves nothing"
        );
        assert!(
            read_progress(&progress) > before,
            "the filtered neighbour stopped making progress — it was frozen, not filtered"
        );
        shutdown_test_stream(stream).await;
    }
    /// A pane created AFTER a terminal attached is silenced on its first
    /// notification, not left streaming for the life of the attachment.
    ///
    /// `refresh-client -A` names one pane and has no wildcard, so the
    /// attach-time filter can only cover panes that already exist — which
    /// makes "the user opens a tab while watching the agent" precisely the
    /// case the attach-time pass cannot reach. Without the late path, that
    /// tab's entire output would be delivered to, and discarded by, every
    /// other terminal of the session for as long as it stayed open.
    ///
    /// Asserts on the count STOPPING rather than on it being small: some
    /// notifications necessarily arrive before the filter can be sent (the
    /// first one is what triggers it), so the property is that the stream
    /// converges to exactly zero new ones, and a late filter that never
    /// took effect would keep climbing with the producer.
    #[farhelm_testtrace::test]
    async fn a_pane_created_after_attach_is_silenced_when_it_first_speaks() {
        let server = ScratchServer::start().await;
        let progress = server.dir.path().join("late-progress");
        let agent = server
            .driver
            .create_session("fh-late-filter", "/", 80, 24, &[], &ticking_pane("AGENT"))
            .await
            .expect("session");
        let mut sink = server
            .driver
            .open_session_sink("fh-late-filter")
            .await
            .expect("session sink");
        let sink_task = tokio::spawn(async move { sink.drain().await });
        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-late-filter", &agent)
            .await
            .expect("replay stream");

        // Only now does the neighbour exist — the shape the attach-time
        // filter provably cannot have covered.
        server
            .driver
            .new_window(
                "fh-late-filter",
                "/",
                &[],
                &bursting_pane("LATE", &progress),
            )
            .await
            .expect("a late window to flood from");

        // Settle: the first notifications from the new pane are what
        // TRIGGER the filter, so they are expected and are not what this
        // measures.
        pump_own_pane_ticks(&server, &mut stream, 10, 30).await;
        let settled = stream.foreign_dropped;
        assert!(
            settled > 0,
            "test premise: the late pane must have reached this client at least once, or the \
             late path was never exercised"
        );
        let before = read_progress(&progress);

        pump_own_pane_ticks(&server, &mut stream, 15, 30).await;
        assert_eq!(
            stream.foreign_dropped,
            settled,
            "a pane created after attach kept streaming to this terminal ({} more notifications \
             after it should have been silenced)",
            stream.foreign_dropped - settled
        );
        assert!(
            !sink_task.is_finished(),
            "the sink stopped draining during the test, so a quiet stream proves nothing"
        );
        assert!(
            read_progress(&progress) > before,
            "the late pane stopped making progress — it was frozen, not filtered"
        );
        shutdown_test_stream(stream).await;
    }

    /// A filter command written while output is live must not desynchronize
    /// the catch-up replay that may follow it immediately.
    ///
    /// The two share one stdout and read it differently: the filter's reply
    /// arrives as chatter whenever it happens to arrive, while
    /// `resume_paused_with_replay` reads four blocks POSITIONALLY. A filter
    /// reply still in flight when the catch-up starts would be taken for the
    /// modes block, shifting every block after it — a replay that returns
    /// the history capture as its mode string and the visible screen as its
    /// history, silently. This is the interleaving that actually orders
    /// them, and the reason `pending_filter_replies` is counted rather than
    /// assumed to have cleared.
    ///
    /// # Why the debt is created directly rather than raced for
    ///
    /// The natural setup — let a late pane speak, then pause — cannot
    /// reach the state under test against a real tmux, and that is worth
    /// recording so nobody "fixes" this into a race. `next_output` keeps
    /// reading until it has an event for its OWN pane, so the filter it
    /// writes for a late pane and the reply to that filter are both
    /// consumed inside the same call, hundreds of milliseconds before the
    /// call returns and the test regains control. The debt is real, and it
    /// is settled correctly; there is simply no instant a test can observe
    /// it from outside. Writing the command directly puts the stream in
    /// exactly the state a filter written moments before a `%pause` leaves
    /// it in, which is the state the catch-up path has to survive.
    #[farhelm_testtrace::test]
    async fn a_late_pane_filter_does_not_desynchronize_the_catch_up_replay() {
        let server = ScratchServer::start().await;
        let progress = server.dir.path().join("racing-progress");
        let agent = server
            .driver
            .create_session(
                "fh-filter-race",
                "/",
                80,
                24,
                &[],
                // A distinctive, continuous marker so the replay's content
                // can be recognized in the capture below.
                &ticking_pane("AGENT"),
            )
            .await
            .expect("session");
        let mut sink = server
            .driver
            .open_session_sink("fh-filter-race")
            .await
            .expect("session sink");
        let sink_task = tokio::spawn(async move { sink.drain().await });
        let (_window, neighbour) = server
            .driver
            .new_window(
                "fh-filter-race",
                "/",
                &[],
                &bursting_pane("RACER", &progress),
            )
            .await
            .expect("a neighbour window");
        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-filter-race", &agent)
            .await
            .expect("replay stream");
        pump_own_pane_ticks(&server, &mut stream, 3, 30).await;

        // A real filter command, left with its reply outstanding.
        stream
            .send_filter_command(&silence_live_pane_command(std::slice::from_ref(&neighbour)))
            .await
            .expect("writing a pane filter");
        assert_eq!(
            stream.pending_filter_replies, 1,
            "a written filter command must be recorded as owed a reply"
        );

        stream
            .send_raw_command(&format!("refresh-client -A \"{agent}:pause\""))
            .await
            .expect("pause command");
        pump_until(&mut stream, 30, |event| *event == OutputEvent::Paused).await;
        stream
            .drain_command_reply()
            .await
            .expect("pause command reply");

        // The catch-up must settle the filter debt first; if it did not,
        // this returns the wrong block for every field it parses.
        let (_, content) = stream
            .resume_paused_with_replay()
            .await
            .expect("catch-up replay");
        assert!(
            contains(&content, b"AGENT-TICK"),
            "the catch-up replay returned something other than this pane's history — the \
             positional block reads were shifted by an unconsumed filter reply"
        );
        assert_eq!(
            stream.pending_filter_replies, 0,
            "the catch-up must have settled the filter debt, not merely worked around it"
        );
        assert!(!sink_task.is_finished(), "the sink must still be draining");
        shutdown_test_stream(stream).await;
    }
}
