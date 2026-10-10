# The final stills check becomes stale during encoding

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Files added during video encoding could be erased afterward.

## Details

F68 — **possible** — `e2e/readme-video/recorder.ts:397-411` — The final stills check becomes stale during encoding

The recorder checks the companion stills directory before waiting for an external encoder, then recursively replaces it
afterward without checking again. An ordinary file copied into the directory during encoding can therefore be deleted
despite the guard that would have protected it earlier. Concurrent modification was not reproduced. Revalidate
immediately before replacement, refuse changed foreign contents, and preserve them when publication fails.

## Evidence and triage context

- e2e/readme-video/recorder.ts:397 validates the directory before :398-407 awaits ffmpeg using the slow preset.
- e2e/readme-video/recorder.ts:411 removes the directory without revalidation.
- e2e/readme-video/recorder.ts:481-487 would refuse a foreign filename if it existed when validation ran.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:6659-6673 does not settle concurrent additions during encoding.
  review_feedback_queue/publisher-copy-race.md concerns copying screenshot pixels into published evidence, not deleting
  newly added stills-directory contents. SPEC.md:2160-2164 does not exclude ordinary concurrent saves.

Caveats:

- Concurrent modification was not reproduced. Shares the deletion sink with F3 but has a separately editable
  validation-timing cause.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_05_sec:p2:F4`.

- `test_infrastructure_05_sec:p2:F4`: confidence as filed: possible — concurrent addition during encoding is the open
  premise; suggested bucket as filed: highest.
