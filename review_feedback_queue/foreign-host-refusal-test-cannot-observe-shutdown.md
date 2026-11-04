# Foreign-host refusal test cannot observe an unintended Shutdown

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The regression meant to protect another session's terminal cannot detect a Shutdown sent before the expected refusal.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by source; no
runtime or mutation experiment.

Source locations: crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs:1162-1171 at
e354c3287db51075ae463dc61df5e257afabb6db; 1195-1204 at ca9b0a19856dc136f63ad2d14f1076027edce1eb.

service/core/tests/host_terminals.rs:1162-1171 checks refusal text, runs(child), other.received()==empty and row
retention. hosts/terminal/test_host.rs:318-327 records only Input bytes in received; :329-331 acknowledges Shutdown and
returns without ending the owned sleep or retaining a shutdown counter. A Shutdown sent over the settled connection
before returning the existing foreign-tag refusal leaves all asserted observables unchanged. Production
service/session_hosts.rs:290-301 currently rejects the foreign tag correctly.

the regression meant to protect another session's terminal cannot detect a Shutdown sent before the expected refusal.
This leaves a lifecycle-safety assertion untested; no production wrong-target shutdown is established. Source at
original commit: service/core/tests/host_terminals.rs:1162-1171 asserts the refusal text, child survival, empty received
input and retained row; hosts/terminal/test_host.rs:318-331 records Input only and acknowledges Shutdown without killing
its sleep or retaining any shutdown observable. At tip the same assertions remain at
service/core/tests/host_terminals.rs:1195-1204; TestHost still exposes only input at hosts/terminal/test_host.rs:131-134
and handles Shutdown by Goodbye plus return at :585-587. Its child remains owned by TestHost and is killed on Drop
(:101-128), not on Shutdown. Therefore even a Shutdown completed over the settled connection before the foreign-tag
refusal leaves all asserted observables unchanged. This is a deterministic source-level test-oracle gap, not a measured
flake or a claim that current production sends Shutdown. Production tip service/session_hosts.rs:406-417 still rejects
the foreign tag correctly.

Possible coverage and caveats: none exact. The real-host e2e test at tip
crates/farhelm/tests/e2e/host_terminals.rs:701-770 creates the other host's launch record under other_session
(:734-742), then checks that unrelated host survives (:765-768). It tests exclusion of another session's records, not
this unit test's specific case where the session being deleted has a record naming a foreign-tagged host; it does not
repair or duplicate f1. Coverage: SPEC_impl.md:2266-2317 explicitly requires foreign-tag refusal without touching the
host; full Testing :6742-6809 accepts no unobservable Shutdown; SPEC.md Lifecycle operations :1338-1557 supplies
ownership/refusal semantics. Planned TODO bucket :23-24 is empty. Searches of SPEC/SPEC_impl, TODO, BUGS, all unlanded
plans/queue, queue INDEX and files, and TRIAGE_OUTCOMES found no exact trigger/consequence/scope cover; earlier matching
queue/ledger shutdown cases concern different fixtures and mechanisms. No filter fully matches a deterministic missing
test observable.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g139/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is e354c3287db51075ae463dc61df5e257afabb6db;
persistence independently verified at pinned tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only
review; runtime occurrence and frequency were not measured.
