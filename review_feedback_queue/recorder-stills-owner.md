# Finishing a video can erase unrelated files in its stills directory

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Finishing a video can erase unrelated files in its stills directory.

## Details

`F46 / COR-RECORDER-STILLS-OWNER` — **possible** — `e2e/readme-video/recorder.ts:339` — Finishing a video can erase
unrelated files in its stills directory

Finishing a video creates review stills at named moments in the recording. For an output called `demo.mp4`, it derives
`demo-stills/`, recursively deletes that entire directory, and then extracts the new PNGs. It does this after encoding
the video, without checking whether existing contents came from an earlier recording or belong to the recorder at all.

A colliding directory can therefore lose unrelated files or manually prepared stills. The video requirements describe a
companion stills directory, but do not explicitly authorize deleting foreign contents; the ownership contract is
ambiguous, and no collision was observed. Refuse an existing unowned directory, or track generated artifacts in a
manifest and replace only those artifacts. Durable stills need an explicit replacement policy separate from private
scratch cleanup. Proposed bucket: highest. Possible cover: `docs/readme-video/SPEC.md:39–46` describes the stills
output, but does not settle removal of foreign contents.

Restater note: The input records an independent D47 assessment of ambiguous ownership. This finding depends on whether
choosing the MP4 output also grants ownership of every file in its derived stills directory.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `auto_systems p1`, `auto_edges p1`, `auto_trust p2`, `auto_secrets p2`.

Possible cover recorded during collection: docs/readme-video/SPEC39–46 names stills but no foreigncontent removal
acceptance..

Collection caveats: Output companionownership ambiguous, chosenoutput collision unobserved; independentD47
ambiguous-record.

Coordinator confirmation: independent audit D47 found no exact permission to erase foreign companion contents. Keep
possible ownership uncertainty for triage.

## Filed reviewer metadata

- `auto_systems p1`: confidence as filed: **definite / confirmed** conditional on a populated colliding directory.
  Suggested bucket as filed: **highest** (loss of unrelated files/work).
- `auto_edges p1`: confidence as filed: definite for deletion after encoding when the chosen output collides with a
  pre-existing stills tree. Severity: recursive loss of user files. Suggested bucket: highest. Suggested bucket as
  filed: highest.
- `auto_trust p2`: confidence as filed: definite for the named collision trigger; no runtime reproduction. Suggested
  bucket as filed: highest, loss of user-owned files.
- `auto_secrets p2`: confidence as filed: definite / confirmed for the existing-directory collision trigger. Suggested
  bucket as filed: highest.
