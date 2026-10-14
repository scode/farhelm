# Pending conversation change can be discarded after reboot

Independent drop decision: a fresh `gpt-6.1-sol` high reviewer rejected the coverage exclusion as ambiguous. The
implementation's missing-pane report discard rule does not clearly accept retaining and offering a superseded
conversation after ordinary reboot. File-watch pickup narrows the window but does not remove it. The report/refresh/
reboot ordering and subsequent vendor Resume were not reproduced.

Reviewed commit: e0c5ca1effae1e59604e6ca9c5c1df56cb00c96a

## TLDR

If the host reboots before a newly reported conversation is recorded, Restart can return to the session’s previous
conversation even after a successful refresh.

## Details

Suggested bucket: `highest`. Confidence: likely; the code path is established, but the precise report/refresh/reboot
ordering and subsequent vendor Resume were not reproduced. Source: dedicated `gpt-6.1-sol` high adversarial review.
Proposed drop: Propose covered by SPEC_impl.md’s explicit rule discarding reports whose panes no longer exist.
Independent fresh-context confirmation is required before dropping; assess whether that rule also accepts retaining an
older Resume target after an ordinary host reboot..

At the reviewed commit, service/listing.rs:153-183 removes refresh-time report reconciliation; handlers.rs:461-470 does
likewise for rename replies. Trigger: a Claude session has stored conversation A, changes to B and writes a valid
report, receives a refresh before the next reconciliation, then its host reboots. Previously, an uncontended refresh
drained that report (first-parent listing.rs:164-167). Now it can remain pending. Startup calls capture_now
(core.rs:5365-5367), but owned_pane_anchor rejects the vanished pane (14445-14459); report_files.rs:252-256 settles that
refusal. Boot conversion preserves captured identity (store.rs:5393-5399), and session_snapshot constructs Resume from
that stored identity (core.rs:6273-6292). Consequently A can remain the Resume target. Scope: reports not yet admitted
when their pane disappears; ordinary agent exit with a retained pane is unaffected. This is a potentially highest
candidate presented for mandatory independent coverage review, not an undisputed actionable finding.

Possible cover: Reviewed SPEC_impl.md, Runtime state → Report files, lines 1967-1968 explicitly discards reports whose
pane no longer exists; lines 1732-1737 and 1975-1977 accept timer-owned pickup. Pinned-main SPEC_impl.md:2255-2256
retains the discard rule. Current SPEC.md:1040-1050 additionally accepts wrong conversation pointers under narrowly
specified crash/combined-fault conditions, but those conditions should not automatically be extended to an ordinary
reboot. TRIAGE_OUTCOMES.md heading restart-resume-older-conversation-report-drain-fails.md concerns a different trigger.

Status at frozen endpoint: Still possible at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703:
report_files.rs:261-265 still settles the missing-pane refusal, and core.rs:14742-14756 still rejects it. Commit
379b215ea30df2ac0a3c29f34a71858aed131578 adds prompt file-watch pickup and narrows the historical timer window; it does
not eliminate disappearance before admission. No complete fixing commit identified.

Review coverage: full commit diff read: true. No commit-diff content remains unread. Large surrounding modules and
specifications were read selectively, not in full; the 13,089-line triage ledger was searched and relevant entries read
rather than exhaustively read. Full report-watcher implementation and remaining supplemental-fix tests were not
inspected. No runtime proof of the reboot candidate or vendor Resume behavior.

Runtime evidence: None, as requested by default. Review was read-only; no checkout files or scratch reports were created
or modified. No nested agents were spawned. The proposed highest drop awaits the parent’s mandatory independent review.
