# A replacement connection can hide credential-push cancellation

Reviewed commit: 49ce8cb76e9f4c841e15122a89264c7c908171dc

## TLDR

A credential push from an ended host connection can keep running if a replacement connection arrives before the push
observes retirement. The new connection then waits behind obsolete transport work, potentially for the SSH operation's
budget. No credential disclosure, wrong-machine delivery or loss is established.

## Details

Suggested bucket: **high**. Confidence: confirmed conditional source race; no runtime reproduction or frequency claim.
Introduced by `cffb06a42563389f4c281c26e50d2057363cac0b` and retained through the reviewed tip.

The delivery entry keeps a reusable watch count in
`crates/farhelm-helm/src/provisioning/credential_delivery.rs:137–153`. Lease drop decrements it at `199–208`; entering a
new lifetime can raise zero to one at `257–270`. Push cancellation tests only the current zero value at `367–377`. Tokio
1.53.2's watch channel retains the latest value, so a push unpolled across `1 -> 0 -> 1` need not observe zero. The old
push and new connect share the serial lock (`306,373`).

This is not fenced by connection setup. A fresh status check, transport dial and hello precede acquiring the new lease
(`manager.rs:3658–3759,3911–3963,4009–4013,4135–4163,4190–4235`; `tensorlake/hosts.rs:82–97`), but nothing forces the
old push to be polled during that interval. SPEC_impl.md, Named credential catalog, `5966` requires final-lease
cancellation before the next transport poll.

If the old push already holds its snapshot, missing zero permits subsequent slot work: Claude settings, Codex
configuration/authentication, and GitHub installation/helper setup/removal (`credential_delivery.rs:412–609`). A queued
push may instead clone the successor's binding after acquiring serialization (`302–309,348–380`); it skips names the new
binding omits and otherwise uses that connection's valid selections. No separate selection violation was found. Observed
cancellation drops the apply future at its next outer select poll; it does not roll back a remote command already sent.

Queued database stamp writes are a separate, preserved ordering boundary. The blocking closure owns the serial guard
through its update (`credential_delivery.rs:748–763`; `crates/farhelm-supervisor/src/db.rs:230–244`), so later delivery
cannot overtake that stamp. Attempt numbers suppress older warning publication (`credential_delivery.rs:157–195`).
Returned failure labels reread the same host entry's current binding (`379–384`); no cross-host attribution or
wrong-destination route was established.

A per-active-lifetime latched cancellation signal could retain counted overlap while making a zero transition
irreversible for its old push. The owning tests' shared-slot cancellation checks do not exercise this missed-zero
schedule. No accepted specification, planned repair, queue item, ledger decision or filter was found to cover it.

Provenance: run `4ca8e296-4f73-4429-a34b-031af1582085`, `g010/g010-lease-zero-coalescing`, with interface handoff
`g012/g012-l1`. The initial possible-highest question was investigated, and the independent delivery consultation agreed
to **high** for the established delay/stale-work consequence only. A demonstrated disclosure or work-loss route would
require reassessing that bucket. Proposed drop: none for the race.

## Abbreviated review through 00de0b6c

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g023/f5`.

### g023/f5

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 26 (candidate, open):

Reviewed commits: 04ffdbb35773cef36e73eb798c005ddbeb9a9de6 and 56659839785691f76da3af5d8f51b511cff4ba8b. TLDR:
Investigated whether moving credential and tunnel acquisition behind the provider hook could leave a credential push or
tunnel alive after a connection hold cancels acquisition. Source refutes that route in this extraction; no reportable
defect is established. Details: at04ff manager.rs:3647-3667 the connected-hook future is selected against the actor
nudge and its returned ConnectedResources remains in scope through serve. A nudge drops an in-progress hook before the
actor continues to park. Tensorlake hosts.rs:157-173 enters its credential lease before awaiting connect, then awaits
tunnel acquisition while retaining that lease. At566 the same order is retained in tensorlake/hosts.rs:339-355 and
environment/podman.rs:1383-1401; environment/mod.rs:393-410 documents partial-acquisition cancellation and owns both
leases. credential_delivery.rs:199-208 decrements the active lease count synchronously on Drop; enter at288-299
increments before any hook await. tunnels.rs:112-126 awaits the recipe read before creating any relay;137-182 has no
further await between registering/spawning and returning TunnelLease. Consequently cancellation during that await has no
spawned relay, and once the worker is spawned its lease is returned in the same poll. TunnelLease::drop at71-79 aborts
the worker, retires its claiming client and removes its view. The actor owns that returned lease across publication and
serving, so hook cancellation does not strand it. Contract: SPEC_impl.md at566 Disposable environments:6419-6421
requires connected credential/tunnel leases; environment/mod.rs:393-395 explicitly requires partial acquisition to drop
on cancellation and the lifetime to end before hold parking. Suggested bucket: no finding. Could-be-highest: yes in the
original considered security consequence if an unauthorized credential push could survive withdrawal, but the
cancellation/ownership route is source-refuted here; no such effect is claimed. Confidence: confirmed source refutation
of this narrow extraction concern, not execution. proposed_drop: source refutation through pre-await lease acquisition
and no-await spawn-to-return region, as cited, not specification acceptance. possible_cover: none known; distinct from
f2 lifecycle result/error hold release and g024/l5 idle stopped-actor manager retention. This candidate records a
considered-and-refuted concern, not a new verified or unverified finding. No runtime tests, reproduction, builds or live
action. Follow-up source checks: credential_delivery.rs:375-406 filters active registrations and biases the
active-count-zero cancellation branch ahead of further push work, including serial-lock waits. Actor parking is
published only on the next loop at04ff manager.rs:3591-3602, after the connected-arm scope has ended. The full
base-to04ff diff shows the selected hook call is the changed seam while the acquisition select and lease-through-serve
ownership are retained, so this is a preservation check of the assigned extraction, not an unrelated old implementation
search. No independent test-oracle or documentation claim is dismissed by this production ownership refutation.
Named-input synthesis opened (turn10): read all20 prior events before revision. Narrow the original source refutation
immediately to absence of lost partial credential/tunnel leases and absence of an await between tunnel worker spawn and
owning-lease return. Lease Drop proves deregistration, not latched cancellation of every independent push before its
next transport poll. The supplied reusable-watch1-to0-to1 premise is a distinct pusher-lifetime concern; verify
current-zero cancellation, shared Podman caller/Arc and whole-range ancestry before finalizing corroboration. Preserve
original could-be-highest security suspicion and exact source-drop basis, but do not infer credential disclosure, wrong
recipient/selection or loss of user work. Parent states it already retains the underlying watch race and owns
affected-scope recording; this is source corroboration under f5, not a duplicate local finding, a coverage-source search
or an accepted parent drop. No execution. Named caller/source corroboration: at566 credential_delivery.rs:199-208 Drop
decrements the reusable active watch count, entry at275-283 reuses the same Entry for the same host/entry_uuid, and
enter at288-299 can restore zero to one. push at375-406 selects against active.wait_for(CURRENT count == 0), even with
biased polling; no per-push cancellation generation is captured there. At566 environment/podman.rs:1383-1401 and
endpoint1381-1399 enter that delivery before connect/tunnel awaits. Endpoint lib.rs:2460-2476 passes clones of
sources.delivery to both concrete providers before installing them in Providers. Thus the named Podman caller shares
this current-count mechanism; this does not change the partial-hook lease-ownership refutation. Continue bounded checks
of watch semantics and base/range preservation before finalizing the named corroboration. No shown disclosure,
misdelivery, unauthorized selection or user-work loss; no new local issue ID. Whole-range ancestry check: the full
exclusive-base-to566 diff of credential_delivery.rs leaves Entry.active, ConnectionLease::drop, enter and the
active.wait_for cancellation branch unchanged; base already has the reusable-watch mechanism and its strong cancellation
comment. The diff generalizes read_bindings from Tensorlake to environment_binding and apply from SSH/Tensorlake to
resolved remote access. At566 lib.rs:2456-2472 installs the actual new Podman provider with the same delivery Arc; base
lib.rs:2415-2430 has only the Tensorlake production hook. Thus old mechanism ancestry does not exclude newly supported
Podman exposure introduced in range. This corroborates affected domain without creating a duplicate issue or changing
impact. Endpoint Cargo.lock pins Tokio1.53.2, and the matching local watch source is available for the remaining
predicate-semantics check. No focused133 artifact or coverage section has been opened. Watch-semantics check confirmed
from matching Tokio1.53.2 primary source: sync/watch.rs:3-4 retains only the last value; wait_for docs at823-838
explicitly allow skipped updates and use the most recent value; wait_for_inner at896-926 evaluates the current shared
value and version; send_modify/send_if_modified at1104-1112,1171-1208 overwrites that value and notifies without
retaining prior values. Therefore a push not polled across active1-to0-to1 can miss the transient zero, even with a
biased select. If its apply future already holds entry.serial while awaiting backend work, resumed old work can retain
that lock and delay the replacement connect at credential_delivery.rs:334-338 until completion/deadline. New enter
clearing live.binding does not erase bindings/row already captured inside apply. This is corroboration of the
parent-retained independent pusher cancellation concern; no duplicate local finding is created. Retract any broad
reading of prior f5 follow-up as proof that every push is latched-cancelled before its next transport poll. Preserve the
narrow partial-lease/spawn-handoff refutation and original security suspicion/drop basis, with no established
disclosure, wrong recipient, unauthorized selection or user-work-loss consequence. Source-only conditional scheduling
mechanism, not observed runtime incidence. Contract/caveat clarification: at566 SPEC_impl.md:6384-6391 expressly
distinguishes hook lease ownership from replacement pushes and says, Dropping the final lease cancels in-progress pushes
before their next transport poll. credential_delivery.rs:199-200 makes the same strong comment. Current-count watch
cancellation does not guarantee that statement across an unobserved1-to0-to1 transition; the preserved comment/contract
cannot serve as evidence that cancellation is latched. This remains part of the named parent-retained race, not a newly
invented documentation finding, and the partial-hook ownership refutation does not dismiss the distinct contract/comment
discrepancy. The same section6393-6398 and push375-406 impose one absolute command timeout across candidates/lock
waits/remote work, so the corroborated delay is bounded by its deadline rather than an indefinite new hang. At566 the
parent-to-commit diff adds the complete Podman connected hook;04ff production lib.rs:2439-2451 installs Tensorlake only.
Endpoint credential_delivery.rs is byte-unchanged from566 (git diff empty), preserving the named current-zero behavior.
No higher-impact consequence or runtime incidence is claimed. Final named synthesis conclusion (turn10): f5 remains a
candidate ONLY for the lost-partial-lease/await-spawn-handoff suspicion; that narrow source refutation stands. Its
earlier TLDR and follow-up must not be read to refute all independent pusher lifetime correctness. An old already-polled
push can retain captured work across a reusable active-count1-to0-to1 cycle before its next poll, because wait_for sees
current1, not a latched zero. Same-entry reconnect enters before awaiting entry.serial; an older apply holding that lock
can delay the new connect. begin_push at166-175 protects a not-yet-started candidate, and enter clearing live.binding
protects subsequent selection reads, but neither retracts row/bindings already captured by an executing apply. This
confirms the named parent-retained mechanism and newly supported Podman affected domain, without a duplicate local
finding. Original considered security consequence is preserved as history, but these checks show no credential
disclosure, misdelivery, unauthorized selection or user-owned-work loss. No parent drop or lowering has been accepted.
Proposed drop applies solely to the lost-partial-hook-lease route and exact original source basis; it does not apply to
this independent watch race or its stronger comment/contract assertion. The same Tokio1.53.2 version is pinned at
base,566 and endpoint. Range attribution is new Podman connected caller/shared delivery at566 plus general remote
delivery routing, with the reusable-watch mechanism itself already present at exclusive base; no blanket pre-range
exclusion follows. Clarify the preceding timeout sentence: the push future has an absolute deadline, but write_record
at814-826 deliberately moves its serialization guard into noncancellable Db::call, whose queued write may finish after
caller cancellation. Therefore do not assert that every replacement-lock wait necessarily ends at the push deadline, or
recast this documented database ordering as a separate defect. Named source premises are settled; there is no remaining
local unresolved lead. Confidence: confirmed conditional source corroboration and narrow ownership refutation, no
runtime observation. possible_cover: underlying reusable-watch race is already retained by parent per supplied named
input; no coverage files or focused133 coverage sections were read. No new issue ID, unrelated discovery,
tests/builds/reproductions, processes, live actions, nested agents or checkout edits.

Returned proposal (claims-133, event 20): **refuted**, suggested bucket **other**.

The narrow extraction concern is source-refuted: cancellation drops partial hook ownership, and relay spawning has no
await before returning the owning lease. This is not a guarantee that all credential transport stops on lease Drop. The
independent pusher can miss a reusable count's 1 -> 0 -> 1 transition; that residual has the same root as the existing
high-priority queue item. Extend that item with the newly added Podman caller and current references, rather than
recording a second defect. Bucket other applies only to the refuted extraction candidate, with no established
user-visible defect; the existing residual remains high. This consultation does not authorize the security-relevant
drop.

Original stronger consequence: Investigated whether moving credential and tunnel acquisition behind the provider hook
could leave a credential push or tunnel alive after a connection hold cancels acquisition, including the possible
highest/security consequence of an unauthorized credential push surviving withdrawal. Both own-ID events already say
that this ownership route is refuted; neither claims that such a security effect occurred. Do not broaden that
refutation to all credential-pusher cancellation.

Unresolved premise or limitation: No source path shows the extraction losing ownership of a credential or tunnel lease
during hook cancellation. No disclosure, wrong-machine delivery, unauthorized credential selection or user-work-loss
route is established for the separate watch-count race; its established consequence remains obsolete work delaying a
replacement connection. A command already sent is not rolled back by dropping its awaiting future. No runtime
reproduction, scheduling frequency, tests or builds were performed.

Proposed basis: Propose refutation only of a new lost-partial-hook-ownership route in the 04ff extraction and 566
integration, retained through 00de. The original select, lease-before-await acquisition, lifetime-through-serve and
no-await relay-spawn handoff remain. Keep the residual watch-count issue in credential-push-misses-retired-lifetime.md;
propose a concrete extension citing the Podman connected hook at 566:1383-1401/tip:1381-1399, shared production delivery
wiring at tip lib.rs:2460-2476, and current delivery.rs:199-208,286-299,375-406. No new queue file, filter,
accepted-behavior dismissal or final independent-drop agreement is proposed.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g023 record](records/review-through-00de0b6c/groups/g023.json). Record ID: `f5`.
