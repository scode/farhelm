# Screenshot cleanup can hide the capture error

Reviewed commit: b6e33332cd8beab675da31b04509fbbee70be8bb

## TLDR

If a capture and its cleanup both fail, the screenshot report can show only the cleanup error and carry the unrestored
fixture into the next shot.

## Details

The handlers at `scripts/docs-shots/release-notes.py:70–81` and `scripts/docs_shots.py:396–419` invoke cleanup while a
previous exception is active. The final report keeps only the later error text and the capture loop can carry on after
failed group deletion. Report both failures and confirm restoration before another shot reuses the fixture. The
exception order and continuation are source-confirmed. The double failure remains a conditional source case.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `docs-capture/l2`.
