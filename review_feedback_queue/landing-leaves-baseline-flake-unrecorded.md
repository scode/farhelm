# Landing reports leave baseline flakes unrecorded

Reviewed commit: 917c63b388eb90b7f5b8d5ead4651135fdba47d9

## TLDR

Landing reports describe recurring failures on unchanged main but defer writing the shared flake records and tracking
entries. Later maintainers must rediscover those observations. The reports' private runtime evidence has not been
independently authenticated here.

## Details

Suggested bucket: **other**. Confidence: **confirmed recording omissions based on committed observations**. The asserted
recurrences were not reproduced in this review, and their causes remain unknown.

`plans/reports/structured-settings-dialog.report.md:144–148,186–189` at the reviewed commit says the sidebar
build-readout test failed on main in two of five attempts, cites retained run `db0c82af-6a1e-4359-8724-7fa17c65dbba`,
and leaves recording it open. Root instructions require a newly established latent flake to be recorded in `FLAKES.md`,
with its Deflake TODO and `deflake/known-flakes.txt` entry. Recording an observed recurrence does not require diagnosing
or fixing it.

The reviewed TODO and exclusion file contain no matching entry. The pinned `FLAKES.md` entries at `546–554` and
`1795–1817` describe resolved earlier failures; `2413–2430` concerns another test. They do not record this recurrence.
Verify the retained evidence before writing a cause; preserve uncertainty if it is unavailable.

### Additional held-save recurrence

Additional reviewed commits: `421fafabc074f55a27dcb0272414eb63ed938f3e` and `89c5b9b7ba7428590f3e5b95150d4fc6ea52ea8a`.

Their landing sections, `plans/reports/launch-attempt-owner.report.md:170–179` and
`plans/reports/helm-launch-admission.report.md:136–145`, describe workspace run `68a7a44f-3142-4013-aaef-7d0dc5350e0c`,
three preceding failures of `api::save_order::tests::a_hung_save_releases_waiters_at_their_deadline`, and 50 isolated
passes. Both defer the FLAKES/TODO/known-flakes writes to another PR. This is one shared omission across two reports,
not two independently observed new flakes. At the first revision, `crates/farhelm-ui/src/api/save_order.rs:199–212`
confirms the named test's held-save assertion. Pinned tip `2ea2826caa0eec867c4e2bddc01badbac7bbe6c7` has no
corresponding entry.

The committed reports establish the recording discrepancy, not the authenticity or diagnosis of the private runs. A
promised draft fix may exist outside the pinned history; no external PR state was checked. No product bug or assertion
about later revisions follows from this observation.

Possible cover: none found in the required coverage sources. Older resolved history and another test's cleanup decision
do not cover either omission. No filter match was established. Proposed drop: none.

Provenance: original trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g021/f2`; held-save extension from run
`30f46c42-a021-475f-96a8-eac06930471b`, `g006/f1`.
