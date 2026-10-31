# Failed screen capture can forget a Working state

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Capture failure can fall back to Idle after repeated unchanged screens; an ordinary busy Muse sequence causing an
unaccepted Restart remains unestablished.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

ticker.rs:608–638,738–743 and screen_reader.rs:365–374 can fold unchanged comparisons into generic Idle; actual
probe/capture-failure dispatch reaches that fold. Recovery, run replacement, generation and attention consumers were
checked. The missing premise is supported active Muse work yielding the required repeated grids, followed by failure and
an unaccepted harmful Restart. Static Thinking/echo fixtures do not supply a live timing trace. Exact weak-detector
Restart acceptance is possible coverage, not a blanket work-loss waiver. Suggested other reflects the established
classification limitation; a complete unaccepted user-work-loss route would raise priority.

Source locations: `crates/farhelm-supervisor/src/service/ticker.rs:738-743`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Final discovery status of original f1; all previous events remain. Reviewed full commit
d6645f771bee8ed03a66486ef8755834937d759d, assigned introducing reader commit 5ebb3c79271ec33905a4806feb6e9ad8c37e1732.
No new finding ID or verified vendor failure. Source basis: observe_screen ticker.rs:608-638 counts unchanged captures
even when anchored Working; forget_tail738-743 replaces the reading with generic_reading(old counts);
screen_reader.rs:365-374 chooses Idle after>=3 unchanged comparisons. Both actual probe/capture-failure dispatches
invoke it (ticker.rs:1491-1514,1583-1597). Suspected consequence remains a genuinely busy agent stopped by Restart
without its extra working confirmation (core.rs:11759-11778). Actual checks/results: read those folds/dispatches, run
replacement503-543, persist_work_started1661-1771, history update1774-1844, recovery tests4639-4852 and6190-6228, plus
warning and observation-only attention consumers. Recovery resets comparison streak and suppresses fabricated
transitions; new run defaults clear prior screen evidence; work-start persistence checks generation and run-cell
identity. These refute broader lifecycle contamination but do not prove failure-time Working fallback. Missing premise:
ordinary supported Muse active work yielding identical live grids for three of its own sampling comparisons, followed by
capture failure and an unaccepted harmful Restart consequence. Supplied static echo/Thinking fixtures show recognized
shapes only; no independent live repeated-grid timing trace or complete harmful producer is present in this source
review. Source-only constraints cannot authenticate the missing vendor behavior; versioned live grids with independently
witnessed active-run state and a compatible failure/restart trace would settle it. proposed_drop: original narrow
Waiting-versus-Idle causality is refuted (both omit the extra question), and SPEC.md:1348-1362,1963-1977 explicitly
accepts weak-detector Restart uncertainty. possible_cover: those exact clauses, with Muse scope in301-303,1885-1887 and
SPEC_impl.md:3081-3087. This is NOT a blanket partial-harness/security/work-loss exclusion. The new failure caveat must
remain visible to a028/parent exclusion checking; no independent verdict on it was consumed here. Confidence:
conditional source behavior confirmed; vendor/consequence route unverified after investigation. Suggested bucket: other
for the established conditional classification limitation; highest only with an established unaccepted
user-process/work-loss route. Coverage-source search for failed capture/unchanged Working/forget_tail across
SPEC.md,SPEC_impl.md,TODO.md,BUGS.md,TRIAGE_OUTCOMES.md,plans/queue and review_feedback_queue/INDEX.md found no narrower
matched coverage; no private runtime evidence or quoted waiver was authenticated.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g101/f1`, `a055/g101-f1`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
