# Custom agent Delete: the close-resume route through the Delete's own reply is untested, and the reworked test lost its post-Delete page checks

Reviewed commit: ee8ef7294a6638d7905fca7dd27083d0d0bb45c9

## TLDR

Closing Settings while a custom agent's Delete is out is supposed to wait for the Delete, then close, or stay open on
the agent's page with the helm's reason if the Delete is refused. Only the case where the helm's catalog update arrives
before the Delete's reply is tested. The ordinary case (the reply arrives first) and the refused case are untested, so a
regression there would let Settings hang open or close over a refused Delete without any test failing. The same rework
also dropped the checks that, after a Delete, Settings shows Add custom agent and the deleted agent's entry is gone.

## Details

The commit adds two resume routes for a close waiting on a custom agent Delete: the push path in the catalog handler
(custom_agents.rs:226-232, `deleted_while_deleting` -> `save_receipt.settled_other(false)`) and the Delete's own
completion (custom_agents.rs:588-593, `settled_other(refused)` before the draft is retired or `deleting` is cleared).
The only test that replies to a custom agent Delete is `creating_and_deleting_a_custom_agent_follows_the_catalog`
(settings/tests.rs:637-660; grep for `Method::Delete, "/api/custom-agents` finds only line 644). After this commit it
pushes the removal, settles, asserts Settings already closed, and only then replies 204, so it exercises the push route
only. Nothing exercises (a) Escape during a Delete whose 204 reply arrives before the push, where the Delete
completion's receipt must resume the close after the retire in the parent, or (b) a refused Delete (e.g. 409/500), where
the close must stay open on the agent's page with `custom-agent-error`, via `CloseRefusal::edit` and
`reveal_edit_failure`. If the receipt call in the Delete completion (lines 589-592) were removed or moved after the
retire, the close would depend on whether dropping the editor (which drops the armed receipt) happens before or after
the operation store's notification (`operations_changed` only resumes when `PendingClose.armed` is 0), and for a refused
Delete the editor stays, its receipt stays armed, and the close would sit waiting until another Escape, which then asks
or closes without showing the refused page; no test would fail in either case (source analysis, not executed).
Separately, the rework removed `assert_eq!(heading(cx), Some("Add custom agent"))` and
`!drawn(cx, "settings-section-custom-example-code")` (old tests.rs:640-642) without moving them elsewhere, and the
surviving comment at tests.rs:633 still says "Changes and Reply arrive together" although the test now settles between
them. Fix: add a test with Escape during a Delete answered 204 before any push (asserting Settings closes without a
question), one with a refused Delete (asserting Settings stays open on the agent's page with its error and asks
nothing), and restore the page assertions in a Delete flow that keeps Settings open; update the stale comment.

Reviewer revision notes: Revision (turn 2): line numbers below are at the pinned tip
b2d540a8dacfb94140e5eb0927d78e447ce46111; the cited code is unchanged since the reviewed commit, where the Delete
completion receipt is at custom_agents.rs:566, the push loop at 180-208, and the test lines at tests.rs:626 and 637.
Corrected the stale-comment line to tests.rs:633 at the tip. Re-checked: no later commit in the range touches the
close-wait code or this test. Revision: corrected the consequence of a removed receipt call for a refused Delete. Title:
Custom agent Delete's own-completion close resume (reply before push, refused Delete) has no test, and the only
Delete-reply test lost its page assertions.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (test coverage; no user-visible defect shown in the shipped code).
- Confidence: confirmed (by reading the tests: no other Delete-reply test exists).
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g03/f1 (finding), group g03 "Settings close waits for outstanding work: spec, panes and tests", findings log line 8.
