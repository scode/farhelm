## What this was about

Quick launch could use the previous template while a newly chosen quick-launch mark was still saving. Closing and
reopening Settings during a post-clone command save could also allow an older save to overwrite a newer one. The
maintainer chose one ordering rule for configuration saves instead of separate fixes for those rare races.

Later reads and actions from the same app now wait for earlier configuration saves. Independent saves remain parallel,
and ordinary session selection and other view preferences do not add a save round trip to navigation. Both original
scenarios have browser coverage.

## Things you should know

The ordering applies within one browser tab or desktop app. Separate clients retain their existing last-writer-wins
behavior. Failed or cancelled saves release later requests, and waiting uses each request's existing timeout. A lost
reply still has an unknown outcome: later requests may proceed while the helm finishes that save.

Tracked saves are the quick-launch mark, number-key template bindings, template save/delete, checkout folder, post-clone
command and ntfy settings. Terminal connections, the event feed and JavaScript's file/clipboard requests remain
independent. No helm change, protocol bump or database migration is needed.

## Open questions and possible follow-ups

None required for this scope. The existing Maybe later TODO about reconsidering an ordered WebSocket channel remains.

## PRs

- [#2062: order app requests after configuration saves](https://github.com/scode/farhelm/pull/2062/changes).

## Checks run, reused and skipped

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, targeted `dprint check`, `git diff --check`,
  `python -B scripts/check-test-sleeps.py` with the isolated pinned interpreter, and
  `python3 releasing/check-changelog.py format`: passed. The sleep checker inspected 301 delays, with zero missing
  rationales.
- Recorded `cargo nextest run -p farhelm-ui --lib`: 492 passed, zero skipped; run
  `1dba8688-d4d8-496a-af4b-252e5a4240cb`.
- Recorded desktop API and persistence selection,
  `cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(api::) or test(desktop::)'`: 201 passed, 442
  excluded by selection; run `b0713030-bffb-461b-82a7-7b658ebd32c3`.
- `cargo build` and pinned Dioxus 0.7.10 `dx build --package farhelm-ui --platform web --release`: passed. Two earlier
  attempts invoked an unrelated program named `dx` and refused before building; the corrected build used the pinned
  Dioxus executable.
- Recorded focused Chromium/WebKit regression selection: six passed; run `f67031e6-8f1d-4bab-8e8b-cd3faa7c8172`. An
  earlier six-case pass used a weaker shortcut-start premise; the final run requires the shortcut's opening generation
  to advance while the save remains held.
- Broader Chromium/WebKit integration selection: 228 passed, run `99f27424-5fed-4f68-ad52-20a9f5d5e80f`. It selects
  quick launch, templates, Settings, authentication, feed and reconnect, excluding the six focused cases already
  covered.
- The reopened-Settings test was further strengthened to observe the field staying locked while the earlier save was
  held, beyond its initial loading state. Both final engine cases passed in run `06907ea4-e4dd-4963-b518-708f00c345ed`.
- Runtime results cover the final production code. Later edits only corrected module comments and strengthened the
  browser premise; no broader Rust rerun or web rebuild was needed for those edits.
- Inspected all incoming main changes before the final rebase: queue states and plan reports only. Rebasing onto
  `4df8d53cf38fbc79167e85c9ded38fbf5fd8d2c1` changed no product code, so the per-command evidence above remains valid.
- Full workspace Rust execution, the full browser suite, installer/provisioning/desktop-smoke gates and website build
  were skipped: this changes the GUI request funnel, and the selected unit, desktop and browser paths cover that risk.
  No asset JavaScript or docs website page changed.

## Review gate outcome

A fresh gpt-6.1-sol reviewer at high effort reviewed correctness, design, idiomatic code and the full test-authoring
checklist. It found one browser fixture-premise issue: opening Templates already made the shortcut generation nonzero.
The test now requires that generation to advance before measuring blocked dispatch. That corrected test passed on both
engines. No production-code findings remained. The commit/PR wording also passed a separate cold read.

### Landing

Landed as #2062, after pin-sessions (#2064) in the same round; both were tested stacked on top of main, which had gained
mid-session YOLO detection (#2063) and plan bookkeeping, none of which sends configuration saves.

#### Review

An independent fresh-context review found nothing blocking. Every slot is released on success, failure, timeout and
drop; a preference that is dirty but idle holds none, so the re-authentication seed read cannot deadlock against its own
replay; locks are always taken in one order and never held across an await; a waiting request reads its credential after
waiting; the wait is charged to the request's existing deadline. Every write the app sends to the helm was classified,
and the tracked ones match the plan; nothing waits on a newer save, and navigation, terminals and the event feed add no
round trip. The restoring-confirmations and template-binding acknowledgments keep their meaning.

#### Fixes made while landing

- No test exercised the timer that ends a read's wait behind a save that never settles, only an already-expired
  deadline; a new test holds a save forever and checks that a 30 ms wait gives up with the ordering timeout. The
  existing deadline test no longer fails if its thread stalls for a second.
- The quick-launch browser test showed only that no preference read happened within 150 ms of the shortcut, which cannot
  tell "held by ordering" from "not reached yet". It now records the order of events at the route and checks that the
  shortcut's read does happen, and only after the held save starts answering.
- The changelog fragment now opens with what the user sees, and SPEC_impl.md says every preference field other than the
  template bindings takes no slot, not only list-view preferences.

#### Left as is

- Nits: every request creates a timer even with nothing pending; a template delete's budget includes its preparation, so
  a slow preparation could report the ordering timeout; a request that times out while waiting leaves no line in the
  per-request log; the Settings test could assert the held read directly.

#### Checks

- See the pin-sessions landing for the shared runs: the helm package (run `90aa4784-5c92-44bc-a321-bd3ccedf81b1`, 1103
  passed), the UI crate after the fixes (run `e482a9d7-033c-4e99-9b34-ce99c43a0bee`, 648 passed), the first browser run
  (run `99fa749b-2ba2-41cb-b452-7efdf838832b`, including the auth, feed and terminal-reconnect specs this change could
  affect) and the follow-up browser run (run `969a625e-943d-45d9-8695-6d72c0327f0a`, including quick-launch and
  settings). The one session-confirmations timeout is described there; it passed in three isolated runs on both engines.
- Run now for this plan's fixes: the save-order and pin unit tests, run `3626eb14-befb-4737-9927-6b491c8cee39`: 9
  passed.
