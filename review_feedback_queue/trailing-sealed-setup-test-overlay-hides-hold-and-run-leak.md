# Sealed-setup regression test cannot observe its claimed no-hold boundary

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the new regression test can pass even if refused setup takes a connection hold or leaves
a provisioning run behind, although its name, docstring and assertion claim to protect those boundaries. This is a
test-oracle defect, not an observed production failure.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g044/l8`.

### g044/l8

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 19 (finding, open):

Reviewed commit: 222bf6253bcd168db04bcbb1df20180c88306998. TLDR: the new regression test can pass even if refused setup
takes a connection hold or leaves a provisioning run behind, although its name, docstring and assertion claim to protect
those boundaries. This is a test-oracle defect, not an observed production failure. Details:
setup_of_a_sealed_sandbox_is_refused_before_it_takes_the_host at tensorlake_add_tests.rs:2033-2068 records note_sealed,
compares manager.state before/after start_add (2047-2048,2058-2061), and checks only that backend events are empty
(2063-2067). manager.state at manager.rs:2237-2238 delegates to status, whose 2214-2224 replaces the actor's state with
sealed_state for any recorded seal. sealed_state at 2669-2679 returns Deleting solely from the independent sealed map.
Meanwhile hold_connection at 1655-1695 does not refuse a sealed host: it changes underlying actor state, removes its
client, advances incarnation and sets status.hold at 1663-1672 without changing that map. Thus moving the refusal in
tensorlake_add.rs:894-900 to after the hold at 905-913 would leave both compared public states equal, retain the same
being-deleted error and execute no provisioning backend action; this test would not distinguish the forbidden hold from
the intended ordering. Independently, start_add inserts memory.busy and memory.runs before this guard (866-874), then
removes them in the refusal branch (895-897), but this test never reads either memory or service.view, despite
explicitly promising 'leaves no run behind' at test line 2037. Omitting that cleanup is another concrete competing
mechanism invisible to its assertions. Current production performs both the ordering and cleanup correctly; that does
not refute the regression-test gap. Suggested fix: observe the actor hold boundary directly through a narrow fixture
oracle (or a stable raw/connection incarnation observation with its premise established), and assert no retained
run/busy entry after refusal. Keep the public deletion-state and backend-empty checks for their distinct guarantees.
Contract: supplied Test authoring review requires an observable that distinguishes competing mechanisms and the premise
before the action; root Docstrings on tests requires the declared what/why to match the actual test. Suggested bucket:
other, maintainer regression assurance. Could-be-highest: no security or work-loss route is alleged; the finding is test
evidence. Confidence: confirmed source-level indistinguishability, not executed mutation or reproduction. proposed_drop:
false. possible_cover: none; supplied seal runtime records g027/l4 and g028/l3 concern other failure bookkeeping paths,
not this assigned test. Attribution: the complete test is added by assigned 222bf625; full pinned path history has no
later change to this test, and endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:2033-2068 retains it exactly. Original
suspected test-state consequence is preserved and settled; no claim of a product failure, accepted drop or severity
lowering. No runtime tests, reproductions or live-install actions. Finishing checks complete: backend_events at
tensorlake_add_tests.rs:339-345 only reads and decodes backend/events.jsonl; it does not inspect actor holds,
incarnation or provisioning memory. The fixture has no Drop assertion. The complete later diff for this test file
through the pinned tip is empty. Other tests in this module explicitly inspect service.memory.busy or service.view, but
the new sealed-setup test invokes neither and uses an independent fixture, so those assertions do not establish its
no-run premise. No readily checkable premise remains unresolved for the stated test-oracle defect.

Returned proposal (claims-028, event 19): **retain-verified**, suggested bucket **other**.

The test's public-state equality cannot detect the hold it claims to exclude, and it never checks that run/busy
bookkeeping was removed. This is a test-oracle defect even though the implementation currently does both correctly.

Original stronger consequence: The regression may pass if refused setup takes a connection hold or leaves an admitted
run/busy entry. This remains maintainer regression assurance, not a claimed product deletion or user-work-loss defect.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain a source-verified test finding. Add a direct hold/incarnation oracle and a no-run/no-busy
assertion; preserve the existing checks for their separate guarantees.

Existing parent disposition (event 19): verified. Parent explicitly accepted named retain-verified proposal from
claims-028 after inspecting it. The test's public-state equality cannot detect the hold it claims to exclude, and it
never checks that run/busy bookkeeping was removed. This is a test-oracle defect even though the implementation
currently does both correctly. Retain a source-verified test finding. Add a direct hold/incarnation oracle and a
no-run/no-busy assertion; preserve the existing checks for their separate guarantees. Checked source/results:
["222bf6253bcd168db04bcbb1df20180c88306998:crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs:2033-2068
compares manager.state and checks only refusal wording/backend events. Its docstring promises refusal before a hold and
no retained run.","Same revision, manager.rs:2214-2238 overlays the public state with sealed_state; :2669-2679 derives
Deleting from the separate seal map. hold_connection at :1655-1695 mutates raw state, client, incarnation and hold
without changing that map. Moving the refusal after that hold would preserve both public-state
values.","tensorlake_add.rs:866-874 inserts busy/runs; :894-900 removes them for the sealed refusal. The test never
observes either map or service.view. backend_events at test lines 339-345 only decodes the event file and supplies no
hidden actor/memory oracle.","git diff
222bf6253bcd168db04bcbb1df20180c88306998..00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db for this test file is empty. The
complete test is in-range. No mutation test or reproduction performed."] Current coverage/results: ["SPEC.md:1511-1533
establishes the seal and deletion/Retry behavior. SPEC_impl.md seal/setup/hold searches found no exception accepting an
oracle that cannot observe its claimed boundary.","TODO.md Planned and plans/queue/INDEX.md are empty. BUGS.md has no
related test or seal issue.","INDEX.md read in full; targeted queue and active TRIAGE_OUTCOMES.md searches for sealed
setup, hold and this test found no exact coverage. Other lifecycle/runtime seal defects are distinct from the assigned
test's missing observations. No FILTER.md boundary covers ordinary missing regression sensitivity."] Exact cited cover:
[] Original stronger consequence and missing premise remain preserved: "The regression may pass if refused setup takes a
connection hold or leaves an admitted run/busy entry. This remains maintainer regression assurance, not a claimed
product deletion or user-work-loss defect."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g044 record](records/review-through-00de0b6c/groups/g044.json). Record ID: `l8`.
