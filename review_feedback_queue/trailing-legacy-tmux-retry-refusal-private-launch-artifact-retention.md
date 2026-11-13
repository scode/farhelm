# Tmux-retry refusal defers cleanup of credential-bearing launch input

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Refusing a create interrupted under an older supervisor bypasses immediate cleanup of its credential-bearing launch input. A retained checkout also retains a valid session credential. The private spec is eligible for existing Error-row cleanup, including on the next writable reload; it is not established to persist across successful supervisor starts. This remains a proposed drop under bounded-retention policy and the local security boundary, subject to the caller's independent decision.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g105/l1`.

### g105/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (candidate, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513; synthesis also checked through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.

TLDR: Refusing a create interrupted under an older supervisor bypasses immediate cleanup of its credential-bearing launch input. A retained checkout also retains a valid session credential. The private spec is eligible for existing Error-row cleanup, including on the next writable reload; it is not established to persist across successful supervisor starts. This remains a proposed drop under bounded-retention policy and the local security boundary, subject to the caller's independent decision.

Details: The early return at core.rs:11226-11243 precedes retry cleanup at :11708-11727. Retry resolution and validation do not clear the old spec. prepare_launch at :15659-15713 writes the spec before terminal start; it includes the command (possibly containing user credentials) and session_token, created 0600 inside private supervisor state. The refusal helper at :15866-15929 deletes and settles an ordinary session, atomically rolls back a still-Planned checkout, or keeps an allocated/unreadable ownership record as a visible Error with its registry rows and memberships. Planned rollback is preceded by absent-path validation (independently confirmed by parent-provided g107/l1). If ownership cannot be reread, the conservative branch retains the evidence. If settlement itself fails, the prior row and pending reservation remain visible and unresolved durability is reported; the helper never claims artifact cleanup succeeded. At the pinned tip the same helper remains at core.rs:14653-14717 with the same branch structure and prelaunch refusal; parent-provided g116 confirms the later wording-only revision.

Credential distinction: successful row removal revokes its session token, but retained ownership does not. store.rs:4004-4059 changes outcome and launch_hooked without clearing session_token; :4806-4831 authenticates from that row's token without testing outcome. handlers.rs:3801-3848 rechecks that credential before relay. A retained Error session therefore keeps valid request authority even after its spec is cleaned, as part of keeping the session rather than a new credential exposure. The command-line credentials in the spec also already exist in its retained stored launch. No source route to an unauthorized reader of the 0600 spec or private state was established.

Lifetime correction from the preceding synthesis events: the directory sweep alone (launch_artifacts.rs:652-805) preserves a current-generation spec of a surviving row, but it is not the only startup cleanup. core.rs:7072-7080 calls cleanup_launch_artifacts for every already-Error row on writable reload, independently of a newly observed sentinel. launch_artifacts.rs:299-339 returns Ok(true) from its preservation gate when generation zero has no sentinel, then removes the settled spec, status and waypoint (:491-504). Thus a successfully retained Error refusal is eligible for cleanup on the next writable reload, including a supervisor restart. The same reload branch survives at tip core.rs:6726-6732. Cleanup read/unlink failures are logged and retryable. With a successfully deleted row, the next startup sweep removes the orphan spec and waypoint. With a failed retention transaction, the pending row's exact lifetime remains governed by subsequent retry/reconciliation or Delete, and the unresolved failure is surfaced; no new accumulation route is introduced. The earlier assertion that a retained Error-row spec ordinarily survives successful starts until Delete is retracted as refuted by this independent reload path. The original stronger concern remains: immediate cleanup of prior credential-bearing input is bypassed, and retaining ownership does preserve credential validity.

Suggested bucket: other for the established private-file retention. Confidence: confirmed source and contract analysis; no behavior observed during execution, no builds or reproductions. proposed_drop: SPEC_impl.md, Leftover files, :94-97 states exactly: "Confirmed 2026-09-28: files left behind are not a leak, and need no extra cleanup machinery, when they are bounded by a fixed amount, grow only with a rare and explicit user action (such as switching the release download source), or are removed by the next attempt at the same operation or by the next start of the process that owns them. As with retained supervisor metadata, the line is accumulation without bound over time during ordinary operation." This policy is unchanged at the pinned tip. The pre-upgrade tmux creates are a finite inherited set, since no new tmux launches are created; normal successful cleanup additionally removes orphan or retained-Error specs on startup. This uses boundedness, not rarity to lower an established security consequence. SPEC.md:4939-4944 states exactly: "The local security boundary is the Unix account on a particular host. Processes the user runs on the target host, including same-account processes that can reach Farhelm's private tmux server or its terminal hosts (the processes that keep its new terminals), are trusted by this threat model. Farhelm does not provide strong same-account isolation against deliberate interference with agents, that private session or those terminal hosts, and session credentials do not provide same-account containment."

Could-be-highest: no established security or loss-of-work route after investigation. Credential-bearing input and retained valid authority are explicit, but their presence in private same-account state does not establish disclosure; a demonstrated unauthorized reader or transfer across the host boundary would reopen highest regardless of rarity. possible_cover: none; coverage sources were not searched. No parent drop is accepted. Lead l1 is resolved back to candidate: the startup cleanup premise is now settled through source, and there are no unresolved readily checkable premises.

Returned proposal (claims-068, event 10): **covered**, suggested bucket **other**.

Immediate cleanup is bypassed, and retained ownership keeps a valid session credential. The established consequence is bounded private-file retention accepted by the leftover-files policy and local security boundary. The stronger claim that retained Error-row specs ordinarily survive successful writable restarts until Delete was expressly withdrawn and is source-refuted. This remains a proposed coverage disposition requiring independent review.

Original stronger consequence: Immediate cleanup of credential-bearing launch input is skipped; retaining checkout ownership retains valid authority. Earlier history claimed its spec could survive ordinary successful starts until Delete, then retracted that assertion after checking Error-row reload cleanup. Preserve that correction and the valid-token distinction; do not replace them with a claim that all credentials were revoked.

Unresolved premise or limitation: A security consequence would need an unauthorized reader or transfer beyond private same-account state. Failed-settlement lifetime is not proof of a new unbounded accumulation route; no such route was established.

Proposed basis: Exact bounded-retention acceptance and local security boundary, plus source refutation only of the stronger ordinary-restart lifetime claim. Parent independent drop check must assess credential-retention coverage and consequence narrowing.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: bounded-private-legacy-launch-input-retention-covered-with-valid-token-caveat. Full068 read. Immediate refusal skips cleanup and retained ownership keeps an authenticating row token; do not claim revocation. Ordinary successful writable reload cleans Error-row artifacts, so earlier until-Delete-through-restarts assertion is source-refuted; failed settlement still exposes unresolved durability. Fixed inherited legacy set and private0600 files fit leftover-files policy, with no new ordinary unbounded producer. Local Unix-account boundary is proposed security coverage only absent unauthorized reader/transfer; not a broad no-secret-content claim. Existing prompt-duplication wording item/active decision explicitly preserve retention behavior and do not cover new disclosure. Gate must judge exact boundaries and retain original valid-authority consequence.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g105 record](records/review-through-00de0b6c/groups/g105.json). Record ID: `l1`.

