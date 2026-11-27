# An owed change hint can still be starved for as long as terminal output saturates the connection, so the helm falls back to its thirty-second backstop poll

Reviewed commit: 36c579d53bd8eaec3cdad590be60361a2eca1d0b

## TLDR

When a host has a busy terminal open over a slow link (the case this change names), the supervisor tells the helm that
the host sessions changed only when its hint finds room on the connection, and it never waits for room. Terminal output
and replies do wait, and they get every slot that frees up first. So the hint can be held back for as long as output
keeps the link full, and the helm session list for that host (statuses, exits, waiting states of the OTHER sessions
there) can lag by up to the helm backstop poll, now thirty seconds. The spec says that poll now only covers changes no
hint reports.

## Details

The owed-hint retry in `send_sessions_changed` (crates/farhelm-supervisor/src/service/hints.rs:155-180) only calls
`link.notify.try_send(...)`, at most once per `OWED_HINT_RETRY` (2 s, hints.rs:81), and owes again on
`TrySendError::Full`. The same bounded writer queue (`CONNECTION_WRITER_QUEUE` = 64,
crates/farhelm-supervisor/src/service/connection.rs:73,244) is fed by each grid stream sender with a WAITING
`self.tx.send(...).await` (crates/farhelm-supervisor/src/service/grid_stream.rs:822-840; module docs lines 57-58), and
by `send_reply`, which also waits (connection.rs:1106). Tokio is pinned at 1.53.2 (Cargo.lock:7785). In its bounded
mpsc, `try_send` goes through `batch_semaphore::try_acquire`, which reads only the available-permit counter
(tokio-1.54.1 src/sync/batch_semaphore.rs:294; same shape in 1.53.2). `add_permits_locked` (lines 336-376) assigns each
released permit to a queued waiter and adds to the counter only once the waiter queue is empty. So while any sender is
parked in `send().await` on the full queue, `try_send` cannot get a freed slot. That holds continuously during a
scrollback catch-up or a history resend, which SPEC_impl.md line ~2894 itself calls the case where the sender fills its
writer queue, and most of the time for live output faster than the link. The helm output pause does not relieve this.
The helm sends `PauseOutput` only when its own per-attachment queue toward the window backs up (SPEC_impl.md
~2883-2888), not when the supervisor-to-helm link is slow, so a slow ssh link with a fast local window never pauses the
sender. The hint is therefore not dropped but can be put off indefinitely, and the 2 s debounce cuts its chances tenfold
compared with the old attempt on every 200 ms mark. The helm own refresh is unaffected, because its ListSessions reply
waits fairly in the queue, so the backstop poll (`REFRESH_INTERVAL` = 30 s, crates/farhelm-helm/src/manager.rs:171)
still corrects the list eventually. SPEC_impl.md, the paragraph starting "Version 33 adds a second message" (~3205-3220
at the tip), says the owed hint is retried "until one is queued". It also says the thirty-second poll, "since version
95, when hints stopped being dropped, ... covers only a change no hint reports (a bug)". Under a saturated queue that is
not true, and neither is the hints.rs module doc (lines 53-56) saying that keeping the hint owed "is what lets the helm
rely on hints and poll rarely". How to verify: read the two tokio functions named above, then the waiting grid send. A
fix is to have an owing link join the fair queue instead of sampling it: one spawned `send().await` or `reserve()` per
link, guarded by the owed flag so it stays a single hint. That is the `notify_detached` shape (connection.rs:1128-1147),
which never blocks the caller. Alternatively, keep the helm poll short while a host hints are being deferred.
Established from source only; not reproduced or observed in execution.

Reviewer revision notes: Revision 2, a complete restatement that supersedes earlier events: it adds two checks from the
synthesis turn (the helm output pause does not relieve a slow link, SPEC_impl.md ~2883-2888; and REFRESH_INTERVAL is 30
s at manager.rs:171) and restates every field.

### Triage notes

- Recorded bucket: `high`. The reviewer suggested other but said it could be high because it defeats the guarantee the
  30-second poll change relies on in exactly the case the commit names; recorded as high because the bucket is
  uncertain. g26/c9 notes the deferral is bounded by the 30 s poll since c2c9a634.
- Suggested bucket (reviewer): other, because the staleness is bounded by the thirty-second poll, needs a saturated link
  and loses no data. It could be high, because it defeats the guarantee that this commit and the poll change rely on, in
  exactly the scenario the commit names.
- Confidence: likely. Confirmed from source: tokio fairness, the waiting grid and reply sends, the helm pause trigger,
  and the thirty-second poll. Unverified premise: how often a sender is actually parked at the 2 s attempt instants on a
  real slow link, which decides whether the delay is a few seconds or reaches the poll interval.
- possible_cover: g26 (polling) reviews the move of the backstop poll to thirty seconds. A finding there about the poll
  relying on hints that cannot be lost would overlap.
- proposed_drop: none.

### Related reviewer records

#### g26/c9 (candidate): Bears on g25/f1: its deferral is up to 30 s because of c2c9a634

Location: crates/farhelm-helm/src/manager.rs:171

Concerns known finding g25/f1 (an owed hint retried with try_send can keep losing to grid senders on a saturated link,
deferring the change to the backstop poll). Not re-recorded as a finding; this note supplies g26's side for the
commissioner. Since c2c9a634, REFRESH_INTERVAL is 30 s (manager.rs:171), so g25/f1's 'deferred to the backstop poll'
means up to 30 s of stale session list, against up to 3 s before. Because of a lost hint the same window also delays the
window's tab, exit and status updates. c2c9a634's premise (manager.rs:149-160, SPEC_impl.md:3218-3220: 'in normal
operation every change arrives as a hint') is what g25/f1 undermines on a busy link. The waiting ntfy alert is only
partly protected: once a listing has started a spell, its 3 s deadline refresh (manager.rs:5826) no longer needs a hint,
but the listing that first reads Waiting is itself the one a lost hint delays. Set aside as a separate record so that
g25/f1 is not duplicated. When triaging g25/f1, weigh its bucket against the 30 s window, not 3 s.

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g25/f1 (finding), group g25 "Helm session rows and supervisor change hints", findings log line 8.
- g26/c9 (candidate), group g26 "Polling each host's session list every 30 seconds", findings log line 10.
