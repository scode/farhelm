# Sandbox list pagination contract unresolved

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246

## TLDR

Sandbox and snapshot listings use one response and ignore possible continuation cursors. Whether the configured
production origin paginates these results remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/tensorlake/api.rs:107-113,211-215,349-370 at da24fb6822d289a144e18677cdd248cc35093246`.

Source assessment (read with the later corrections below) (review record g050/l5, event 24):

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246; unchanged pagination/HTTP code through pinned
8a8c9579e04299410652f0e4681975a7198dfc7b (only later api.rs edit adds max_pending_secs). TLDR: If Tensorlake returns its
supported engine-style cursor pages, Farhelm silently reports a complete listing after the first page. Test would
undercount, orphan discovery would omit sandboxes, and snapshot sweeps could forget later billable snapshots. Confirmed
consumer mechanism: SandboxPage reads next_cursor only (:107-113), ignoring unknown cursor; list_all_sandboxes(:349-370)
returns success when next_cursor absent. SnapshotList(:211-215) exposes no cursor and http.rs:266-268 makes a single GET
snapshots. Immutable pre-change primary source
https://raw.githubusercontent.com/tensorlakeai/tensorlake/aaf2f7e586f438d0d8daf92470e0aecc06efe832/crates/cloud-sdk/src/sandboxes/models.rs
:805-811 and :939-945 explicitly documents/decodes response cursor for sandbox and snapshot pages. Same commit
sandboxes/mod.rs:67-78 extracts cursor, :718-758 follows it on GET sandboxes, :1014-1019 on GET snapshots; later queries
add cursor and limit=100. Primary tests
https://raw.githubusercontent.com/tensorlakeai/tensorlake/aaf2f7e586f438d0d8daf92470e0aecc06efe832/crates/cloud-sdk/tests/sandboxes.rs
:1530-1556 and :1607-1625 confirm exact raw JSON cursor spelling and identical top-level REST path/query (read, not
executed). This rules out a Python-only output alias or archive-pagination confusion. Consequence callers at Farhelm
tip: mod.rs Test counts returned sandboxes; orphans discovery consumes list_all_sandboxes; build.rs:1259-1297 sweeps
snapshots from list_snapshots then clears builder record, hosts.rs:197-219 lists/deletes matching snapshots then returns
SandboxDeleted, orphans.rs:232-243 likewise reports cleanup success after the incomplete list. Original account commit
has only Test listing caller; destructive/removal/build cleanup consequences arise in later caller commits, not that
original slice. Contract SPEC_impl.md Tensorlake account promises Test lists every page; original SnapshotList comment
claims a one-response observation on 2026-11-03. Critical investigation qualification: upstream docs explicitly
distinguish paged Sandbox Engine from one-response Indexify Server, and immutable SDK lib.rs:103-163,:449-458 selects
sandbox.tensorlake.ai for lifecycle calls, while Farhelm intentionally pins api.tensorlake.ai and follows no redirects.
No checked public producer/route implementation or retained trace establishes whether Farhelm's exact origin already
returns cursor pages, proxies to that engine, or remains the one-response service. Do NOT state that these are observed
omitted production pages or that the original live observation is false. Thus actual current user impact is unverified
after investigation; the consumer/schema incompatibility is source-confirmed. Missing premise: authoritative
routing/response contract or retained paginated response from the pinned api.tensorlake.ai sandbox/snapshot routes,
including nonempty cursor. Public SDK tests establish another supported service's shape, not historical deployed
routing; public SDK has no private gateway producer. A controlled stub would confirm local omission only. No
runtime/API/cloud/network experiment or credential use performed; only read-only immutable primary-source retrieval.
Suggested bucket other for established compatibility gap; high if the paginated producer at the pinned origin is
established because discovery and billing cleanup become silently incomplete. No highest data/security-loss route
claimed. Confidence confirmed transformation, unverified deployment premise. proposed_drop false; possible_cover unknown
(parent handles coverage). Distinct root: ignored continuation metadata for successful listings, unlike known
structured-diagnosis decoding, deadline overshoot, or project-key foreign-ID status. Suggested fix: support actual
cursor spelling(s) and bounded pagination for both resources, retain existing no-cursor behavior, and never attest
complete snapshot cleanup until all pages are read; preserve no-retry mutation policy. No open lead remains under l5.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g050/l5`; supporting records
`g071/c22`, `g032/g050:l5`, `g032/g032-c2`, `g065/g050-l5`, `g073/f6`, `g074/f15`. The retained private log preserves
earlier wording and its corrections. This record does not claim runtime reproduction.
