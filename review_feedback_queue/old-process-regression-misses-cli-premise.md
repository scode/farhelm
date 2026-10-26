# The old-process regression also lacks its CLI premise

Reviewed commit: b1eed46975aab16ccc2217b799d5d9a045d8e979

## TLDR

The old-running-version regression can fail because its fixture omits a CLI observation, even if the intended
old-process guard regresses. The production guard remains correct.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `releasing/mac-vm-test/test_guest_operations.py:199`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Later-turn synthesis retains f1. Reviewed commit: b1eed46975aab16ccc2217b799d5d9a045d8e979. TLDR: the release regression
can stay green after losing the old-running-version check. test_guest_operations.py:195-204 supplies
running_versions=['0.29.0'] but no cli.stdout; guest_tester.py:291-294 rejects the missing CLI with the identical
running-version error even if the running-version predicate is absent. The ordinary update-relaunch contract requires
only completed versions (checkpoint_contract.py:16,30-36), which the fixture supplies, so the error is not an
omitted-operation rejection. The sibling wrong-CLI test at test_guest_operations.py:220-229 isolates CLI mismatch;
process extraction at 345-363 and native restart handoff at test_guest_ui.py:80-112 exercise different boundaries and do
not test this managed_result predicate. Current managed_result, restart and extraction guards remain correct. The
reviewed tests and relevant implementation files have no diff through pinned tip
463cac1528b5c5e259404975380d321a3fff0412. Suggested bucket: other. Confidence: confirmed from source; no runtime or
mutation test. Caveat: test-oracle gap only, not a current false release pass or production restart defect.
possible_cover: none found in the pinned coverage-source searches recorded in the prior event. proposed_drop: none. No
source-premise gap remains for the test-oracle claim.

Provenance: `g313/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
