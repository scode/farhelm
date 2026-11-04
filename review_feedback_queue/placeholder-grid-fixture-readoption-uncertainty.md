# Placeholder attachment may race initial re-adoption

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A placeholder-attachment test lacks a handshake with the supervisor's initial retry scan. Source checks found no
barrier, but the complete execution causing a false failure remains unverified after investigation.

## Details

Investigation: at introducing commit `70cd68a03c101493e0adbf00bb27bb8004adfe41`,
`tests/host_terminals.rs:2258–2272,2350–2377` constructs the normal supervisor, waits for incumbent replay, then inserts
a Busy placeholder without a lifecycle claim. `core.rs:6311–6317` starts re-adoption independently;
`core/host_handover.rs:857–878,903–939,946–955,975–1033,1052–1077` supplies the initial pass, delayed retry, lifecycle
exclusion and replacement. The test host is in another directory (`hosts/terminal/test_host.rs:333–340`), so this scan's
socket lookup can return Absent (`host_handover.rs:734–757`; `hosts/registry.rs:518–524`).

Waiting for ReplayComplete does not prove that the initial pass finished: `service/grid_stream.rs:422–497,501–513` needs
no timer wait when the requested size already matches, as the fixture's 80×24 does. The current-thread test runtime and
exact Tokio 1.53.2 timer/current-thread scheduler sources were inspected. A due-now timer is rounded to the end of its
millisecond (`runtime/time/source.rs:17–29`); registration and scheduler polling provide no fixture handshake.

Missing premise: the complete execution in which replay finishes before the first pass, that pass sees the new
placeholder, and its one-second retry replaces it before the second viewer's refusal completes. Source establishes the
missing barrier but not that complete execution. A controlled executor trace or authorized targeted reproduction would
settle it; those experiments were outside this review's source-only charter. Tip `tests/host_terminals.rs:2380–2449`,
`core.rs:6555–6561`, and `host_handover.rs:1280–1302,1319–1378` preserve the interaction. Holding the existing lifecycle
claim over the fixture's insertion, assertions and restoration is a contained possible fix.

Suggested bucket: **other**. Confidence: **unverified after investigation**. unverified after investigation

Source locations:
70cd68a03c101493e0adbf00bb27bb8004adfe41:crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs:2363-2377;
persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb:2393-2407.

host_terminals.rs:2363-2375 says manually installing the Busy placeholder cannot wake re-adoption, but the constructor
launches start_readoption before returning. In a4345e86a3f51f4c5abf3b2e97737991df900142 core.rs:6307-6314 and
core/host_handover.rs:857-878, that task independently starts with next=Some(Instant::now()), not by waiting for a
notification. A delayed initial pass can therefore first see the manually inserted placeholder; readopt_pass at
host_handover.rs:903-938 schedules it after one second and can replace it during later test awaits. This test holds no
lifecycle claim, unlike the neighboring fence/backoff tests. Its original host runtime remains live, so background
probing also violates the test's intended single-runtime fixture. Fix: take and retain s1's lifecycle claim across
placeholder installation, refusal checks and restoration, or explicitly synchronize completion of the initial empty
pass.

The placeholder-attachment test assumes its fixture cannot be retried, but it never establishes that the supervisor's
independently scheduled initial re-adoption pass has completed. A delayed schedule can therefore remove the placeholder
before the test asks to attach, producing a failure unrelated to attachment refusal.

Possible coverage and caveats: no exact trigger/consequence/scope match found in initial coverage searches of SPEC.md,
SPEC_impl.md, empty TODO Planned, BUGS.md, unlanded plans/queue, queue INDEX/files or TRIAGE_OUTCOMES.md. Full matching
SPEC_impl.md Re-adoption paragraph :2380-2392 and Grid streams bullet :2442-2482 prescribe production retry and refusal,
not accepting this fixture interference; Testing :6742-6809 supplies no same-runtime-race exemption.
TRIAGE_OUTCOMES.md:11580-11595 accepts contained fixes to this test-defect class but excludes purely
speculative/new-production-hook work; whether this unobserved timing route meets the purely-speculative exclusion is a
possible coverage judgment, not silently treated as acceptance. FILTER.md Rare, self-correcting glitches and imprecise
diagnostics likewise has an unclear boundary for test-only retryable failures.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g140/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41;
persistence checked at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime
occurrence and frequency were not measured.
