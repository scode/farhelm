# Interrupted image inspection can leave an unnamed allocation

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An interrupted account inspector can leave container storage allocated before its automatic-removal cleanup becomes
effective.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Matching Podman source establishes a pre-conmon Configured allocation boundary before the ordinary --rm cleanup path is
armed. This is distinct from the named keeper's late-creator fence race and from checkpoint restoration. No initialized
user files or live-work loss is assumed.

Source locations: `crates/farhelm-supervisor/src/container/account.rs:344-361`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Final synthesis for f3 supplements the preceding complete route. Reviewed d6645f771bee8ed03a66486ef8755834937d759d;
introduction f73589c5201a0408f2f1555c18a4efca6f6057be. Retain other-bucket source-level cleanup defect, confidence
confirmed for the specified unscoped terminal interruption; no runtime occurrence or frequency claim. Full Podman v4.9.3
source search of libpod, cmd/podman and pkg finds only the libpod exit callback and API-service callback registered with
shutdown; there is no pre-start --rm cleanup callback. The API callback is installed only by APIServer.Serve
(pkg/api/server/server.go193-204), not this CLI run. Terminal ProxySignals calls shutdown.Stop only later
(pkg/domain/infra/abi/terminal/sigproxy_commn.go18-25); a shutdown goroutine already waiting on shutdownInhibit is not
cancelled by that Stop. A pending SIGINT/SIGTERM during setupContainer can run after its deferred Uninhibit but before
conmon startup. Persistence is explicit in SQLiteState.addContainer (libpod/sqlite_state_internal.go408-476, transaction
Commit at472) and the BoltState db.Update path (boltdb_state_internal.go653-659). Source checks thus settle the prior
uncertainty about another normal asynchronous reaper: none is registered for this not-yet-executed allocation.
AutoRemove itself is just a spec annotation (libpod/container.go1254-1260), acted on by ABI completion or the later
conmon exit callback. Farhelm has no inspection cleanup identity, and later Delete removes only its named main
container. Practical consequence is a disposable Configured container/storage entry requiring Podman cleanup, not a live
user workload, lost credentials, or loss/re-ownership of checkout files. The earlier creator-only death claim for an
already-running inspection is explicitly not retained: conmon has independent --rm cleanup, and a live CLI child can
finish after its creator dies. Exact trigger/consequence coverage searches remain negative; TODO185-236 identifies
separate work for agent-started containers and expressly separates experimental session containers. No existing
acceptance/filter covers persistent unnamed storage residue. Tests read use a fake inspection that allocates no Podman
state and test SIGHUP, not this SIGINT boundary. proposed_drop none; possible_cover none found. Suggested fix: retain a
separate inspection identity and cleanup handle through confirmed absence, including interrupted creator recovery. No
investigated uncertainty remains necessary for the narrowed source-level finding; runtime reproduction would validate
the stated timing on an installed substrate, not authenticate any historical private evidence.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g125/f3`, `a006/g125-prose-inspection`,
`a024/interface-a006-g125`. These are original reviewer identities for this concern, not additional bugs. Full
revisions, withdrawals and independent verdicts remain in the private review evidence. No fix or triage decision was
executed.
