# The process-presence fixture does not clean up its sleep descendant

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Running the process-presence unit test can leave its sleeping helper behind even when the test passes.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
ownership defect; no runtime occurrence or survivor duration is claimed.

Source locations:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/registry/tests.rs:779-793.

tests.rs:746-759 deliberately prevents shell exec optimization by running /bin/sh -c sleep 30; : and sets kill_on_drop
only on the shell. At :760-763 the test scans, kills that shell and waits for that shell alone. Once the shell has
started sleep, SIGKILL of the shell cannot run descendant cleanup; the test never owns or signals sleep. The default
test wrapper does not add process-tree cleanup
(ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-testtrace/src/lib.rs:994-1025). The locked Tokio 1.53.2 Unix
Child kill delegates to std_child().kill() (local package src/process/unix/mod.rs:173-177); no group kill is implied.
Fix with a childless blocking stand-in or effective descendant cleanup ownership armed at acquisition, retaining the
argv premise.

the fixture runs /bin/sh -c sleep 30; : with kill_on_drop(true), then calls start_kill and wait only on that shell. The
comment explicitly keeps the shell from replacing itself with sleep, so sleep is a separate child outside Tokio Child
cleanup ownership.

Running the process-presence unit test can leave its sleeping helper behind even when the test passes. Tip persistence
verified at crates/farhelm-supervisor/src/hosts/registry/tests.rs:776-793: the comment preserves the shell instead of
execing sleep, the shell runs sleep 30; :, kill_on_drop(true) covers that shell, and success cleanup start_kill/wait
operates only on the shell. Once sleep has started, killing its shell cannot run descendant cleanup. The owning per-test
Tokio runtime does not add process-tree cleanup (same tip crates/farhelm-testtrace/src/lib.rs:994-1025). The locked
Tokio 1.53.2 Unix Child kill delegates to std_child().kill() (dependency src/process/unix/mod.rs:173-177). A childless
blocking stand-in or effective descendant cleanup owner preserves the process-argv premise without leaving that child
unowned.

Possible coverage and caveats: none exact. Correction to the initial grouping suggestion:
review_feedback_queue/signal-test-child-lacks-unwind-cleanup.md:58-90 concerns failure-before-owner gaps; this
normal-success direct-shell cleanup leaving a separate descendant is a different cause, and should remain distinct under
the parent synthesis direction. SPEC_impl.md:3820-3861 limits the production spare-builder contract, not this fixture;
Planned TODO is empty; the named ledger fixtures in the prior f1 event are different scopes. No matching coverage or
FILTER acceptance was found.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g115/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
introduced by c0a0c9fb126f0a6d05d72a245f19cd6288b11cbc at tests.rs:749-763.. This was a source-only review; runtime
occurrence and frequency were not measured.
