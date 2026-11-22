# Late replay history can displace an already browsed final screen

Reviewed commit: df6199142d6495a95b2d008400ba2686efb775c6

## TLDR

If a replay reveals the exit frame before its history is delivered, the viewer can browse that frame and then jump to
old history without a scroll request. An already stored selection may be dropped or name different rows.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-grid-model/src/lib.rs:333`.

grid.rs 197–219 and 260–266 put the first committed screen before oldest-first history and permit an initial history
base other than zero. The explicit idle exception (farhelm-helm-client/src/terminal.rs 54–77,636–640,692–701.
Attached.rs 1176–1198) can expose that screen without waiting for the history or its marker, and later output continues
on the same attachment. Grid model lib.rs 436–444 assumes catch-up prevents anchoring at provisional screen base zero.
In the changed viewport, first subtracts the lift even with no history (viewport.rs 61–86), and post-exit local wheel
uses that range (attached.rs 1490–1535). When the first history arrives, lib.rs 326–347 changes screen_base and reports
only ordinary history damage. viewport.rs 48–55 retains the anchor and 113–119 clamps it into the newly numbered range.
The padding-only anchor previously displaying the final screen can now clamp to the oldest history, even though the
screen's own frame was not replaced. The desktop similarly only trims the old selection against the new oldest line
(terminal.rs 536–548,731–738). If the numeric range now falls in history, it denotes that history. This is an unwanted
effect of provisional numbering, distinct from deliberately allowing partial reveal. SPEC.md tip 3299–3304 promises
ordinary replay and final-frame browsing. Terminal.rs 288–295 describes preserving the anchored text across ordinary
history changes. The unvalidated idle assertion at attached/tests.rs 1080–1120 checks new screen updates on the same
attachment, and the exited scrollback test at 3003–3059 completes replay before scrolling. Neither checks the first
history append into an already navigated idle-revealed final screen. Rebase or invalidate provisional anchors when that
first history settles screen numbering. Bucket other for displaced view or selection. No data loss claim. Confidence
confirmed by static model and replay-admission source under the documented idle-reveal condition. No observed native
stall or view change. Possible coverage none. Follow-up source check: the binary receive branch at
farhelm-helm-client/src/terminal.rs 619–624 forwards GridOutput regardless of catch_up state. Only the text marker at
636–640 is ignored after ReplayIdle. Thus neither the late history nor the renumbering requires a new attach or an
exit-state change. An initial history chunk with retained lines covering a previously selected provisional row preserves
that numeric range in TerminalView::sync, so this can retarget the selection without an off-screen or unreachable line.
Existing assertions do not cover the combined branch.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `scrollback/l21`.
