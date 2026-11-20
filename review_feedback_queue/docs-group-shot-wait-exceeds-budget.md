# The group screenshot can exceed its membership wait

Reviewed commit: b6e33332cd8beab675da31b04509fbbee70be8bb

## TLDR

If the staged helm stalls, the group screenshot’s ten-second membership wait can spend about a minute on one request.

## Details

The synchronous probe at `scripts/docs-shots/release-notes.py:64–77` uses a helper with its own 60-second socket timeout
at `scripts/gui-agent.py:2244–2258`. The outer poll at `:1481–1491` can inspect elapsed time only after the probe
returns. Pass the remaining budget into the request or state the separate request limit. The boundary follows from
source. Actual capture duration remains unmeasured.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `docs-capture/l9`.
