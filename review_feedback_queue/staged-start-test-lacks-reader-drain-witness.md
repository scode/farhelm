# The staged-start test can miss an early spec delivery

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The new regression test can pass without protecting the boundary that prevents a terminal from starting before its owner
is recorded.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed test-oracle
gap, not an established production defect or runtime occurrence.

Source locations: crates/farhelm-supervisor/src/hosts/spawn.rs:1161-1199@ca9b0a19856dc136f63ad2d14f1076027edce1eb.

at both revisions, spawn.rs:1161 creates the empty read file, publishes the shell PID, and only then runs /bin/cat to
copy stdin. PID publication (:1172-1175) therefore does not establish reader progress before the empty-file observation
(:1187). After dropping StartingHost (:1189), the test waits only for that shell PID to stop (:1191-1198), not for the
stdin reader to drain or finish, before checking the file again (:1199). A schedule with the shell paused after PID
publication permits the test to kill it before cat even starts; another permits cat to remain unscheduled until after
both assertions. Thus an implementation that prematurely writes spec bytes into the pipe can satisfy these observations.
The missing reader-completion observation is confirmed from the test source; this is a conditional false-pass mechanism,
not a measured flake, and no faulty implementation was constructed or run. Use an owned reader and observe its completed
EOF/drain before judging the byte record.

assigned p0410.diff lines 361-407; spawn.rs:350-369 and 1154-1206 at the reviewed commit.

Possible coverage and caveats: none found. Exact coverage review at pinned tip: SPEC_impl.md:1763-1774 and 2023-2077
require withholding the spec until ownership is recorded, not acceptance of this test gap; SPEC.md:2127-2165 specifies
terminal behavior; TODO.md:23-25 has no Planned items. Targeted immutable-tip searches of SPEC.md, SPEC_impl.md,
TODO.md, unlanded plans/queue, BUGS.md, queue INDEX/matching records and TRIAGE_OUTCOMES.md found no exact
fixture-trigger/consequence/scope match. Existing signal-test-child-lacks-unwind-cleanup.md:1-90 and
inherited-lock-test-self-dup.md:1-26 concern different fixtures. FILTER.md:23-46 does not explicitly cover this
regression-oracle defect. Source checks also confirm production start_host does not write the spec (spawn.rs:349-379),
finish does (:420-425), and current create/tab callers record the starter before finish (host_launch.rs:548-574;
host_tabs.rs:629-652). No production early-delivery or user-work-loss claim is made.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g020/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is cf5188fd16a5cd4dec1e57fc257f0f08037f636d,
retained at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence and frequency
were not measured.
