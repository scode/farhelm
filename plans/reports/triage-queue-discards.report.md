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

### Landing

#### What landed with it

This plan landed last of three in its round, after the provisioning, Podman and runtime triage and the spec and comment
updates. It archives seven findings triage decided to discard and touches no code or spec. Its rebase onto the other two
plans' triage records needed one hand resolution: the open outcomes file was rebuilt as the earlier plans' version minus
exactly this plan's seven sections, and the queue's files, index and open outcomes were checked to agree. The landing's
independent review confirmed it removes exactly the seven recorded discards and nothing else.

#### Fixes made while landing

- None.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the round's three plans stacked in queue order on main as it stood at the start of the landing (`491df5c5`), with the
shared triage records' conflicts kept both ways (by hand for one review queue index block in the spec plan's PR and the
open outcomes file in the discards plan's PR, then checked: the queue's 21 feedback files, its 21 index entries and the
21 open outcomes agree). Every other change is exactly what the executors pushed until the landing fixes, which change
only a SPEC.md sentence and two changelog drafts.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `0368f541-87a9-4191-8302-5b438567d5bf`): all 8765 passed, with no runtime `SKIPPED` lines. That includes the
  provisioning, Podman, upload, UI channel and runtime tests the plans added or changed, on the rebased tree.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `19013678-79f1-4d2e-9e1b-0271274e62f8`): of its 49 steps, 48 passed and one (paste) was skipped by design on Linux.
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format` (again after the fixes),
  `cargo fmt --all -- --check` and `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally:
  it fails only on over-long lines in the triage outcomes archive: main's earlier ones and the new entries of the
  provisioning, Podman and runtime plan (the other two plans' entries pass); no PR fixes them yet; a formatting-only one
  would, and I recommend it, as earlier rounds' notes did.

Reused across the fixes: every run above, since they change no code. The executors' runs were on older bases, and
everything they covered ran again above.

Skipped: the screenshot comparison and the desktop app's outer shell tests, since no drawing code or desktop shell code
changed; real Podman and Tensorlake hosts and a Mac run, which no automated check here reaches; and CI, which runs only
on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- Nothing beyond the round's shared checks.
