# failed-spec test accepts an attempted host start as never started

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: failed-spec test accepts an attempted host start as never started

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g108/l1`.

### g108/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 12 (finding, open):

# failed-spec test does not distinguish no start from a failed start

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513

## TLDR

The test intended to prove that a failed launch-instruction write prevents any terminal start accepts an error from an
attempted terminal start too. Later changes fix its terminal fixture and rename its assertion, but the negative oracle
still observes only final cleanup state. This is a regression-test weakness, not an established production ordering
error.

## Details

At the reviewed revision, core.rs:28545–28631 uses new_with_exe(dummy_exe()) with default seams, freezes launch/ to 0500
and accepts any ControlMsg::Error plus empty map/store/tmux. host_launch.rs:371–378 selects the deliberately nonexistent
unit-test host program; an attempted start can therefore fail for ENOENT, and definitive FailedStart::Terminal cleanup
(:11873–11946) can yield the same empty state. The test does not check EACCES or the launch-spec-publication diagnostic,
and the tmux-empty check no longer watches creates. Later commit 1ed482dde220452e424973faf38a5e1318986333 changes this
test to host_supervisor and an empty-hosts-directory assertion. At pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db
it is core.rs:26089–26164, create_session_never_starts_a_terminal_after_a_failed_spec_publish. Thus the
nonexistent-host-program premise is historical only. The replacement still accepts any Error and checks only final
directory emptiness. The alternative failure path is concrete in tip source: host_launch.rs:537–565 starts the real
first stage before writing artifacts.host(); the same unwritable launch/ then prevents this .host write if the earlier
JSON-spec write were incorrectly skipped or deferred. StartingHost is dropped without finish; hosts/spawn.rs:280–291,
:347–360 and :403–424 establish that it kills the first stage and never sends the child spec, so it binds no host
socket. FailedStart::Terminal (tip core.rs:11436 onward) can sweep/clean this confirmed-absent attempt and remove its
session row. All the test's negative final-state assertions can then hold despite a first-stage attempt having occurred
and despite failure being the host-record write rather than the promised launch-spec write. This is source-derived
counterfactual control-flow analysis, not an executed mutation or reproduction. A host's first-stage attempt is not
proof that its child or full terminal host ever ran; do not conflate them. Use an observable host-program/start seam to
assert it was never invoked, and require the specific spec-publication failure as the refusal. Final hosts-directory
emptiness alone proves no retained socket, not no attempted start. The converted provenance and confirmed-absent-cleanup
tests witness positive publication and do not isolate this negative failure boundary. Contract: the test docstring
promises 'a genuine EACCES' and pins publication before any terminal start; the supplied test-authoring checklist
requires an observable distinguishing competing mechanisms rather than a shared Error. Current production prepare_launch
still precedes start_host at original and tip; no production ordering violation is claimed. Suggested bucket: other
(test quality). Confidence: confirmed by source/contract analysis only. proposed_drop: false. possible_cover: none
known; no forbidden coverage sources read. Could-be-highest: no established route; preserved concern is regression
detection around launching work, not actual lost work. Caveats: a suite-wide mutation deleting all spec writes may fail
other tests; the issue here is this test's failed-publication/no-start oracle. Readability failure in tip's
read_dir().map_or(true, ...) also cannot establish emptiness, but no separate runtime defect is asserted. Exact
diagnostics, alternate cleanup state, later seam repair and tip persistence have been checked; no remaining premise is
open for l1.

Returned proposal (claims-070, event 12): **retain-verified**, suggested bucket **other**.

The negative test accepts any Error and observes final cleanup rather than attempted first-stage execution. At its
original revision it also watches tmux although creates use hosts. Later fixture repair removes that historical mismatch
but leaves the distinguishing no-attempt observation absent.

Original stronger consequence: Weakened regression detection around launching work; no production ordering violation,
security issue or actual work-loss route was asserted or demonstrated.

Unresolved premise or limitation: None for the bounded oracle weakness. The counterfactual establishes an attempted
first stage, not execution of the agent/full terminal host. A suite-wide mutation deleting all spec writes may fail
other tests; this finding is this test's negative boundary.

Proposed basis: Retain as a source-confirmed test-quality defect. Preserve the later partial repair and separate
historical nonexistent-program premise from the tip's still-missing attempt witness.

Existing parent disposition (event 12): verified. Parent explicitly accepted named retain-verified proposal from
claims-070 after inspecting it. The negative test accepts any Error and observes final cleanup rather than attempted
first-stage execution. At its original revision it also watches tmux although creates use hosts. Later fixture repair
removes that historical mismatch but leaves the distinguishing no-attempt observation absent. Retain as a
source-confirmed test-quality defect. Preserve the later partial repair and separate historical nonexistent-program
premise from the tip's still-missing attempt witness. Checked source/results:
["647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513:crates/farhelm-supervisor/src/service/core.rs:28534-28631 promises genuine
EACCES/no start, uses new_with_exe(dummy_exe()), accepts any ControlMsg::Error, then checks empty
sessions/store/tmux.","647f0227:service/core/host_launch.rs:371-378 selects
/nonexistent/farhelm-unit-tests-have-no-host-program without an explicit seam. core.rs:11873-11961 can sweep/clean a
definitive failed start and remove the row, or retain it if cleanup is uncertain. Thus ENOENT can supply the accepted
alternative cause under the hypothesized ordering
regression.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:core.rs:26079-26164 now uses host_supervisor but still accepts
any Error and only checks final hosts-directory emptiness (including read errors through map_or(true)).
host_launch.rs:537-565 starts the first stage before writing the companion .host record; that later write can fail in
the same unwritable launch directory if the earlier spec write is
skipped/deferred.","647f0227:hosts/spawn.rs:280-300,347-360,403-475 owns the first stage with kill_on_drop and sends the
child spec only through finish. A failed record write can kill an attempted first stage without ever starting the full
host/child or retaining a socket. This is source-derived counterfactual analysis, not an executed mutation. Production
host_launch.rs:454-468 still prepares the spec first."] Current coverage/results: ["Both specifications searched for
spec-publication/start boundaries. SPEC_impl.md:2200-2233 distinguishes first-stage identity recording, spec delivery,
confirmed absence and uncertain retention; it does not accept this weak test oracle. SPEC.md:1375-1392 requires
intended-create/retry identity, not this test implementation.","TODO.md Planned is empty; BUGS.md has no related entry.
plans/queue searched for failed-spec/spec-publication/provenance/uncertain-create terms; no stated unlanded work
specifically adds this no-attempt witness.","INDEX.md and active TRIAGE_OUTCOMES.md searched for publication, provenance
and held-start witnesses. held-start-test-lacks-prelaunch-fencing-witness.md was read; it concerns settling an earlier
Restart first stage before a later launch, a different fixture and ordering. No exact coverage or useful extension to
that item. No archive read.","FILTER.md has no boundary covering this deterministic regression-test sensitivity
defect."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Weakened regression
detection around launching work; no production ordering violation, security issue or actual work-loss route was asserted
or demonstrated."; "None for the bounded oracle weakness. The counterfactual establishes an attempted first stage, not
execution of the agent/full terminal host. A suite-wide mutation deleting all spec writes may fail other tests; this
finding is this test's negative boundary.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g108 record](records/review-through-00de0b6c/groups/g108.json). Record ID: `l1`.
