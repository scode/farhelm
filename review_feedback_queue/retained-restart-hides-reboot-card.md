# An old Restart success hides a later reboot's recovery card

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

If Restart succeeds while another session is open, then the host reboots before any listing sees it running or stale,
opening it can show an attachment attempt to a dead terminal instead of its interrupted-session explanation. A later fix
repairs the case once that observation happens. The remaining gap was expressly accepted in the landed plan; that
possible cover needs reconciliation with the product specification. Header Restart and Replace still work.

## Details

Bucket: **other**. Confidence: **confirmed source defect**, not a runtime observation. g007/g009 originally suggested
high; g007 revised to other after checking the available header recovery. g009 preserved high provisionally. A fresh
independent priority check agreed to other for the retained residual: the recorded consequence is incorrect recovery
presentation, with equivalent header recovery and no established loss of work or Resume capability.

Reviewed source change: `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`, still present at the reviewed tip.
`crates/farhelm-ui/src/session_ops.rs:107-115` and `list/view.rs:3037-3040` retain successful Restart. While its source
view is absent, list reconciliation at 1893-1899 does not retire that outcome merely because the host ran and later
rebooted. Returning `session_view.rs:621-647` seeds `relaunched=true`; a current nonstale Interrupted detail at
1029-1050 does not clear it. Local retirement at 1872-1876 preserves it, and the card predicate at 1934-1936/3691-3697
remains false. The investigation found no other expiry covering this absent-view history.

`restart_available` at 2079 still uses the valid stored Resume offer. Header Restart at 2433-2466 and Replace at
2559-2575 remain rendered and enabled apart from their ordinary lifecycle/sandbox gates. Interrupted does not need the
working-agent question (`restart_needs_confirmation`, 197-206), so header Restart remains a direct recovery action. The
same offer powers the missing card; no captured conversation or Resume capability is lost.

`SPEC.md:2127-2128` and “Durability and resume”, 3499-3505 require the interrupted explanation/card and forbid presuming
the old process survived reboot. Retire obsolete successful optimism when the listing establishes the later lifecycle,
without losing its useful short interval before a successful Restart reaches the listing.

At earlier recording-time main `cb03def00870e18a98a1fd9046073d2849be3253`, the exact TODO was Near term, not Planned.
Current-main recheck at `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741` narrows this draft: the later fix `b3d6b3ef` retires
remembered success when a committed listing positively observes the restarted session running or stale. Current
`session_ops.rs:79–111,215–225` tests stale/non-Interrupted and no pending request, then clears `restart_succeeded`;
`list/view.rs:1976–1985` invokes it even for positive filtered, truncated or one-session observations. That established
subset is repaired and is no longer proposed as a current defect. Current `session_view.rs:633` still seeds from
retained success if no qualifying observation happened before the second reboot, leaving the narrower gap described
above.

Possible cover: `plans/queue/session-action-leftovers.md:67–70`, Decisions already made, explicitly records the
maintainer's acceptance: “if the host reboots again before any listing saw the restarted session running or stale,” the
second interruption cannot be distinguished and “the dead terminal still shows.” The plan has landed. It therefore is
not current unlanded-plan repair coverage under recording rules, and neither current specification nor active ledger
repeats this exception; it is still material evidence of intended acceptance. Current `SPEC.md:2651–2652` says “A
rebooted session still shows its interrupted card.” Coverage remains ambiguous, so the residual is retained for triage.
Planned is empty; neither indexed unlanded tmux-removal plan repairs the remaining cause, and no exact queue or BUGS
item was found.

Possible filter cover also remains: `FILTER.md`, “Session status and history after a crash or a partly failed
operation”, permits inaccurate recovery presentation while ordinary lifecycle operations still recover it. Its trigger
is ambiguous here: the old operation completed before the later reboot, and its GUI optimism overrides correct current
Interrupted evidence. Conversely, the filter's lost-valid-Resume-offer exception does not clearly apply merely because
the card is hidden while equivalent header Restart remains. Preserve that ambiguity for triage.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `restart-reboot-recovery`; `g007/g007-l2`, `g009/l2`,
`g013/l1`. Exact checks, original severity and revisions remain in `prep/groups/<group>/findings.jsonl`.
