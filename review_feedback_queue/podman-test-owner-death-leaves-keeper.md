# Podman test owner death can leave its keeper

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Abrupt death of a real-Podman test owner can leave its detached keeper outside cleanup.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Drop-only cleanup cannot run after abrupt owner death, and the general test-state sweep does not own Podman container
removal. Matching Podman/conmon lifetime checks narrow the distinction between process-group cleanup and detached
storage/process ownership. This does not claim every timeout leaves a container, and no interruption experiment ran.

Source locations: `crates/farhelm/tests/e2e/containerized_sessions.rs:14-22`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Discovery precision revision of a037/f5, reviewed d6645f771bee8ed03a66486ef8755834937d759d. Retain OTHER, confirmed
conditional source lifecycle gap, no runtime reproduction. Narrow proven trigger: abrupt termination of the test-owner
process after its independent production container is running, without an external cleanup owner stopping that
container. Do not infer Unix process-group separation solely from a separate systemd cgroup: scope.rs:39-51 and 895-946
show an exec-in-place scope prefix and no parent tether or runtime limit, but cgroups and process groups are different
boundaries. The repository's 300-second nextest timeout demonstrates an owner-termination mechanism, not proof that
every runner signal policy leaves every Podman process alive. The invariant source basis remains that the fixture owns
cleanup only via in-process Drop (containerized_sessions.rs:14-22), the launched keeper is podman run -d with an
infinite loop (container.rs:328-381), production explicitly preserves containers across agent exit (SPEC.md:3638-3651),
and teststate recovery kills private tmux servers then removes directories without Podman cleanup
(farhelm-teststate/src/lib.rs:430-485). The scoped source uses a disjoint unit (container.rs:490-525;
SPEC_impl.md:5774-5787), with no expiry or owner-death tether in its prefix; --collect only collects inactive units
(scope.rs:895-946). Host-wide shutdown, deliberate unit cleanup or external operator cleanup may of course stop it. No
universal claim about every unscoped runtime, timeout implementation or external host policy. Possible broader coverage
is still the different finite-child ledger decisions at TRIAGE_OUTCOMES.md:20307-20336; they do not exactly cover this
fixture's persistent container lifetime. proposed_drop none. No security, user-owned work loss or catastrophic tooling
consequence established. The wrapper's pre-guard selection assertion was checked further: compilation only fills omitted
permission choices (launches.rs:302-338) while this fixture supplies Guarded, the compiled launch retains selection
(487-491), the client passes launch unchanged (client.rs:2811-2851), and the supervisor stores/clones it
(service/core.rs:10117-10150,15709-15725). No current returned-selection mismatch producer was established. Thus that
assertion-order sublead is not a separate finding; its generic regression-unwind limitation remains documented and is
not the proof of f5.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a037/f5`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
