# Discovery registration bypasses an executing row lock

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Discovery registration can bypass a host row's execution lock, but a supported differing-path transition with lasting
consequences remains missing.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Source checks establish the lock-boundary difference. Ordinary Update uses in-place paths and does not supply the
suspected change. The remaining premise is a compatible ordinary discovery transition that changes those paths while
execution relies on them; no wrong-target or work-loss route is claimed.

Source locations:
`crates/farhelm-helm/src/provisioning/service.rs:566-697,777-878; crates/farhelm-helm/src/store.rs:6036-6081`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

a054/d1, reviewed d6645f771bee8ed03a66486ef8755834937d759d. Revision preserves initial investigation event. Source
basis: probe_owned holds a planning slot, not the host provision lock or busy claim; an answering or skewed supervisor
reaches register_discovered -> register -> register_probed_ssh_host, which rewrites saved dial paths for an existing
destination. Existing-entry GUI Setup actually sends a no-new_entry probe with its row's saved coordinates
(ui/provisioning.rs:1599-1621; api.rs:4520-4551); HTTP dispatch adds no lock (provisioning/http.rs:200-209). The backend
returns the probe's state-dir argument and its command-v-resolved executable (backend.rs:1273-1288,1971-1998,2033-2059).
Checks also found the important limit: ordinary Update plans use the registered executable/state paths in place
(service.rs:1127-1165,plan.rs:942-971), so merely overlapping Setup and Update does not establish different coordinates
or lasting damage. Suspected consequence: an actually differing compatible discovery reply could rewrite coordinates
under a frozen executing plan, making later connection/attachment use the wrong saved path. Exact missing premise: a
supported shipped ordinary sequence producing a differing reply while a same-ID run owns its provision lock, plus a
downstream effect beyond the accepted reconnect. Source permits the write, but inspected ordinary producer and in-place
Update do not establish that conjunction. Resolving evidence: complete source-backed host/path transition and callback
ordering, or authenticated observations tied to that supported topology; no runtime tests or experiments were run.
Suggested bucket other on the conditional contract concern, no established highest route. Confidence unverified after
investigation, not a verified durable-path defect. proposed_drop none. possible_cover: TRIAGE_OUTCOMES.md:13010-13019
probe-reregister-drops-terminals.md discards self-healing reconnect and explicitly calls the during-Update row rewrite
unverified; whether that whole discard settles a newly established lasting-coordinate consequence is ambiguous. No
Planned/plan/BUGS/queue exact coverage was found by candidate searches. Distinct from a030/d1's
destination-owner-changing confirmed-ADD sequence; related registration seam, not claimed independent harmful
occurrence.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a054/d1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
