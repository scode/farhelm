# A refused ADD can rewrite the new destination owner

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A racing ADD can change another busy host entry's install paths before returning Busy.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

ADD claims the original destination owner, then a concurrent retarget can move the destination to a different busy
entry. Registration overwrites that newly resolved owner's durable install paths/reconnect state before start_run
notices Busy. The shipped new-entry producer is established and the behavior predates the reviewed range. Durable
mutation prevents treating the whole result as a rare safe refusal; wrong-machine harm or a security consequence was not
established.

Source locations: `crates/farhelm-helm/src/provisioning/service.rs:1000-1045`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. New original owner/ID a030/d1, not g140/f5. TLDR: when a
destination changes registry owners while a setup confirmation is being admitted, the refused setup can still replace
the new owner's saved executable/state-directory paths and reconnect it. Source-confirmed conditional interleaving; no
observed occurrence, frequency or practical latency claimed. Producer established: a host row's Set up again uses
prepare(Add) and probe_ssh_host(...,None) (UI provisioning.rs:1599-1644; api.rs:4519-4551), so its pending Add
intentionally converges on an existing destination, unlike the add dialog's new_entry marker (SPEC_impl.md:4085-4093).
service.rs:1000-1017 claims busy only for the id found by an initial destination lookup, without taking its provisioning
lock. A second GUI may retarget that row before its run lock exists (UI hosts.rs:2170-2206 -> api.rs:4746-4752 -> helm
hosts.rs:908-940), then retarget another entry to the now-free destination; update_ssh_destination preserves the second
entry's custom paths (store.rs:6300-6305,6363-6374). That entry can admit a manual/automatic Update and remain busy
while its queued child or probe waits. The delayed Add's register resolves the destination again and writes plan paths
to that different entry (service.rs:1019-1025,845-878; store.rs:6036-6079), before checking its busy marker at
service.rs:1038-1045 ->1754-1756,1771-1773. register also synchronizes/retries at881-888; manager.rs:1514-1517,1610-1665
applies changed dial coordinates and retires/reconnects the client without taking the provisioning lock. Only after that
does Add receive Busy; the late-Busy branch does not restore the row or retain this plan. No provisioning guard or busy
check in registration serializes the newly selected id; add_confirmations excludes only other Add confirmations, not
retarget or Update. Consequence route: choose a second entry whose saved custom install paths differ from the Add plan.
Keep its admitted Update queued/not yet revalidated through the Add's store write and later Busy check. When that Update
resumes, require_registration_unchanged at2123-2143 fails against its original registration; finish_preflight_failure
records failure and clears busy without restoring paths. The wrong saved paths survive both refusals and ordinary actor
refresh/retry, which read the registry; a later explicit repair is needed. No arbitrary client, recycled ID or new entry
marker bypass is required. Existing test provisioning.rs:4080-4162 verifies no row changes for a same-id busy Add, but
supplies no intervening destination-owner change and cannot catch this variant. History: exclusive-base
service.rs:951-996 already has the same ordering, so pre-existing, not introduced by04e8b3f. Suggested bucket other;
confirmed conditional durable configuration mutation, no established wrong-machine execution, security or user-work
loss. proposed_drop none. possible_cover ledger TRIAGE_OUTCOMES.md:3638-3656 is the earlier same-id repair and intended
busy-before-write contract, not accepted behavior or an unlanded repair of this re-resolution; parent should compare any
adjacent registration review, but the known Busy-only duplicate cannot cover the established write-before-refusal
consequence. Coverage searches across SPEC.md/SPEC_impl.md, empty TODO Planned, BUGS.md, plans/queue, queue INDEX and
files, TRIAGE_OUTCOMES.md found no exact accepted trigger/consequence/scope. SPEC.md:252-262 permits logical-entry
retarget semantics but does not accept a rejected setup changing another busy entry's saved install coordinates. Rare
safe-retry/diagnostic filters do not cover the persistent registry write. No highest route claimed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a030/d1`, `a054/a030-d1`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
