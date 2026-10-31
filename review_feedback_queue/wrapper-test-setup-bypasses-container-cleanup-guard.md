# Wrapper test setup can bypass its container cleanup guard

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A failed wrapper-container setup can leave its real keeper outside the test's cleanup guard.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Fallible unwrap/assert work occurs before ContainerCleanup owns the created resources. The resulting private-harness
leak is source-confirmed and distinct from an in-flight late creator after panic. No real Podman test was run during
review.

Source locations: `crates/farhelm/tests/e2e/containerized_sessions.rs:524-540`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d; new test introduced by 2d98a04e3e291bf2b05e9f9bdaf2a9fc2595ee11.
Owner/ID a013/f6. TLDR: if creating the test session fails after its detached container has started, the test panics
before acquiring the only fixture guard that removes that container. Test create_session_with_extras(...).await.unwrap()
is at 524-537; even its metadata assertion at 538 precedes ContainerCleanup at 539-540. Concrete product failure path:
after tmux creates a pane, mandatory marking can fail and return SpawnFailure::Tmux (core.rs:15428-15449); when
has_session confirms it exists, create retains the live row for Stop/Delete and returns Err (core.rs:10798-10839). The
shim and mark request are independently scheduled, so the keeper may already have completed creation before that error.
This needs a launch/mark failure after container startup, not an assertion hypothetically becoming false in future code.
Keeper creation is podman run -d with an idle loop (container.rs:338-380); SPEC.md:3638-3644 explicitly says its
lifetime is independent of the agent exec. Fixture unwind has no compensating container owner: Harness fields at
harness.rs:883-904 have no Harness Drop; TmuxServerGuard at teststate/tmux/guard.rs:71-99 only snapshots and stops tmux;
TestDir at teststate/lib.rs:672-699 removes files/lock, not containers. Production supervisor has no Drop that removes
its sessions, and no container cleanup exists in this harness's destruction path. The intended explicit Podman rm guard
is containerized_sessions.rs:14-23, never reached on this route. No runtime reproduction, failure frequency, residual
resource quantity, or actual leaked container is asserted. Confidence confirmed conditional source/failure ownership
gap; adverse timing not measured. Suggested bucket other (test tooling only, no user-work/security consequence).
proposed_drop none. possible_cover none found after searching SPEC, SPEC_impl, TODO Planned (empty), BUGS, queue/index,
ledger and queue plans. SPEC retention clauses cover correct product recovery, not test teardown discarding the last
handle. This is distinct from the two preserved earlier findings. No injection routes/examples were explored.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a013/f6`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
