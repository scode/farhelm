# Notification publication does not recover from a failed reload

Reviewed commit: fca3a695dc66a871f9d7f403c991e23d288f56a2

## TLDR

After a notification read fails just after a session notification is recorded or resolved, the bell can remain missing
or urgently unresolved after storage recovers. Normal capture passes do not retry that publication.

## Details

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

The prior notification read-mark and build-mismatch decisions concern GUI marks and explicit listing refreshes, not a
supervisor cell reload after committed notification changes. The unlanded rail-bell plan adds a different, non-session
notification system. This persistent mismatch is outside the rare self-correcting diagnostic filter.

Narrowed on 2026-11-12, after tmux removal: the finding was found on the old-terminal warning's per-session reconciler
(`reconcile_old_terminal_notifications`), which no longer exists; nothing records that warning now and every start
resolves it in bulk. The shared helpers it relied on are unchanged, and the same route holds for the kinds recorded
through `notify_session` and for the resume-resolving kinds the capture pass resolves.

## Recorded source assessment

Reviewed fca3a695dc66a871f9d7f403c991e23d288f56a2; rechecked at 938cafee0. TLDR: if reading notifications fails just
after a notification is recorded or resolved, the bell can stay missing or urgently unresolved after storage recovers,
until another lifecycle/notification event reloads it or the supervisor restarts. Trigger/consequence is conditional on
the explicit notification-read error path, not an observed runtime failure.

Source at the recheck: `reload_notification_cell` (crates/farhelm-supervisor/src/service/notifications.rs:460-467) logs
a read error and returns false without a retry marker. Recording: `notify_session` (:322-357) reloads only when the
store reports the row added, and `record_session_notification` turns away a launch's existing unresolved row, so a
repeated recording after a failed reload never reloads. Resolution: the capture pass's `resolve_resumable_notifications`
(:388-449, called from capture.rs:117) trusts the cached unresolved bit and reloads after resolution only on Ok(true),
while `resolve_session_notifications_if_current` (store.rs:5737-5771) updates only unresolved rows, so the next pass's
retry returns false after an already committed resolution. Therefore a missed publication cannot be repaired by
repeating normal capture passes. Other reload callers are startup, creation/relaunch/adoption and the vendor
pin-correction paths, not periodic unconditional refresh; startup tolerates read errors with an empty cell.

SPEC.md Status requires each notification to show and resolve on the session; the session-notifications rules in
SPEC_impl.md require replacing the cell after recording/resolution. Bucket other: persistent diagnostic mismatch after a
read error; no established loss or security route and highest not possible on this path. Confidence confirmed
conditional source route; no runtime fault frequency or concrete OS cause claimed, no runtime checks allowed. Fix by
retaining a dirty/reload-needed marker until a successful refresh or refreshing after successful idempotent writes when
the cached condition still disagrees. proposed_drop false; possible_cover none. No unverified premise remains for the
conditional failure behavior.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g006/l5`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
