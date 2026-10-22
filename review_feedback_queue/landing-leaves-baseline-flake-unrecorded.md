# Landing report leaves a baseline flake unrecorded

Reviewed commit: 917c63b388eb90b7f5b8d5ead4651135fdba47d9

## TLDR

A landing report describes a reproduced browser-test flake on unchanged main but leaves it out of the flake records and
tracking files. Later maintainers must rediscover that observation.

## Details

Suggested bucket: **other**. Confidence: **confirmed recording omission based on committed observations**. The private
runtime evidence was not independently authenticated or reproduced, and its cause is unknown.

`plans/reports/structured-settings-dialog.report.md:144–148,186–189` says the sidebar build-readout test failed on main
in two of five attempts, cites retained run `db0c82af-6a1e-4359-8724-7fa17c65dbba`, and leaves recording it open. The
root agent instructions require a newly established latent flake to be recorded in `FLAKES.md`, with its Deflake TODO
and `deflake/known-flakes.txt` entry. Recording the observed recurrence does not require diagnosing or fixing it.

The reviewed TODO and exclusion file contain no matching entry. The pinned `FLAKES.md` entries at `546–554` and
`1795–1817` describe resolved earlier failures; `2413–2430` concerns a different test. They do not record this
recurrence. Verify the report's retained evidence before writing a cause; preserve uncertainty if it is unavailable.

Possible cover: none found in the required coverage sources. Older resolved history and another test's cleanup decision
do not cover this omission. No filter match was established. Proposed drop: none.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g021/f2`.
