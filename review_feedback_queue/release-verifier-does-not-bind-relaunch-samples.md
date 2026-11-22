# The release verifier does not bind reopen continuity samples to app relaunch

Reviewed commit: 2d1bb2e4ef0113729c79e3ea7e2648c262669cd1

## TLDR

The release verifier can accept continuity samples without establishing that they span the app relaunch.

## Details

**other.** Source inspection.

2d1bb2e4ef0113729c79e3ea7e2648c262669cd1 releasing/mac-vm-test/guest_operations.py:319–337,824–859;
guest_tester.py:292–296,553–594; brick_test.py:378–407,705–708

A passing bonus check can attest that a session process survived an app reopen without independently proving the two
samples spanned that reopen. BRICK-TEST.md:586–594 requires a separate before/after pair spanning normal quit/reopen.
The prompt in guest_workflow.py:514–517 requests this order, but checkpoint_contract.py:124–127,174–199 and
guest_tester.py:291–296 enforce only operation membership and completion. guest_operations.py:824–837 reads a
pass-scoped before sample or substitutes a new one, without checking current-turn launch evidence, and
brick_test.py:378–407 checks process identity and counter progress only. Native validation in guest_tester.py:553–594
checks general UI provenance but adds no reopen transition constraint. This gap predates the patch’s move to
attempt-local receipt filenames. An actual model deviating from the prompt and a release affected thereby are
unverified. Require the before sample from this attempt to precede an independently observed app stop/relaunch and gate
the after sample on that transition. The reviewer suggested other (bonus release verification only), confirmed missing
invariant, , Possible coverage: null.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `test-and-release-tools/trl-07`.
