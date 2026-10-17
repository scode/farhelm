# WebKit input-method guards

## What this was about

Pressing Enter or Escape to finish input-method composition could also save a template, rename a session or close a
dialog in the Mac app and Safari. WebKit can announce that composition ended before delivering that key, so Farhelm's
existing composing checks saw an ordinary key. The requested scope was every existing guard outside the terminal,
including page shortcuts, with browser replay coverage and real-Mac verification left to the maintainer.

A shared listener now marks an immediately following key on the same field as composing after earlier typing keys have
been released. This is a partial workaround under the plan's agreed fallback, not full protection of every ending key.
The inventory was 29 Rust checks and five JavaScript checks outside the terminal; they keep their existing actions and
consume the shared event flag. Terminal input remains with xterm.

## Things you should know

The compatibility hint lasts 16ms and is consumed once. Chromium delivers its ending key before composition ends. An
earlier WebKit typing or modifier key can also remain held at that boundary; the tracker cannot infer which key caused
the end. It conservatively declines a hint in both cases, keeping Chromium's separate ordinary key usable before the
ending key is released. That leaves WebKit endings after an earlier held typing or modifier key unprotected, including
switch-session and quick-launch shortcuts when their modifier keys remain held. An unrelated key release cannot retire
the recorded composing key. A bare key code 229, as Pinyin can use for ordinary Enter, is insufficient on its own and
does not extend the hint indefinitely.

This is a bounded workaround. Synthetic browser replays measured 0–2ms between the replayed end and key dispatch; that
is not a measurement of a real Mac input method. A delayed ending key outside 16ms can still act as an ordinary key. The
original timing proposal was tested on the actual asset: at equal elapsed time it protected WebKit's ending key and also
swallowed Chromium's separate overlapping key. The 229 branch swallowed that real Enter regardless of the window. This
established the plan's conditional fallback; the implementation favors real keys and explicitly keeps the held-key and
delayed-key omissions. Browser tests also prove event marking reaches a real Farhelm dialog guard on both engines;
source inspection confirms desktop serialization reads that same flag. Neither proves native Mac timing or desktop-shell
behavior.

For the manual Mac check:

1. In the Mac app, enable a Japanese input method. Open New, choose an agent, then Save as template. Type a name while
   the input method is still composing, and release the earlier typing keys. End it with Enter: the template must not
   save and the name panel must stay open. A separate Enter should save it. Repeat with Escape: finishing composition
   must leave the draft open; another Escape should close the name panel.
2. Open a session's Rename dialog and repeat both endings. The composition-ending Enter must not rename; the
   composition-ending Escape must not dismiss. Separate Enter and Escape must retain their ordinary actions.
3. In New's model field, compose a custom model and end with Enter. The suggestions must remain open until a separate
   Enter accepts the choice. With Pinyin enabled but no composition underway, Enter in a template name must still save.
4. Repeat these checks in Safari if available. Also try ending while an earlier typing or modifier key remains held:
   that is a known unprotected case, rather than a claimed pass criterion. These are manual follow-through checks; the
   Linux run did not type with a native input method.

The plan described a text field using the shared choice-button Enter helper. All eight actual callers are buttons. The
new browser case covers the model text field's own composing guard; existing composing choice-button coverage is
retained. No composition-bearing button fixture was invented to claim input-method coverage.

## Open questions and possible follow-ups

Real Mac event timing remains unverified. Held earlier typing/modifier keys and delayed endings remain known gaps.
Further coverage needs an event invariant that distinguishes the ending key from a separate real key; widening a timer
does not establish that invariant. Native event captures would help a later attempt. No further maintainer decision is
required by the plan's agreed fallback for this delivered scope.

## PRs

- [#2194: guard immediate WebKit composition-ending keys](https://github.com/scode/farhelm/pull/2194/changes).

## Checks run, reused and skipped

On Linux x86_64, browser runs used Playwright 1.64.0, Chromium and WebKit, one worker, zero retries, and the pinned tmux
3.7c with the recorder's identity check.

- Run now: `cargo fmt --all -- --check`,
  `dprint check SPEC_impl.md TODO.md releasing/changelog.d/webkit-input-method-guards.md`,
  `python3 releasing/check-changelog.py format`, and the isolated interpreter's `python -B scripts/check-test-sleeps.py`
  passed; 302 test delays had reasons.
- The final Dioxus web release build passed at the source-reviewed revision `6edab83d`. Earlier `cargo build` for the
  helm/fixture backend, `cargo clippy --all-targets -- -D warnings`, and `cargo check -p farhelm-ui --features desktop`
  passed in this plan after the new Rust hook was added, but their exact covered commit identities were not retained.
  Those are observed earlier passes with incomplete revision identity, not new runs at the delivered revision. The
  backend and Rust hook/wiring are unchanged; the subsequent script release-identity correction is covered by the final
  web build, JS and browser runs, not by a new desktop runtime claim. Rebased onto main `1ccd6fe1`, which added named
  keyboard scroll targets and focus outlines to host plans and trash, with their tests and bookkeeping. All landed diffs
  were read; they change neither composition handlers nor the listener's event order. The regions contain text and
  scrolling controls, not composition editors. The plan's patch was unchanged across the clean rebase, so the final
  JS/browser evidence at `6edab83d` still covers the delivered `03ef2aba` behavior. No extra runtime rerun was warranted
  by those interactions.
- The recorder-backed UI JavaScript harness (`node --test` from its directory) passed 269 tests, no skips, run
  `df3c7cad-8d6b-458d-ab9f-d4bb6c9efeab`, after the fallback correction and release-identity regression. Earlier
  268-test pass `12a9d054-d778-4189-85bf-f0d01a7f549e` and 267-test pass `4210a0f3-7657-468b-b3d1-d6f27819ec6f` are
  superseded for this command.
- Transport proof passed 2/2, one per engine, run `290ff0ad-5eae-4b0c-89b2-33c6f888447c`.
- Browser selection
  `ime-composition\.spec\.ts|templates\.spec\.ts|restart-with\.spec\.ts|terminal-replay-rename\.spec\.ts`, grep
  `composition|composing|rename survives a real committed row reorder`, run `a8c32a8f-2fd0-4526-9b3d-f430aec955d8`,
  finished 26 passed and 6 failed. The new tests used a nonexistent template lookup, expected a deliberately hidden
  trigger to be visible, and gave WebKit's initial rename row 5s to appear. These failed attempts are retained
  privately; they are not a passing aggregate or a latent-flake entry.
- After correcting those test contracts, the narrow IME selection `Enter 229 without|stays in rename|precedes window`
  passed 8/8, four per engine, no skips, run `48caa36b-f681-49d5-b91a-83a48b92895d`. This predates the release-identity
  correction; the final browser run supersedes it for completion evidence.
- The focused outlined-policy experiment, grep `overlap boundary`, intentionally finished 4 passed and 4 failed in
  `a451ee2c-c813-4604-9061-79312c48aa43`: in both browser engines, WebKit-order endings were protected and
  Chromium-order separate keys were incorrectly marked composing. Both real-code Escape and 229 Enter conflicted. That
  failed candidate and its private patch remain evidence; it was restored before the final build and is not shipped.
- The final representative composition selection, with grep
  `composition|composing|rename survives a real committed row reorder|overlap boundary`, passed 40/40, 20 per engine, no
  skips, failures or interruptions, in `bb4f491b-c8ff-4f02-801b-3947509b0df3`. Its report is complete and retained
  output untruncated, with no runtime `SKIPPED` marker. This supersedes the earlier ordinary regression selections and
  includes the explicit conservative overlap omission.
- `scripts/check-desktop-assets.sh` passed: 26 requested assets matched the bundled set. Its exact covered commit
  identity was not retained; it ran before the release-identity correction and rebase, not at the delivered revision.
  That correction changes the inline script's decision and documentation, and the rebase changes existing CSS content;
  neither changes asset inventory or URL registration. Reuse is limited to that unchanged inventory relationship. The
  new file is included as source text by Rust, not a fetched asset URL.
- Skipped native Mac input-method execution: this executor runs on Linux; the manual steps above remain. Skipped the
  full workspace and browser batteries: the change is event normalization and existing action guards, covered by the JS
  traces, representative browser cases and compilation. No installer, service or live-install check was needed.
- Dprint cannot format TypeScript under this repository's plugin configuration; the attempted TS invocation exited 14.
  Supported changed Markdown files passed. No instruction file or Markdown heading was renamed, so the instruction link
  checker was unnecessary.

## Review gate's outcome

The required fresh-context gpt-6.1-sol high source reviewer found two initial gaps: the held-key inference missed WebKit
rollover, and the conditional fallback had not been established. A required scope reassessment retained the central
normalizer and called for the focused candidate-policy experiment. The same source reviewer then reviewed the corrected
full diff and that retained evidence, found no actionable findings, and accepted the documented partial workaround under
the plan's pre-agreed fallback. That is not approval of an unconditional guarantee or native Mac proof. The executor
checked those findings against source and retained results and owns the runtime evidence above; reviewers ran no tests.
Fresh gpt-6.1-sol medium readers checked commit and PR wording. No implementation work was delegated.

### Landing

Landed on its own in round 75, as #2194, unchanged from what the report describes.

#### What landed since the PR was based, and whether it interacted

Only plan-queue bookkeeping (claims and report deliveries, no code) reached main between this PR's base and its landing,
so the landing rebase was clean and the PR landed byte-identical to the head the executor delivered. Its base already
held the other recent keyboard fixes: the template panel's composing-Escape fix (#2115), the menu focus fix, the
new-session dialog fixes and the keyboard-scrollable host plans and trash. I read the PR against them, and a
fresh-context reviewer did the same independently. Neither found an interaction. The new guard runs before every other
window-wide key listener, leaves the terminal's own input alone, and only marks a key in the field whose composition
just ended. The checks that already ignore composing keys, such as the template panel's Escape and dialog Escape, now
also ignore WebKit's composition-ending key, which is the intended fix. Menus, approval cards and the launcher's buttons
never compose, so they are unaffected.

#### Fixes made during the landing

None.

#### Things to keep in mind

- The Mac app has not been tried with a real input method. The browser tests replay key events in Playwright's WebKit on
  Linux. The changelog entry's first paragraph already calls this a partial workaround and names its gaps, but it
  describes the protected case without saying that it is unverified on a real Mac; only the body says so. Release
  curation should keep that qualifier in the entry unless the report's manual Mac check has passed by then.
- The guard only acts once the earlier typing keys have been released before the key that ends composition arrives. If a
  real Mac input method, or the user's fast typing, leaves an earlier key held at that moment, the guard stands aside
  and that key behaves as it did before this change. Only the manual Mac check can show how often that happens.
- This PR removed the near-term TODO entry for WebKit input-method guards, as a fix should. The known gaps (an earlier
  key still held, or an ending key that arrives after 16ms) are also recorded in SPEC_impl.md and the changelog entry,
  but the pending manual Mac check is now recorded only in this report.
- Minor: the new SPEC_impl.md section sits between two paragraphs about session menus, and its "matching key code" means
  the physical key, not the 229 key code discussed next to it. Moving it is a possible documentation follow-up; nothing
  depends on it.

#### Checks

The report says its clippy, desktop renderer compile and desktop asset parity check ran without a recorded revision,
before the executor's last code change. Those are cheap, so they were run again.

- Run now, at the rebased tip `58d60067`: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, and `scripts/check-desktop-assets.sh` (26 assets, requested set equal
  to bundled set), all clean. The new script is built into the page as source text rather than fetched as an asset, so
  the asset check confirms nothing else changed rather than covering it.
- Reused, because the landing rebase left the PR byte-identical to what the executor delivered: the JavaScript harness
  run `df3c7cad-8d6b-458d-ab9f-d4bb6c9efeab` (269 passed) and the final browser run of the composition cases on Chromium
  and WebKit `bb4f491b-c8ff-4f02-801b-3947509b0df3` (40 passed), both recorded at revision `6edab83d` whose patch the
  executor's clean rebase left unchanged; the final web release build at `6edab83d`; the transport proof run
  `290ff0ad-5eae-4b0c-89b2-33c6f888447c`; and its Markdown formatting, changelog-format and test-sleep checks.
- Skipped: native Mac input-method execution and the full workspace and browser batteries, for the reasons the report
  gives, and the backend `cargo build`, which clippy's full-target compile at the rebased tip now covers for
  compilation.

Nothing in the executor's report became untrue at landing.
