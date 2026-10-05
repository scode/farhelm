# Sidebar YOLO replacement can run after cancellation

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Sidebar YOLO replacement can run after cancellation.

## Details

`F20 / COR-YOLO-SIDEBAR-CANCEL` — **definite** — `crates/farhelm-ui/src/list/view.rs:3260` — Sidebar YOLO replacement
can run after cancellation

Cancelling the sidebar's YOLO question does not revoke an answer already queued for that question. YOLO means the
successor agent runs without approval prompts. When the sidebar renders the question, both affirmative handlers retain
the source session and the permission to delete it. Cancel clears the displayed question, but a queued affirmative
handler can still use those retained values. The replacement path checks whether an operation is already running; it
does not check whether this question is still open.

A Cancel followed by either queued affirmative answer can therefore launch the unrestricted successor and delete the
source session. The permanent answer can also turn off future YOLO questions on that host. Before dispatching either the
replacement or the permanent preference change, consume a live question bound to this opening and source session. Test
Cancel followed by each answer in one event burst. The session-header version already reads the live question before
proceeding and differs from this sidebar path.

Suggested bucket: highest

Possible cover: none

Caveats: The existing confirmation tests in `ops.rs:483–510` and the earlier decision in `TRIAGE_OUTCOMES.md:2229–2270`
establish queued Cancel/Confirm events as a case the UI must handle. No browser reproduction was performed, and the
frequency of this ordering in actual use was not measured.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `ui_data p1`, `ui_lifecycle p1`, `ui_security p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Eventburst premise source-established ops483–510/ledger2229–2270; browser frequency/replay not
measured. Header prompt reads live slot and differs.

## Filed reviewer metadata

- `ui_data p1`: confidence as filed: **definite / confirmed**. Premise: cancel and confirm events can be delivered
  before the prompt's rerender removes the old handler; this premise is explicitly established by `ops.rs:483–510`,
  `e2e/tests/terminal.spec.ts:3378–3435`, and the earlier confirmation-race triage. Suggested bucket as filed:
  **highest**.
- `ui_lifecycle p1`: confidence as filed: definite; confirmed by closure capture and operation admission. Caveat: not
  runtime reproduced. Suggested bucket as filed: highest (security consent and loss of a session/conversation).
- `ui_security p1`: confidence as filed: definite / confirmed by code. The event-burst premise is already an explicit UI
  contract in `ops.rs:482–511` and the accepted prior regression in `TRIAGE_OUTCOMES.md:2229–2269`; this particular
  wiring was not reproduced at runtime. Suggested bucket as filed: highest (unwanted process launch and source-session
  deletion).
