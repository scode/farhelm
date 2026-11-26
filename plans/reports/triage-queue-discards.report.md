### What this was about

The maintainer approved closing seven pending review findings as discards in one draft. The decision accepts the stated
tradeoffs and preserves the reasoning for later review.

- **Release-note screenshot.** A parked pointer could show a tooltip during recapture. Published shots stay frozen, and
  any future recapture gets human review.
- **Concurrent host provisioning.** Two helms setting up the same host could interrupt one temporary upload. The failure
  is visible and retryable, with the established binary and data intact.
- **Release payload cache.** The cache does not add its own directory permission check. It lives under the helm's
  already private state directory in production.
- **Duplicate Podman host entries.** An unused Podman place can move to its twin during identity verification. The
  possible effect is a routing preference change between entries for the same local account, with no containers, images,
  or helpers owned by the place.
- **Conversation hook hint.** A stale dismissal error can reappear in the session window. Triage already closed this
  same finding under the small-polish threshold. [#4863](https://github.com/scode/farhelm/pull/4863/changes)
- **Trust hint on reconnect.** A dismissed hint stays hidden across a brief disconnect. Preserving that dismissal is
  intentional.
- **Old Claude cleanup records.** A legacy relative lock path can leave folder trust in place after archiving and cause
  cleanup to retry on supervisor startup. It can also let the edit proceed without the intended lock. Triage accepted
  this rare case while planned work retires post-clone Claude trust edits.

### Things to know

The draft removes the seven feedback files and their index entries, moves the decisions out of the pending ledger, and
retains the reasoning in the archive. It is verified and awaiting landing.

### Open questions and possible follow-ups

None.

### Pull request

- Archive the seven settled discards. [#5042](https://github.com/scode/farhelm/pull/5042/changes)

### Checks

`dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`, `git diff --check`, and
`python3 scripts/check-agents-md.py` passed. The instruction checker reported one warning for an archive citation to a
missing “Bound polling time” heading in the test authoring instructions. That citation is present in the base archive
and the target instructions match the base. The 54 remaining feedback files match the index one-for-one. The seven
appended entries are formatted, and the earlier archive matched byte for byte. Skipped runtime suites because this
change only moves Markdown records.

### Independent review

The source review confirmed all seven authorized moves, the preserved archive, and the complete index match. The draft
was then verified at `e5013c24` with all seven archive links pointing to it.
