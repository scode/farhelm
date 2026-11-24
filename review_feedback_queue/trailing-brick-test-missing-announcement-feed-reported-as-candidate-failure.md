# An unpublished brick-test announcement feed (HTTP 404) is reported as a candidate product failure

Reviewed commit: 356718b9d0bca3a27758ec6904688da2f54b9639

## TLDR

If the maintainer has not published (or has removed) the standing test announcement files, the brick test reports the
release candidate's announcements check as FAILED, as if the candidate app were broken, even though the app never ran
and nothing about the candidate was tested. The release report then blames the candidate for a missing fixture.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. The reviewer proposed dropping it as deliberate per BRICK-TEST.md:611-612 and RECIPE.md:243-244; the
independent drop check DISAGREED: procedure documents are not coverage sources, and they take one side of a conflict
with the tester contract in guest_tester.py:261-263.

### Reviewer record (`g10-brick-announcements/f2`)

Location: `releasing/mac-vm-test/guest_operations.py:565`. Summary: Unpublished brick-test feed (HTTP 404) is reported
as a candidate product failure

Revision of candidate f2 (orchestrator: the procedure documents are not SPEC.md/SPEC_impl.md, and the code does not show
a 404 means the candidate failed). Re-recorded as a finding.

Reviewed commit: 356718b9d0bca3a27758ec6904688da2f54b9639 (still present at tip
e159ffc79ab147be44e92a92b59571650a4f322d).

TLDR: If the maintainer has not published (or has removed) the standing test announcement files, the brick test reports
the release candidate's announcements check as FAILED, as if the candidate app were broken, even though the app never
ran and nothing about the candidate was tested. The release report then blames the candidate for a missing fixture.

Details: `announcements_feed` (releasing/mac-vm-test/guest_operations.py:539-569) probes
https://get.farhelm.io/announcements-test.json and its .minisig with curl before any app launch. On HTTP 404 it returns
`outcome: 'fail'` (line 565), and `announcements_instructions` (releasing/mac-vm-test/checkpoint_contract.py:55-76)
tells the tester to 'return its outcome'. The tester then reports status `fail`, which `brick_test.py` imports as a
product failure (record(), brick_test.py:~700-830; report P2 `announcements: fail`). Nothing in the code relates a 404
to the candidate: the files are published by the maintainer's external tooling (releasing/AGENTS.md:78-84), the probe
runs before the candidate is relaunched, and `launch_announcements` is refused afterwards (guest_operations.py:730-731),
so the candidate is never exercised. This contradicts the tester's own classification contract in managed_prompt
(releasing/mac-vm-test/guest_tester.py:261-263): 'fail only for actual Farhelm product failure, could-not-run for
unavailable product prerequisites'. Every other non-2xx and every transport error is already `could-not-run` (lines
553-567); only 404 is singled out. The test `test_missing_announcement_file_reports_404_without_starting_the_app`
(test_guest_operations.py) and `test_missing_announcements_feed_preserves_all_earlier_results`
(test_native_coordinator.py) pin the `fail` mapping. Consequence is bounded: P2 is nonblocking and the detail names the
missing file, so a careful maintainer can see the cause, but the named result and P2 verdict say 'fail', which is what a
skim of the report or an unattended release flow acts on. Fix: map 404 to `could-not-run` like other publication
problems (and update the tests and the BRICK-TEST/RECIPE text), or, if a missing fixture is meant to be loud, give it a
distinct verdict rather than the product-failure one.

Suggested bucket: other — misclassified result in maintainer release tooling; no user-facing impact.

Confidence: confirmed (source shows the mapping and the contract it contradicts).

proposed_drop: The mapping is deliberate per procedure docs, not the spec: releasing/mac-vm-test/BRICK-TEST.md:611-612
'A 404 is a P2 failure with the missing filename, independent of the other checks. An unreachable origin makes this
checkpoint unavailable.' and RECIPE.md:243-244 'A 404 fails this check with the missing filename. If the origin cannot
be reached, report could not run.' If the maintainer intends a missing fixture to read as a failure, this is working as
designed; triage decides.

possible_cover: releasing/mac-vm-test/BRICK-TEST.md:611-612 and RECIPE.md:243-244 (quoted above) document the
404-is-fail choice.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g10-brick-announcements/f2`.
