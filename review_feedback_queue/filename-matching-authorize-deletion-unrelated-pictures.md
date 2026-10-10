# Filename matching can authorize deletion of unrelated pictures

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Recording can delete unrelated pictures in a matching stills directory.

## Details

F62 — **definite** — `e2e/readme-video/recorder.ts:481`; `e2e/readme-video/recorder.ts:481-482` — Filename matching can
authorize deletion of unrelated pictures

The guard accepts an existing companion directory when its regular files have recording-shaped names. That proves
filename compatibility, not that the recorder owns the directory. An independently created collection with matching
names therefore passes and is recursively removed. Require explicit ownership evidence before replacing an existing
stills directory, refuse unrecognized directories, and provide a deliberate migration path for older recordings.

## Evidence and triage context

- scripts/readme-video.sh:27-44 accepts an operator-selected output path and makes it absolute without restricting its
  directory.
- e2e/readme-video/recorder.ts:181 and :397 call ensureStillsReplaceable; :443 accepts names such as 01-notes.png;
  :480-487 reject non-directory or nonmatching contents but accept unrelated regular files with matching names.
- e2e/readme-video/recorder.ts:409-412 derives the stills directory from the output and recursively deletes it after
  encoding.
- e2e/readme-video/recorder.ts:434-435 derives the stills directory from the chosen output.
- e2e/readme-video/recorder.ts:443 and :481-482 accept any regular file matching the numeric-prefix PNG pattern;
  01-diagram.png qualifies without recorder provenance.
- e2e/readme-video/recorder.ts:181 and :397 apply that predicate; :411 recursively removes the accepted directory.
- scripts/readme-video.sh:27-30 accepts a caller-selected output; e2e/readme-video/capture.spec.ts:67-82 passes it to
  Recorder.start.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:6659-6673, recorder-stills-owner.md, concerns the same output-directory collision and deletion
  consequence. Its completion criterion at :6667-6669 requires showing the recorder made the directory. It neither
  accepts filename collisions nor demonstrates that the replacement guard meets that requirement. This is residual
  substance beyond the recorded completed fix.
- TRIAGE_OUTCOMES.md:6659-6673, recorder-stills-owner.md, concerns the same broad destructive operation but requires
  proof that the recorder made the directory. It records completion, not acceptance of filename collisions.
  SPEC.md:2160-2164 concerns deliberately fabricated filesystem states; ordinary numbered PNG files need no deliberate
  imitation.

Caveats:

- Requires an output-path collision and directory contents satisfying the filename predicate.
- Maintainer tooling, not a shipped product operation; the earlier ledger classified the original tooling finding below
  highest.
- The highest suggestion conservatively reflects deletion of operator-owned pictures.
- Verified by inspection; no runtime reproduction.
- Maintainer tooling. No files were created or deleted during verification.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_05_cor:p1:F1`,
`test_infrastructure_05_sec:p2:F3`.

- `test_infrastructure_05_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `test_infrastructure_05_sec:p2:F3`: confidence as filed: definite; suggested bucket as filed: highest.
