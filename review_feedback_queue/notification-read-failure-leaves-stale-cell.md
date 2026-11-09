# Notification publication does not recover from a failed reload

Reviewed commit: fca3a695dc66a871f9d7f403c991e23d288f56a2

## TLDR

After a notification read fails just after an old-terminal warning changes, the bell can remain missing or urgently
unresolved after storage recovers. Normal reconciliation does not retry that publication.

## Details

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

The prior notification read-mark and build-mismatch decisions concern GUI marks and explicit listing refreshes, not a
supervisor cell reload after committed warning changes. The unlanded rail-bell plan adds a different, non-session
notification system. This persistent mismatch is outside the rare self-correcting diagnostic filter.

## Recorded source assessment

Reviewed fca3a695dc66a871f9d7f403c991e23d288f56a2. TLDR: if reading notifications fails just after an old-terminal
warning is recorded or resolved, the bell can stay missing or urgently unresolved after storage recovers, until another
lifecycle/notification event reloads it or the supervisor restarts. Trigger/consequence is conditional on the explicit
notification-read error path, not an observed runtime failure. Source confirms: new reconcile_old_terminal_notifications
at notifications.rs:493-525 trusts the cached unresolved bit and reloads after resolution only on Ok(true);
store.rs:5664-5670 updates only unresolved rows, so a subsequent successful retry returns false after an already
committed resolution. reload_notification_cell at :538-546 logs a read error and returns false without a retry marker.
The recording branch similarly delegates to notify_session :328-361, which reloads only when added; store.rs:6191-6240
ignores an existing unresolved row. Therefore a missed publication cannot be repaired by repeating normal ticks.
Searched all service reload/record callers: they are startup, creation/relaunch/adoption and other notification
transitions, not periodic unconditional refresh; core.rs:7820-7838 also tolerates startup read errors with an empty
cell. The new per-session reconciler thus inherits the helper's stale-publication failure for this new warning and its
newly added cross-generation resolver. SPEC.md Status:2517-2528 requires the warning while something old runs and
resolution once nothing old runs; SPEC_impl.md Session notifications:5846-5851 requires replacing the cell after
recording/resolution. Bucket other: persistent diagnostic mismatch after a read error; no established loss or security
route and highest not possible on this path. Confidence confirmed conditional source route; no runtime fault frequency
or concrete OS cause claimed, no runtime checks allowed. Fix by retaining a dirty/reload-needed marker until a
successful refresh or refreshing after successful idempotent writes when the cached condition still disagrees.
proposed_drop false; possible_cover none. No unverified premise remains for the conditional failure behavior.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g006/l5`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
