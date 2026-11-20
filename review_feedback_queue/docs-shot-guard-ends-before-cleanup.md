# Screenshot cleanup runs outside the session guard

Reviewed commit: b6e33332cd8beab675da31b04509fbbee70be8bb

## TLDR

A screenshot can be accepted without checking whether its cleanup changed the staged sessions that the next shot will
use.

## Details

`scripts/docs_shots.py:392–415` checks the fixture’s session IDs before invoking the cleanup callback and writes the
result after it. The callback contract is to restore the shared fleet. The current group-delete callback changes
membership without changing session IDs, so the observed gap is in the runner’s generic guarantee. Check the same IDs
after successful cleanup and preserve the earlier check.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `docs-capture/l6`.
