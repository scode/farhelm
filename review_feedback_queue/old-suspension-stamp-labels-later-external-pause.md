# Old suspension stamp labels later external pause

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a

## TLDR

A suspension remembered from an earlier operation can be reused to describe a later independent pause, misattributing
its timing.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/manager.rs:3840 at 1a57aa8e265cbb59cedc228433b189f59f7ad59a`.

Source assessment (read with the later corrections below) (review record g049/l1, event 9):

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a. Origin: lifecycle timestamp writes activated by
8a7f66ede90651d47d1facc1e877ce81f842af1a (the assigned store.rs patch removes the setter's dormant annotation); not
repaired through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: after a sandbox is resumed outside Farhelm,
a later independent or idle suspension can display the source and completion time of its earlier Farhelm suspension.
Exact source route at reviewed commit: tensorlake/lifecycle.rs:107-118 saves Some(completion_time) for Farhelm Suspend
and None only for explicit Farhelm Resume. The actor's status gate maps positive vendor Running to Dial
(manager.rs:3387) and attempt dispatches that result straight to dial (:4151-4155), with no timestamp write. Successful
connection publication (:4508-4519) and refresh publication (:4602-4613) replace the live Suspended state with
Connected, so the old snapshot-local attribution really is discarded; this finding does not claim it persists inside
Connected. The durable host-row stamp survives. Once the externally resumed connection later ends, the next loop reloads
its row (:3657; reload_row :4973-4982). store.list_hosts (:6315-6421) is a SELECT: the suspension column is read at
:6326 and passed through at :6387-6395; tensorlake_binding (:4641-4663) copies suspended_at unchanged. The new
independent Suspended result then rehydrates the old durable value at manager.rs:3838-3844. hosts.rs:538-550
additionally uses the same registry fallback. The user-visible consumer, crates/farhelm-ui/src/hosts.rs:1153-1162,
renders any Some(at) as 'Suspended from Farhelm at ...' instead of 'Suspended outside Farhelm, noticed at ...'.
Rechecked alternative clearing paths: manager.sync_registry (:1679-1880) only reconciles actor handles from list_hosts;
its non-reconfigured branch assigns the fresh row to handle.row (:1849), without writing the store. reconfigured
(:1433-1437) compares transport coordinates, not attribution. Reconnect identity admission only updates host_identity
(store.rs:7619-7669). Session refresh uses replace_host_sessions (:7877-8004), which rewrites session_cache and updates
only hosts.cache_truncated (:7994), not the suspension stamp. Dynamic host permission SQL (:7350-7418) uses
BrowserPermission's closed column set (:998-1005), so it is no hidden stamp clearer. Tensorlake account refresh/save at
pinned tip, tensorlake/config.rs:114-159, reads/updates only tensorlake_account.config. Tip-wide git grep for
suspended_at/tensorlake_suspended finds no other normal production stamp mutation beyond the lifecycle caller and store
setter. No INSERT OR REPLACE/REPLACE INTO hosts path was found. The pinned-range path history shows the later
lifecycle/manager modification is #3048, which adds snapshot-carried attribution but leaves the persisted-value lifetime
unchanged. SPEC.md Topology at origin:269-272 requires details to say since when and whether Tensorlake independently
suspended the sandbox; the requirement remains at the reviewed commit. Keep authority separate from diagnosis: vendor
status still controls Running versus Suspended and whether dialing occurs; stale attribution does not itself block
connection, resume a sandbox, or lose processes/data. Source-derived reproduction: Farhelm Suspend at t1; external
Resume; actor positively observes Running and establishes Connected; external/idle Suspend at t2; actor observes
Suspended and details show old t1/Farhelm source. A future runtime regression should exercise exactly these two distinct
suspension periods. Fix direction: retire prior attribution on established running evidence under ordering that cannot
erase a newer explicit suspension confirmation; retain attribution across probes of the same suspension. Suggested
bucket: other (diagnostic source/time error only). Confidence: confirmed by source inspection, not runtime/vendor
validation; external resume and independent suspension are explicitly supported in the governing contracts.
proposed_drop: false. possible_cover: parent is sharing this named attribution finding with lifecycle/manager interface
owners; no queue/coverage search performed. Caveats: no runtime tests, cloud calls, HTTP experiments, or process actions
were performed or authorized. Synthesis resolved the store/config-refresh alternatives; no substantive open lead remains
for l1.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g049/l1`; supporting records
`g043/l5`, `g044/l6`, `g056/g056-l12`, `g049/iface_g056_l12`, `g040/g049:l1`, `g034/c6`, `g045/interface-g049-l1`,
`g056/g056-c14`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
