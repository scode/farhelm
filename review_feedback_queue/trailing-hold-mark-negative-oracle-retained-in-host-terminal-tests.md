# Host-terminal test module preserves the missing/stale hold-mark oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the single replacement test in core.rs does not exercise missing/stale marks, but a separate
host-terminal test preserves those exact negative and positive assertions. The prior test-coverage finding is withdrawn
on this new source evidence, not on unchanged production correctness. Original suspected consequence retained: accepting
missing/stale version-hold marks could let shutdown stop waiting before a just-starting session protects its program
version; the earlier record alleged lost regression protection, not an observed product bug. Refutation:
core/tests/host_terminals.rs the_hold_wait_waits_for_a_host_agents_child (475–527), marked farhelm_testtrace::test at
485, adopts a real-child stand-in host, tracks it with no mark at 501 and asserts false at 502–505; writes child+1 and
asserts false at 506–511; writes the exact child pid and asserts true at 512–517; and clears the mark, exits the child,
waits for no child root, then asserts true at 519–526. Thus accepting absent marks, accepting a different pid, or
refusing a correct mark is distinguished at the tracked-terminal boundary. The core test module declares host_terminals
at core.rs:16284. Repository-wide pinned source search found these test calls; the earlier core.rs-only search missed
them. The deleted parent's an_orderly_stop_waits_for_new_windows_to_hold_their_version at core.rs:27577–27637 had the
same four cases, now exercised against a host. This is direct test relocation, not an assumption of coverage from shared
subsystem names. SPEC_impl.md Terminal host, line 2230: The orderly stop's hold wait compares th

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g115/c1`.

### g115/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (candidate, withdrawn):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: the single replacement test in core.rs does not
exercise missing/stale marks, but a separate host-terminal test preserves those exact negative and positive assertions.
The prior test-coverage finding is withdrawn on this new source evidence, not on unchanged production correctness.
Original suspected consequence retained: accepting missing/stale version-hold marks could let shutdown stop waiting
before a just-starting session protects its program version; the earlier record alleged lost regression protection, not
an observed product bug. Refutation: core/tests/host_terminals.rs the_hold_wait_waits_for_a_host_agents_child (475–527),
marked farhelm_testtrace::test at 485, adopts a real-child stand-in host, tracks it with no mark at 501 and asserts
false at 502–505; writes child+1 and asserts false at 506–511; writes the exact child pid and asserts true at 512–517;
and clears the mark, exits the child, waits for no child root, then asserts true at 519–526. Thus accepting absent
marks, accepting a different pid, or refusing a correct mark is distinguished at the tracked-terminal boundary. The core
test module declares host_terminals at core.rs:16284. Repository-wide pinned source search found these test calls; the
earlier core.rs-only search missed them. The deleted parent's
an_orderly_stop_waits_for_new_windows_to_hold_their_version at core.rs:27577–27637 had the same four cases, now
exercised against a host. This is direct test relocation, not an assumption of coverage from shared subsystem names.
SPEC_impl.md Terminal host, line 2230: The orderly stop's hold wait compares the "held" mark with the child's pid. The
replacement suite tests that observable contract. The retired blocking-tmux probe stimulus still has no equivalent
external-host-wait boundary in synchronous hold_settled (7798–7808), while the mark boundary remains and is tested
separately. Suggested bucket: other for the withdrawn test-only allegation. Could-be-highest: no established product
security or user-work-loss route in this allegation; no underlying production consequence has been lowered from an
established highest finding. Confidence: source refutes the claimed loss of missing/stale-mark regression coverage; no
execution. proposed_drop: the exact host test above preserves the alleged missing oracle, so do not queue c1.
possible_cover: source refutation, not queue/plan/filter coverage. Material caveat: no tests run, and this does not
assert every removed tmux scheduling case has a new asynchronous counterpart.

Returned proposal (claims-074, event 11): **refuted**, suggested bucket **other**.

The record's later withdrawal is supported by the exact negative and positive assertions in the separately compiled
host-terminal test. The earlier core.rs-only test search was incomplete.

Original stronger consequence: Originally considered shutdown failing to protect a just-starting session's version. The
later retained allegation was only lost missing/stale-mark regression protection; event 11 withdraws it after locating
the relocated host test. No runtime version loss was observed or established.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: The exact alleged lost assertions remain in a compiled host test. Conservatively route the withdrawal to
the independent drop register because the original stronger motivation involved a session's executable/version
protection, even though the established allegation is test-only. Do not drop the separately queued registration-witness
issue.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g115 record](records/review-through-00de0b6c/groups/g115.json). Record ID: `c1`.
