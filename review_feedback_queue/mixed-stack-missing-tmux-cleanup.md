# Shared restart fixtures lose cleanup ownership of old terminals

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

An old-terminal migration test that fails can leave its private server and agent running after its state directory is
removed. The shared test fixture lost the guard that owned those processes. This is a conditional tooling cleanup
defect; no actual leftover was observed.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/host_restarts.rs:30-46`;
`crates/farhelm/tests/e2e/host_migrations.rs:343-355,470-489; crates/farhelm/tests/e2e/host_restarts.rs:30-78`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Reviewed change: f42178f9402ce22c60530360edc10d6721ef5b16, surviving at the pinned tip. host_restarts.rs:30-46 removes
Stack's _tmux guard on the premise that its supervisors create hosts. Several users of that same Stack explicitly create
tmux terminals: host_migrations.rs:39,71-75,343-355; host_tabs.rs:432-445,615-636; host_reclaim.rs:623-637. The owner
audit also examined host_restarts and host_relaunches, including Interrupted.stack:748; those inspected branches are
host-only and are not additional leak examples. No other Stack imports, reexports or Drop implementation were found.

Before the change, _tmux was the first field (parent host_restarts.rs:30-49), so its Drop collected diagnostics and
stopped tmux before state removal. TmuxServerGuard's owner is farhelm-teststate/src/tmux/guard.rs:71-99. TestDir::drop
atfarhelm-teststate/src/lib.rs:741-765 ends hosts only, then removes the directory while retaining its lock. The
distinct dead-run sweep:443-495 can stop tmux before deleting a directory, but cannot discover a directory already
successfully removed. host_reclaim.rs:633-637 supplies a concrete unwind window after retiring the tmux supervisor and
before creating hosts, avoiding a dependency on an active supervisor's teardown race. If a test unwinds with its old
server still serving and TestDir removes its directory, that server and possibly its program can survive without their
socket or later sweep handle. Successful explicit Delete and last-window-close paths may clean up normally and are
outside the claim.

Restore a tmux cleanup owner for Stack callers able to start old terminals, or separate their constructor while keeping
field/drop ordering explicit. SPEC_impl.md:2141-2157 retains migration tests as real old-terminal subjects; replacing
their terminal API does not remove their process ownership. Suggested bucket other because this is noncatastrophic
maintainer fixture cleanup, not an established loss of production sessions. Confidence source-confirmed ownership
regression, conditional on the named failure/unwind and successful state-directory removal; no runtime leak was
measured. proposed_drop false; no exact existing coverage was established.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g042/g042-l4`,
`g041/g041-s1`, `g046/g042-l4`. The retained append-only histories preserve all earlier claims and corrections. Reviewed
source references name their own revisions; the inclusive reviewed tip is the hash above. This review used source
inspection only, with no product runtime experiments or live-install changes.
