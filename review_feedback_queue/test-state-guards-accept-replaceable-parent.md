# A shared temporary base can leave test-state paths under another account’s control

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

With some shared temporary directories, the ownership check can accept a test-state path another account can replace.

## Details

`deflake/bin/deflake:95–113` verifies ownership and permissions on the run root and checks the immediate parent’s owner.
It never verifies that the directory entries used after validation are protected against substitution elsewhere in the
selected temporary path. Deflake later prepares and executes its worktree through that path
(`deflake/bin/deflake:1094–1148,1332–1362`). The test recorder uses a related owner-only check at
`scripts/record-test-run.py:275–307,1442–1482`. The affected configuration is a supported custom shared temporary path
whose directory-entry permissions allow another account to replace part of the tree. Validate the whole ownership and
permission boundary and bind later filesystem operations to the verified directory identities. The ordinary sticky,
root-owned temporary directory with a protected child does not by itself establish the premise.

Confidence: confirmed by inspection for the configured permission premise. The ordinary sticky system /tmp plus
caller-controlled child does not supply it.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `releaseGate/l2`, `upgradeBackend/l4`.
