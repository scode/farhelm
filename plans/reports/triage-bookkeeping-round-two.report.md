## What this was about

Eleven review findings had already received the maintainer's decisions but still remained in the feedback queue. Nine
were discarded, meaning they were not worth further work at this time. Two were deferred into Maybe later: keeping a
session available for another Delete after failed create cleanup, and correcting Mac window restoration across displays
with different scaling. This plan records those decisions and removes the eleven queue items. Nothing changes for
someone running Farhelm.

## Things you should know

The discard decisions remain in the triage ledger with their original assessments. They cover an upload waiting after
its session is deleted, an old Update staying busy until reload after a different Update completes, briefly disconnected
terminals after probing an already healthy host, extra whole-host reads at Farhelm's intended scale, tab cleanup blocked
by a broken host, an approval feed recovering after browser sign-in, and a terminal-font test whose fallback is already
slated for removal. A possible output-client cleanup delay was also discarded by the maintainer; its uncertain trigger
was not silently converted into specification acceptance. The aggregate host-identity finding was explicitly discarded
as refuted after the maintainer saw its cross-host implications. Execution does not re-triage any of these findings.

The failed-create follow-up is now a third case in the existing systemd-user-manager TODO entry. If local storage fails
while confirming a create and the process sweep also fails, a successful terminal teardown can still remove the session
record while a detached process survives. The wanted remedy is to keep the session when cleanup cannot be confirmed, so
a later Delete can retry. This storage-failure trigger is described separately from the entry's first two cases, which
come from mixing processes with and without systemd scopes. The entry's finishing instruction now names all three cases.
No cleanup behavior was changed by this plan.

The new Mac window-restoration TODO sits beside the existing desktop entries. It describes reopening at the wrong size
or position when displays use different scaling, and saving that wrong geometry again on close. It requires work and
verification on a real Mac with mixed-scale displays, consistent with the implementation specification's outstanding
native-display check. This plan performs no Mac experiment or proposed fix.

All eleven outcomes still had their planned state and original decisions when execution began. No item was skipped,
already removed or changed in meaning. Only the eleven Execution fields, their feedback files and index lines, and the
two authorized TODO edits changed. No Deflake entry was removed.

## Open questions and possible follow-ups

The two TODO entries preserve the requested future work. Their implementation remains deferred; this plan completes
queue bookkeeping, not those fixes. No additional decision is required for this delivery.

## PRs

- [#1931 — record discarded and deferred review findings](https://github.com/scode/farhelm/pull/1931/changes), draft;
  pushed head `d03df1541825819ad2110349c17630e075c91fdc`.

## Checks run, reused and skipped

- Run: targeted `dprint check TRIAGE_OUTCOMES.md TODO.md review_feedback_queue/INDEX.md` and `git diff --check`; passed.
- Run: exact directory/index comparison; all 45 remaining feedback files appear exactly once and every indexed file
  exists. The private bookkeeping helper passed Deno format/type checks and its wrapped-bullet-removal check. The
  separate documentation pass confirmed that the two original systemd cases keep their context and that the related
  storage-failure case and Mac requirement remain distinct.
- Reused: the source review still covers the final source after the clean rebase onto `71ba2dd2`; the only intervening
  main change records the browser plan's landing claim and has no source interaction. The later edit inserts the PR URL
  into the eleven Execution fields only; its interdiff and final formatting checks passed. No earlier runtime result is
  needed for this Markdown-only change.
- Skipped: Rust, JavaScript, browser, desktop, installer and provisioning runtime checks. These ordinary Markdown
  changes affect no generated artifact or executable example. No Deflake entry changed, so `deflake/known-flakes.txt`
  needs no edit. No changelog fragment is required for a bookkeeping-only `chore` PR.

Private state is recorded in `farhelm-plan-triage-bookkeeping-round-two-log.md` beside the checkouts.

## Review gate outcome

The required fresh-context gpt-6.1-sol high source review reported no findings. The executor read its full artifact and
confirmed the eleven-outcome scope, both TODO contracts and the directory/index parity. The final PR URL insertion was
verified separately as a mechanical edit. A separate fresh gpt-6.1-sol medium reader accepted the revised title and
empty body after the first reader's clarity feedback. Implementation and validation stayed with the executor; no
workhorse was delegated.

Private galaxy-brain evidence reuses session `29405ad6-d9d8-4919-8d82-c1a7afa9b9f2`, referenced in the working log.
Native tools expose no independently reported model attribution, effort or token usage; those remain evidence gaps. The
local implementation decision was recorded after editing and describes its recording time, not an invented start time.
These accounting limitations supply no runtime or cost claim.

### Landing

Landed on 2026-10-12 (UTC) as #1931.

#### Review before merging

No findings. The PR removes exactly the eleven findings its plan names, nine discarded and two deferred, marks each
ledger entry complete, and changes nothing else in the ledger. Its two TODO changes follow the recorded decisions: the
failed-create cleanup case is folded into the existing Maybe-later entry about requiring a systemd user manager, and a
new Maybe-later entry covers Mac window restoration across display scales, saying it needs work on a real Mac. Note left
as is: the new systemd case leans on internal terms more than the plan's "in product terms" asked.

#### Checks

These two plans were landed together, triage-bookkeeping-round-two first with dev-tooling-robustness-fixes on top, after
ui-layout-and-escaping-fixes; the rebases met only the review queue index, which still matches its files.

- Run now, on the two stacked: `cargo fmt --all -- --check`, `dprint check` (after fixing a double blank line the
  landing's own index merge left), the test-delay checker, `shellcheck` on `website/scripts/preview.sh` and
  `deflake/eval-setup.sh`, the test recorder's self-tests (60), the deflake driver's tests (18), the brick-test
  tooling's unit tests (26 and 6), `cargo clippy --all-targets -- -D warnings`, and the supervisor's tmux module tests
  through the recorder (run `31f6afcc`, 52 of 52).
- Not run: the signing workflow, which runs only on a release; the deflake end-to-end evaluation, which the executor ran
  and passed; and any browser or wider Rust tests, since nothing they cover changed.

Nothing in the report above was made untrue by the landing.
