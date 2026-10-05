# Starting video recording can erase an unrelated sibling directory

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Starting video recording can erase an unrelated sibling directory.

## Rebase context

At rebase onto `eb71c32e0f14f4569938e7da293e366aa07eae8a`, the staging issue previously recorded as F38 is fixed by
#1582 (`799ee85db4101f3548c9fb463e3c9bdd92c67400`): `e2e/readme-hero/stage.ts:270–272` supplies an explicit boolean. The
earlier staging-failure caveat was removed. The recursive deletion of the output's `.frames` sibling remains unchanged,
as does the uncertainty about ownership of existing contents.

## Details

`F45 / COR-RECORDER-FRAMES-OWNER` — **possible** — `e2e/readme-video/recorder.ts:156` — Starting video recording can
erase an unrelated sibling directory

The video command accepts an arbitrary MP4 output path. Before recording, it derives a scratch directory by appending
`.frames` to that path and recursively deletes whatever is already there. Choosing `demo.mp4` therefore also authorizes
deletion of `demo.mp4.frames/` in practice, even though the code has not established that it created or owns that
directory.

If that sibling directory already contains unrelated files, starting a recording erases them. The default
generated-output layout does not present the reported collision, and no populated collision was observed. Create scratch
storage with `mkdtemp` and clean only the private directory allocated for this run. Proposed bucket: highest. No
possible cover was identified.

Restater note: The input records an independent reviewer disagreement under D47, without supplying its reasoning. The
recursive deletion is present; whether caller-selected output implicitly grants ownership of this sibling directory
remains a review decision.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `auto_systems p1`, `auto_edges p1`, `auto_trust p2`, `auto_secrets p2`.

Possible cover recorded during collection: none identified.

Collection caveats: Conditional preexisting populatedcollision; defaultgenerated layout unaffected. IndependentD47
disagree-record.

Coordinator confirmation: independent audit D47 found no exact permission to erase foreign companion contents. Keep
possible ownership uncertainty for triage.

## Filed reviewer metadata

- `auto_systems p1`: confidence as filed: **definite / confirmed** conditional on a populated colliding directory; the
  destructive behavior is direct source evidence. Suggested bucket as filed: **highest** (loss of unrelated files/work).
- `auto_edges p1`: confidence as filed: definite for deletion when an accepted output path collides with an existing
  directory. Severity: recursive loss of user files. Suggested bucket: highest. Suggested bucket as filed: highest.
- `auto_trust p2`: confidence as filed: definite for the named collision trigger; no runtime reproduction. Suggested
  bucket as filed: highest, loss of user-owned files.
- `auto_secrets p2`: confidence as filed: definite / confirmed for the existing-directory collision trigger. Suggested
  bucket as filed: highest.
