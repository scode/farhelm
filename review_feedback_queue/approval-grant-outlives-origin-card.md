# Permanent approval can save authority after its card and origin end

Reviewed commit: 8835a1e184a54beb2a560f094089753120e6a92e

## TLDR

A permanent approval answer waiting for a host lock can enable unattended session control after the requesting
connection has ended and its card has disappeared. The original command remains fenced, but the saved permission can
survive.

## Details

Suggested bucket: **highest**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

The exact older ledger item at TRIAGE_OUTCOMES.md:7824-7839 assesses check-before-lock, but accepts operations following
a logical host across destination changes or installation adoption. Independent clarification disagrees that it fully
covers ordinary same-install disconnect and card removal before the write. The specific live-card contract at
SPEC_impl.md:8640-8664 is narrower than general logical-entry wording; its expiry exception starts after the write.
Preserve this policy tension and original security priority. No different-installation grant or execution of the expired
original operation is claimed.

## Recorded source assessment

Reviewed 8835a1e184a54beb2a560f094089753120e6a92e; source reconciliation at pinned
9e3ab60b86025ca74948ededd09b99cd81a2477c. TLDR: the permanent answer can save same-install unattended session authority
after the requesting connection and card are gone. Confirmed source route: approvals.rs:791-834 copies context, checks
liveness, then awaits the host cache/write lock. manager.rs:2195-2206 takes that lock and :2228-2240 awaits the ordinary
durable remember_session write while holding it. Independent peer-close handling at manager.rs:4668-4737 publishes None;
publish_refresh :5073-5180 changes client/incarnation without cache_lock. The waiting approval's feed/liveness loop at
approvals.rs:534-546 ends on that publication, and PendingGuard :344-348 removes its card. IMPORTANT correction to the
preceding event: client.retire calls abort_agent_tasks, but mutation answer tasks deliberately survive retirement
(approvals.rs module docs :38-43); the established ending here is the explicit origin watcher, not an assumed abort of a
mutating wait. The independent HTTP answer remains server-owned (lib.rs:818-823,:941,:2632-2640; middleware.rs:66-75).
Once the lock is free, it does not recheck liveness or card presence, and store.rs:7555-7580 writes OwnHost for the same
recorded installation identity before approvals.rs:878-896 discovers the missing card and returns Gone.
Different-installation grant remains source-refuted by the SQL identity predicate; original operation crossing origin
replacement remains refused by agent_requests.rs:2546-2570. Contract: SPEC_impl.md:8640-8647 says Always allow stores
only for a still-live card connection; :8658-8664 applies live-origin/identity checks to this narrow grant;
SPEC.md:3603-3604 binds approval to the origin connection. Acceptance of expiry BETWEEN write and take does not cover
writing AFTER connection/card termination. Confidence confirmed for reachable source ordering and same-install stale
grant, no runtime or frequency claim. Original highest-potential security severity/history retained for parent judgment;
established effect is the persistent permission selected for that install, not proven cross-install escalation,
original-command execution, or data loss. proposed_drop: none; the earlier withdrawal is superseded and pre-existence is
not a drop basis. possible_cover: SPEC.md:352-363 general logical-entry rule creates a policy tension with the specific
live-card rules, which parent can adjudicate; not assumed acceptance. The ordering existed for fleet/browser grants
before assigned commit and now also handles OwnHost. Suggested fix: recheck live origin and pending request after lock
acquisition, and coordinate final authorization with publication if a fully atomic boundary is required. Earlier lead,
withdrawal and consequence discussion remain append-only.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g040/l1`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
