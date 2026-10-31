# Completed follow-up can leave an older Update tracked busy

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

If the GUI misses a successor's Running state and first sees Completed, an earlier tracked Update can remain busy.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Source checks preserve both the multiple-client case and an automatic/offline single-GUI producer. The tracked run
identity and successor-completion reconciliation can fail to retire the old owner. A complete adverse observation
schedule and resulting persistence were not observed. This differs from the existing completed-only retry warning
record's narrower symptom.

Source locations:
`crates/farhelm-ui/src/provisioning.rs:2465-2512,950-971; crates/farhelm-helm/src/provisioning/automatic.rs:113-150`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d. New a010/f7, all earlier revisions retained. Introducing
interface change 04e8b3f97901f79e07e2fc7cb95a211f8cb60335 supplies the helm-owned automatic producer; UI reconciliation
predates it. TLDR: after a failed manual Update and a successful automatic follow-up, one GUI may keep showing Updating
and keep Update and that host's Settings unavailable until reload or another ownership-changing event. Complete
conditional source route: start manual A while automatic updates are off; after acceptance its page lock is released (UI
provisioning.rs:1786), so enable automatic updates from the same GUI (settings.rs:511-526; preference commit wakes scan
at helm preferences.rs:262-264). Scan skips a busy host before attempt reservation (automatic.rs:113-129), so a first
manual A does not spend the automatic attempt. A transient payload-preparation failure can fail A before any plan action
(service.rs:2256-2267,2381-2413), leaving the old connected host eligible. Failure memory only applies to the matching
automatic run (2226-2233); manual A has none. Failure publishes a revision after busy clears; listener's revision loop
(automatic.rs:43-64) may plan/start B (129-150). If the GUI misses A's terminal view and every B Running view, its next
applicable progress view names B Completed: backend retains only the latest run (service.rs:2604-2620), replacing the
per-host entry on claim (1771-1778) and retaining terminal Completed after busy clears (2280-2291). UI reconcile_tracked
writes uncertainty at 2465-2482, but neither adopts B Completed nor clears old nonterminal A at 2487-2512.
retained_run_live/update_owned (950-971) keep ownership; derive_row_update_progress (897-919) keeps awaiting-progress
Updating; menu publication (3751-3783) withdraws offers and HostRow settings is disabled through
provisioning_menu.planning (hosts.rs:4091). Source checks found all tracked.set(None) sites: late changed-target
acceptance, idle vanishment, correlated success and actual target change; repeated B Completed triggers none. Normal
incarnation/path learning does not clear accepted continuity (same_update_target 379-384). Tests inspected: unit
6364-6435 checks owned/terminal predicates; browser provisioning.spec.ts:4275-4325 deliberately supplies an older
Completed then returns A's own completion, and 5086-5115 observes B Running before B Completed. Neither establishes
recovery from first-seen genuinely newer B Completed. No tests executed. Confidence: unverified after investigation with
confirmed conditional state-machine mechanism and accessible shipped producer. Exact missing premise: an observed
single-GUI schedule skipping A terminal and all B Running while preserving the row/identity; source coalesced read loop
(reader.rs:644-678; provisioning.rs:2995-3102) supplies no transition-replay guarantee, but cannot authenticate an
actual end-to-end timing trace. Source-only restrictions preclude running that trace. A controlled single-GUI held-read
trace across manual payload failure and successful automatic B, then repeated B Completed and recovery assertions, would
settle it. Suggested bucket other: host-specific UI availability/state persistence; no user-work loss, unauthorized
target or security route established, not potentially highest. proposed_drop none for this single-GUI automatic
residual. possible_cover: TRIAGE_OUTCOMES.md:12926-12936 newer-completed-run-leaves-older-update-permanently.md discards
the same reconciliation mechanism for another-window trigger under One GUI at a time; SPEC.md:3816-3832 confines that
best-effort rule to multiple GUIs. The new helm-owned automatic trigger is not clearly covered; if ledger scope is
judged broader, retain the ambiguity for triage. The concurrent-window subset is covered. Queue
completed-only-retry-keeps-host-warning.md concerns attention acknowledgement, not retained ownership/disabled Settings.
Searched SPEC/SPEC_impl, empty TODO Planned, BUGS, plans/queue, full INDEX and ledger; no exact corrective plan or
duplicate for this residual. No filter: ordinary refresh/re-read does not release ownership. All original g135/g136
exclusions remain unchanged and narrow.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a010/f7`, `g136/f4`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
