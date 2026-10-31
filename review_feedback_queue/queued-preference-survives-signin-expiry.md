# Queued preferences survive recognized sign-in expiry

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A queued preference can survive one tab's sign-in expiry and later commit using another tab's refreshed credential.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The root writer retains unsent records when the first delayed request recognizes revoked credentials. Another tab can
restore the shared credential, after which a never-sent queued record uses it. The conditional source sequence is
complete; auth-discard wording versus multi-GUI behavior remains ambiguous. Fixed API base and native no-proxy checks
refute different cross-helm/credential-diversion hypotheses, not this retained queue lifetime.

Source locations:
`crates/farhelm-ui/src/api.rs:1231-1289,1507-1548,2141-2191; crates/farhelm-ui/src/auth.rs:364-371,408-416,429-476`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

New discovery finding, owner a039, stable ID a039-d1. Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. TLDR: a
browser page that has been told to sign in again can still save a choice it had not sent before expiry, if another tab
has refreshed the shared sign-in credential. Concrete conditional shipped sequence: start with auto-update false; in Tab
A use the enabled Settings checkbox to choose true, false, then true while the first true PUT is still pending
(settings.rs:516-525 has no busy disable). Only the first record has been sent. Arrange ordinary token
rotation/credential eviction before that request's middleware authentication and delay delivery of its same-build 401
while same-origin Tab B signs in. The three choices can precede revocation; no click after the expired page is withdrawn
is needed. Rotation revokes browser credentials (helm auth.rs:171-187), middleware returns unauthenticated without
reaching the handler (360-396), and token exchange mints a new stored credential (203-225,332-345). B writes that secret
into shared origin localStorage and reloads only B (UI auth.rs:408-416,459-476). A's recognized 401 sets TOKEN_REQUIRED
but does not clear the queue or abort root-owned writers (auth.rs:425-432; api.rs:2174-2191). send_preference_patch
collapses Unauthenticated to false (:1539-1548); finished sees recorded_since_send and returns true even though latest
equals the failed sent value (:1282-1283), because the newest true is a distinct later record. The writer loops and
sends it (:1507-1523), reading B's valid secret at dispatch (:2159-2162). The previously false backend value can
therefore become true after A's expiry without A redoing its choice. Existing admitted work is not involved: the second
PUT was not sent at expiry, and the first was definitively refused. This is neither a wrong-URL producer nor
rollback-generated PUT nor unknown-outcome/pre-policy-lock overtaking. Checks/results: refreshed exact
queue/loop/funnel/auth/control paths; read helm revocation/exchange/middleware; confirmed literal TokenPrompt reload
affects only its own window and read existing auth/skew queue items from initial coverage. Local installed Dioxus core
0.7.10 matches Cargo.lock:1193-1196 and global_context.rs:145-201 binds spawn_forever to ScopeId::ROOT, so replacing
PreferencesGate children cannot cancel the writer. The code has no TOKEN_REQUIRED dispatch guard. Read current auth
browser test :263-402: it exercises sign-in and reload in the same page, without a second refreshing tab or queued
preference; inspected queue rollback unit test api.rs:7281-7318, which calls writer_lost rather than recognized-401
continuation. These tests do not cover this interaction; none was run. Normal response latency and request ordering are
explicit trigger conditions; actual occurrence/frequency are unmeasured, not a missing source producer.
Specification/coverage: SPEC.md:3794-3803 explicitly discards all page-held unaccepted state on sign-in expiry; this
path retains and sends a newer unaccepted choice. One GUI at a time (:3816-3827) broadly accepts multi-GUI races, so
retain as a scope conflict rather than silently treating that as exact coverage. This is not merely a stale view or a
lost report. TRIAGE_OUTCOMES.md:18520-18533 covers failed dirty opt-out replay through removed seed_with_local_changes;
no reseed/reload or retention of the failed record is required here, so trigger/scope do not fully match. Existing
auth-skew-remount-retains-row-operation-count.md and shortcut-armed-state-survives-auth-remount.md share credential
refresh ingredients but have different consequences. TODO Planned is empty; the Near-term infallible-connection item
(:140-167) is not Planned coverage and concerns transport failures; this trace can receive a normal completed 401 and
later 204 without a lost reply. No matching unlanded queue-plan or BUGS/filter coverage found. FILTER
rare-safe-failure/display rules do not fully cover the durable new preference commit. Suggested bucket OTHER:
preference/sign-in contract gap, with no established user-work-loss or separate security consequence; do not infer
highest solely from the auto-update field. Confidence confirmed conditional source mechanism, product coverage
interpretation explicitly surfaced. proposed_drop: none. possible_cover: SPEC.md One GUI at a time versus Signing in
again, unresolved exact scope. Missing empirical evidence: occurrence/frequency only; a source-accurate two-page
queued-write/401 regression would validate the conditional schedule, but runtime proof is not required to retain this
source finding. No fixes or checkout mutations performed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a039/a039-d1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
