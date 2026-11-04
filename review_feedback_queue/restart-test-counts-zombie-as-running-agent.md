# Restart ordering test treats terminated zombies as running agents

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The Restart regression can fail after correctly stopping the old agent because its observation mistakes an exited,
unreaped process for a running one.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
mismatch; actual failure frequency unmeasured, no runtime reproduction authorized

Source locations: 32e20f84647951a8ea717b3c712a037c231d92dd:crates/farhelm/tests/e2e/host_terminals.rs:359-364,396-400.

The new wrapper at host_terminals.rs:359-364 uses kill -0 for each recorded PID and the test requires exactly ' gone
gone\n' at :396-400. kill -0 also succeeds for zombies. The existing harness.rs:2090-2098 explicitly treats zombies as
gone because orphan reaping is outside the test's control. Production service/sweep.rs:908-917, as located by
immutable-source search, likewise accepts zombies as confirmed absence. The new fixture at :389-390 uses an unmarked
child/grandchild stopped together, allowing the grandchild to be orphaned before its adopter reaps it.

The Restart regression can fail after correctly stopping the old agent because its observation mistakes an exited,
unreaped process for a running one. This makes validation depend on the machine's orphan reaper.

Possible coverage and caveats: No exact coverage found in SPEC.md, SPEC_impl.md, empty TODO Planned bucket, unlanded
plans, BUGS.md, queue or TRIAGE_OUTCOMES.md at ca9b0a19856dc136f63ad2d14f1076027edce1eb. SPEC.md:1512-1556 requires
stop-before-relaunch; SPEC_impl.md:2120-2194 describes that lifecycle but does not require waiting for an orphan reaper.
FILTER.md rare/self-correcting diagnostic filter does not fully cover the test oracle: an adopter that does not reap can
keep the false failure persistent, and the test currently conflates running with exited independently of any production
defect.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g143/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 32e20f84647951a8ea717b3c712a037c231d92dd. This
was a source-only review; runtime occurrence and frequency were not measured.
