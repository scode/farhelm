# Replay test consumes the outstanding reply before testing its recovery

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The replay test settles its pending reply before replay begins.

## Details

F149 — **definite** — `crates/farhelm-supervisor/src/tmux/stream.rs:3376`;
`crates/farhelm-supervisor/src/tmux/stream.rs:3322` — Replay test consumes the outstanding reply before testing its
recovery

On the intended ordered command path, reaching the later pause consumes the earlier filter reply. Replay therefore
starts without the outstanding reply debt that the test claims it must recover, and removing recovery settlement can
leave the result unchanged. Complete pause and its reply before sending the filter command, assert outstanding debt
immediately before replay, then verify recovered content and modes.

## Evidence and triage context

- crates/farhelm-supervisor/src/tmux/stream.rs:3363 sends the filter before sending pause at :3373.
- crates/farhelm-supervisor/src/tmux/test_support.rs:94 pumps next_output until the pause event.
- crates/farhelm-supervisor/src/tmux/stream.rs:1454 consumes filter debt while processing command terminators.
- crates/farhelm-supervisor/src/tmux/stream.rs:3385 invokes replay only after that pumping and the pause reply drain;
  replay settlement is at :1521.
- crates/farhelm-supervisor/src/tmux/stream.rs:3300–3320 states that the test must exercise positional replay reads
  while a filter reply remains outstanding.
- crates/farhelm-supervisor/src/tmux/stream.rs:3363–3380 sends and counts the filter command first, then sends the pause
  command, pumps until Paused, and drains the pause reply.
- crates/farhelm-supervisor/src/tmux/test_support.rs:85–105 implements pump_until by repeatedly calling next_output.
- crates/farhelm-supervisor/src/tmux/stream.rs:1449–1461 decrements pending_filter_replies when next_output consumes the
  earlier filter reply's End or Error marker.
- crates/farhelm-supervisor/src/tmux/stream.rs:1602–1635 explains and implements draining the remainder of the pause
  command's reply. On the normal ordered forced-pause path, the earlier filter reply has already been consumed.
- crates/farhelm-supervisor/src/tmux/stream.rs:840–855 makes settlement a no-op when the debt count is zero. Lines
  1513–1523 call it before replay; lines 3384–3395 assert replay content and final zero debt without asserting debt
  immediately before recovery.
- crates/farhelm-supervisor/src/service/connection.rs:1575–1585 is the production consumer of resume_paused_with_replay
  and sends its returned replay to the viewer.
- SPEC_impl.md:1090–1096 describes ordered synchronous tmux command processing and distinct reply blocks;
  SPEC.md:1228–1231 requires recovery without a silent output gap.
- crates/farhelm-supervisor/src/tmux/stream.rs:3363 sends the filter command and asserts debt before sending the
  explicit pause.
- crates/farhelm-supervisor/src/tmux/stream.rs:3376 pumps output until pause and :3377 drains its reply before recovery.
- crates/farhelm-supervisor/src/tmux/stream.rs:1454 decrements debt when those earlier reply terminators are consumed.
- crates/farhelm-supervisor/src/tmux/stream.rs:1521 performs production settlement; :844 skips it when debt is already
  zero.
- SPEC_impl.md:1091 documents ordered command execution and separate reply blocks.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:1091 establishes command ordering/accounting. No exact existing coverage found.
- review_feedback_queue/shutdown-expiry.md:14–24 concerns unsafe client teardown on supervisor shutdown, not positional
  reply alignment or this regression-test setup.
- SPEC_impl.md:1068–1071 accepts missing normal-buffer history after alternate-screen replay. That is a different
  trigger and consequence from misaligned command replies.
- No exact coverage identified.

Caveats:

- No mutation test or live tmux run was performed.
- The claim concerns the ordinary ordered command path; accidental additional pause events are not a reliable fixture
  premise.
- Production currently performs the required settlement.
- No tmux or mutation run; incidental additional pause events do not establish the fixture premise.
- No runtime or mutation test was performed.
- The conclusion is that the test can pass without the intended settlement behavior on its normal forced-pause path; it
  is not a claim that every possible incidental pause follows that path.
- Current production recovery includes the settlement call. This finding establishes a defective regression test, not a
  current production replay failure.
- An unsolicited automatic pause could change the interleaving, but the test neither establishes nor requires that
  alternative. No mutation test performed; the current production settlement call is present.
- An automatic pause could alter ordering, but the test does not establish that alternate premise.
- No mutation test performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_cor_02:p1:F2`,
`gap_supervisor_runtime_sec_03:p1:F1`, `gap_supervisor_runtime_sec_02:p1:F5`.

- `gap_supervisor_runtime_cor_02:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_runtime_sec_03:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_runtime_sec_02:p1:F5`: confidence as filed: definite; suggested bucket as filed: other.
