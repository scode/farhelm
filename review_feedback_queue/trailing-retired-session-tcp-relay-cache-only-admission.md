# Bounded refutation of ordinary retired-session TCP cache bypass

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Bounded refutation of ordinary retired-session TCP cache bypass

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g074/l2`.

### g074/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (candidate, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc. TLDR considered, preserved: a retired container session lacking a decodable durable cache payload can authorize a TCP tunnel on its former outside host instead of being Delete-only. The constructed API gap remains real: sessions.rs:4527-4550 checks only store.cached_session after route_session, store.rs:9719-9795 returns None for bad/missing payload while raw paired membership still routes, and supervisor service/tcp_relays.rs:264-286 checks only row existence before dialing loopback. client/tcp_relays.rs:327-369 confirms this is the BeginTcpRelay path used by tunnels.rs:303,361. No live-state/view/attachment guard is substituted for that distinct TCP boundary.

Contract check: the complete pinned SPEC_impl.md Rootless Podman sessions section (8642-8657) says 'A retired session supports Delete only. Refuse all other lifecycle, terminal and exec paths, including plain Replace.' This is a global retirement requirement. It does not explicitly prescribe an independent retirement check at each supervisor endpoint. The complete TCP relay channels section (3520-3581) instead says 'The supervisor checks session existence and confines the target to literal loopback' and requires each accepted socket to resolve the coherent route again. Neither text explicitly accepts a cache-less retired TCP call; absence of a known producer is not proof of global contract compliance. The route's source doc (sessions.rs:4525-4526) also says the cache is only a negative fence and the supervisor checks its stored launch, which TCP does not do.

Ordinary-route investigation and bounded refutation: manager.rs:4893-5040 publishes identity-bearing lists through the durable typed cache; failed cache writes retain the previous cache and publish no live fallback. resolve_owner:935-960 refuses missing raw membership for those hosts. The claimless construction noted at supervisor service/core.rs:4985-5000,6528-6591 can indeed hold None identity, correcting the prior overly broad reliance on the manager's test/embedded comment. However serve:8483-8506 refuses it before binding: ownership must exist and begin_serving must succeed. An owning construction calls ensure_host_identity before serve. A repository-wide source search of handle_connection callers finds the sole production call in the guarded serve accept loop (core.rs:8723); other direct callers are tests. Thus that concrete None-identity construction supplies no ordinary serving route. No shipped cause of a retired row's durable payload becoming malformed/mismatched was established; the spec deliberately preserves routing under pre-existing corruption (SPEC_impl.md:5993 onward), so such corruption would expose the known API gap but is not invented as a normal producer.

Suggested bucket: other for the established constructed/API contract gap; no material production failure established. Could-be-highest: YES, retain the original potential outside-host service access/security-boundary concern for independent check; neither the ordinary shipped trigger nor actual security impact is established. Existing full-authority desktop access and tunnel consent still apply. Confidence: confirmed source-level gap and bounded source refutation of the checked ordinary None-identity route; remaining production premise unverified after investigation, no execution or reproduction. proposed_drop: bounded ordinary-route refutation only, not an assertion that the API gap complies with Delete-only retirement; parent has accepted no drop. possible_cover: none, no queue/coverage sources searched. Exact missing premise: a supported caller or retained production state with a retired supervisor row that routes while its durable cache payload is unusable, or a contract explicitly requiring refusal even for constructed full-authority API callers. Accessible constructor/serve/cache/relay source cannot establish the former or the latter. A concrete shipped producer/cache incident or maintainer clarification of that API contract would settle it. Lead is investigated and returned as a candidate for independent assessment, with original consequence preserved.

Returned proposal (claims-048, event 8): **retain-unverified**, suggested bucket **other**.

The cache-only retirement fence and existence-only supervisor TCP admission establish a conditional gap. The checked claimless supervisor cannot serve, but that bounded ordinary-route refutation does not settle the global Delete-only contract or remove the corrupt-cache case.

Original stronger consequence: A retired container session without a decodable durable cache could authorize a TCP tunnel on its former outside host, potentially exposing services across the retired-container boundary. History first proposed no shipped route, reopened the contract question, corrected the overly broad test-only identity-less explanation, then bounded the refutation to the checked claimless serving path. None of those revisions establishes global compliance or retracts the conditional API gap.

Unresolved premise or limitation: A supported production caller/state exposing a retired supervisor row while its durable payload is unusable, or clarification that Delete-only refusal must also hold for the constructed full-authority API call. The checked None-identity constructor cannot serve. Actual outside-host security impact beyond already granted desktop tunnel authority also remains unestablished.

Proposed basis: Retain the genuinely investigated missing-premise claim. Other reflects the established constructed/API consequence only; any dismissal or acceptance of the earlier security-consequence lowering requires the independent drop check.

Existing parent disposition (event 8): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-investigated-cache-only-retired-TCP-gap-with-priority-gate. Parent read full048. Raw durable membership can route when display payload is undecodable, while retirement checks only decoded cache and supervisor TCP checks existence/loopback, not retired launch. Same current code falsely says supervisor checks stored launch. Identity-bearing ordinary refresh caches durable state; tested identity-less constructor cannot acquire serving ownership, so that specific route is refuted, not global compliance. Investigation leaves a supported corruption/state producer or full-authority API contract scope unresolved, with no observed runtime incident/security crossing. Retain UAI rather than claim source-confirmed production bypass; proposed other reflects only constructed API consequence, original outside-host service exposure/retired-boundary security remains for fresh priority gate. No exact cover/filter; Delete-only versus existing full-authority tunnel consent needs triage if unresolved.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g074 record](records/review-through-00de0b6c/groups/g074.json). Record ID: `l2`.

