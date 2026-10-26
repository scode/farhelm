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

### Additional initial-prompt recurrence

Additional reviewed commits: `f49e4c0c94ff85b58ca3c1dbe7ca30ea21b0fa6c` and `9394188108a1f5e0249fb4f2dd81253d76690be6`.
`plans/reports/gui-saves-last-write-wins.report.md:68–73` describes failures of “initial prompt survives a recent but
clears on Clone and Replace prefills” on both plan code and unchanged main, while deferring the required FLAKES, Deflake
TODO and exclusion entries. Searches of the tracking files at pinned tip `616d79f45d6a1d817833291429d144a5c686bb4b`
found no matching entry. This is another test under the same recording omission, not a duplicate count of the held-save
recurrence repeated later in that report.

Confidence and bucket remain **confirmed committed omission, other**. Private runs and diagnosis were not authenticated;
verify them before recording a cause. No product bug or flake was reproduced. No exact coverage or filter match was
found. Provenance: run `4fae501c-3686-4822-aa6c-037266637632`, `g010/f1`. Proposed drop: none.

### Further committed recurrence reports

At `a03d6baa7bf2a320ab738c51dffa7247a944a594`, `plans/reports/sign-in-preservation-sweep.report.md:53–56` reports
intermittent Chromium failure of “templates busy root stays keyboard-scrollable,” including one of four attempts on
unchanged main, and defers recording. The landing addition at `aa2078cbb622a543147eec79abb3d00003a8661c:187–190` reports
a recurring wrong-host Update acceptance failure during ten repeats and likewise defers shared records.

At `45e0bedbc29f0be890eb0a3480accac5d978458c`, `plans/reports/session-view-fixes-oct22.report.md:204–210` reports
WebKit's “remove-and-re-add-host: removal forgets, re-adding rediscovers” failing seven of ten stack attempts and five
of ten unchanged-main attempts. It defers FLAKES/TODO tracking to a separate draft. Later approval removes the report
but supplies no corresponding shared record or exception.

Chromium's “accepted ADD registers before execution and releases the page lock while running” is one recurrence repeated
by three reports: `52ccb64c5a497adb359e9ce1e3a4210e96933450`,
`plans/reports/update-refuses-newer-host-schema.report.md:143–153`; `9b9b554eacabf4570aa4437eefba198849e74af3`,
`plans/reports/dialog-saves-oct22.report.md:152–162`; and `ad01c7122d24952c7d589a02d36b9eab08c7fb9e`,
`plans/reports/build-and-tooling-oct22.report.md:150–160`. All cite two of twenty failures in the same main run and
defer recording. Count it once.

There is a narrower historical sidebar Replace omission. The report introduced at
`7e7255845c94d0c2a07b3c2c051bd74779063901`, `plans/reports/session-details-bottom.report.md:157–161,184–188`, reports
WebKit's “plain Replace keeps a newer same-source key after old sidebar YOLO cancellation” failing three of six plan
attempts and two of six main attempts. Existing header pointer-race records partly cover the shared menu transition, but
omit this exact sidebar title and its disposition. The machine exclusion matches a complete test title, so its header
entry does not exclude sidebar. Later commit `d6abf445783e49711b5ad71c1a84e11fbed29d32` adds an explicit menu-close step
to both variants. This is **not** a claim that the old fixture still fails or a recommendation to add a new current
exclusion without verifying its disposition.

Pinned-tip tracking searches found no matching entries for these additional observations. Confidence remains **confirmed
committed reporting/tracking discrepancy, other**. Private runs, causes and possible external draft fixes were not
authenticated. No test or bug was reproduced. Preserve the historical observations and later source changes when
deciding what to record. Provenance: run `ef6e18fa-8ac0-4850-a557-d59b8af5efb9`, `g138/f1`, `g141/f2`, `g144/f1`,
`g145/f1`, `g146/f1`.
