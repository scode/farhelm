# Desktop smoke detaches its supervisor before registering cleanup

Reviewed commit: 05ce9d59f38bfcaf5cdc40162c26695c37d7e44b

## TLDR

Interrupting desktop-test startup between detaching its supervisor and saving the ownership record can leave that test's
supervisor outside cleanup. The timing and survivor duration were not measured.

## Details

`crates/farhelm-ui/src/desktop.rs:1230–1249` detaches the supervisor before publishing its ownership record. The smoke
test signals the app at `scripts/desktop-smoke.sh:343–350`, then relies on record-based supervisor cleanup; that helper
returns when the record is absent at `290–291`. There is no parent-lifetime tether to stop the unrecorded process.

The same boundary remains at recording-time main `b7227751f968e78005d8bccfffcf4323b21d24c5`: `desktop.rs:1267–1284`, and
smoke helper `514–515`, app teardown `567–569`, record cleanup `574–580`. The subsequent terminal-host cleanup does not
stop that unrecorded supervisor. Arrange ownership before the fallible startup interval or provide cleanup independent
of the unpublished record, while preserving the product's intended detached lifetime.

Suggested bucket: `other`. Confidence: confirmed conditional fixture-ownership gap. No runtime reproduction, leak
duration, incidence, catastrophic tooling harm or loss of unrelated work was established. Proposed drop: none.

Possible cover: `TRIAGE_OUTCOMES.md`, "replacement-test-supervisor-spawns-before-pid-publication.md", records a pending
browser-helper discard with a trivial-fix exception. Its assessment names a different test and its execution removes
that whole older feedback file. Broader intent behind "fix flakes when observed" is ambiguous. This desktop scope is
kept separately so executing that browser disposition cannot silently remove it. No exact acceptance, planned repair,
known bug or filter was found.

Provenance: g023/f3; Codex gpt-6.1-sol, high reasoning. Discovery pass 1 found this issue; pass 2 checked its
persistence, ledger scope and other boundaries without finding another issue. The earlier retained-terminal smoke
oracle, g023/f2, was separately repaired by `58094288259b0c29f74ea25936d4fe006c72128f` and is not included here.
