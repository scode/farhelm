## What this was about

In the desktop app, a rapid menu arrow press could pull focus back after the user clicked another item or moved to
another control. Cancelling work still queued in Rust did not stop a focus move already sent to the window. The
maintainer chose a small page-side check for stale moves, while keeping keyboard navigation in Rust and preserving rapid
arrows and initial menu focus.

The change covers session menus, host menus, the help and update menus, and the Templates number-key popup. A delayed
arrow is rejected after independent focus, and the next arrow starts from the actual focused item. Initial opening focus
remains app-owned and preserves arrows already queued behind it. The TODO entry was removed, the implementation
specification updated, and a fixed changelog fragment added.

## Things you should know

Execution found three assumptions in the proposed outline that needed correction. The observer must exist before the
first opening. Each request must keep the focus epoch from its own key press, so a later key cannot revive an old move.
And Rust must distinguish an app-owned landing from an independent move onto the very same item. One synchronous input
relay carries the page's independent-focus epoch and current focus source before the ordinary focus event reaches Rust.
An old but legitimate initial opening therefore does not cancel a fresh arrow.

The existing serialized, coalescing queue and Rust keyboard rules remain. No request ledger, rejected-result handling,
framework change or per-menu navigation rewrite was added. Item markers use the menu instance, opening and action,
rather than position. Two fresh own-model scope assessments supported the bounded corrections. The measured change is
151 nonblank, noncomment product Rust and JavaScript lines: close to the approximate 150-line budget, rather than well
over it. The required reviewer found no complexity-gate violation.

A development version using only epoch equality lost a fresh arrow behind an old initial opening. Its deterministic
browser test failed in both engines; that same scenario passed after the focus-source correction. The failed run is
retained as same-session development evidence, not classified as a latent flake.

The careful rebase includes newly landed host-entry session IDs, helm runtime ownership, host-panel resize dismissal,
and template-list label refresh. The complete upstream delta was audited in bounded groups. The patch was unchanged by
the clean rebase; the new tests already pass full session references through the updated helpers and wait for host rows
to settle. Matching backend and web builds were regenerated for the integrated browser checks.

## Open questions and possible follow-ups

No required work remains for this plan. Automated coverage holds real page-side calls in flight in Chromium and WebKit;
it does not exercise timing inside an actual desktop webview. The separate race when a closing menu returns focus to its
toggle while another popup opens remains outside this arrow-navigation fix.

## The PRs

- [#2438: stop delayed menu focus from overriding clicks](https://github.com/scode/farhelm/pull/2438/changes), draft,
  head `44e2c45eb73af0dea08acccec05b2195e7e7cd69`.

## Checks run, reused and skipped

Runtime checks used the recorder on Linux x86_64. Browser runs used Chromium and WebKit, one worker, zero retries and
pinned tmux 3.7c. The real test helm, supervisors and SSH connection ran; host provisioning commands used the harness's
explicit injected backend, so these checks do not claim to install onto a remote machine.

- Recorded `npx playwright test 'sidebar\.spec\.ts|feedback\.spec\.ts|templates\.spec\.ts|terminal-multihost\.spec\.ts'`
  with the menu-contract grep selection: 32 passed, 16 per engine, zero failures, skips, unstarted cases or errors. This
  covers all eight held-request scenarios, session wrapping and dismissal, help-menu keys, host keyboard/order/busy
  behavior, and Templates popup arrows under modal isolation. Run `c00e8908-9a6c-4f93-aa01-a92d67a0704f` has complete
  source/report/output and cleanup; its source is the published head above, fingerprint
  `a644347c2fac0b7912afaaa2652a8ad84f51c74a6350fa55b85688bad88ec996`.
- Before rebase, the same 32-case selection passed in run `8dedc159-55b9-4639-8c8e-9f889b07c30a`. A direct Templates
  ArrowDown/ArrowUp check was then added and passed in both engines in run `2699aa2e-b8b4-43d8-8df7-ad1184d68db9`. These
  are historical observations; the integrated run above supplies post-rebase browser evidence.
- `cargo build` and `dx build --package farhelm-ui --platform web --release`: passed after rebase onto `2b185f71`. Main
  subsequently added only the cleanup-on-quit specification section through `b38bb6f1`; its complete diff was read and
  has no menu interaction, so no further rebase or runtime repeat was needed.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, changed Markdown formatting, changelog
  format and the test-delay source checker passed. The last delay check inspected 304 delays with none missing a reason.
  The agent-instruction checker passed with zero errors or warnings. A TS-only dprint invocation found no applicable
  files because this repository's plugins format Markdown, JSON and TOML; it changed nothing.
- Reused `node --test` from the UI asset harness: 322 passed, including 11 new guard tests, run
  `7bb4a906-78bd-418f-9c7d-63383a73b37b`, source `00e0b0a7` plus the recorded patch. The shipped JS and its tests are
  identical after rebase. Also reused `cargo nextest run --locked -p farhelm-ui --lib -E 'test(menu_panel::)'`: 21
  passed, 528 selected out, run `542f5f5c-7092-427c-8bc3-e769bda76c53`, four slots and zero retries. The tested pure
  menu functions are unchanged; matching rebased UI compilation and browser checks cover integration. Formatting and
  Clippy evidence is reused for the unchanged menu patch; main's unrelated backend and UI changes were audited
  separately.
- The retained failing opening run is `fd5cfc13-c1cf-4ed4-af55-222f6b760a71`: two failed, one per engine. The corrected
  narrow run `f98cfaa3-f6f8-45c0-833f-8ce90697f43c` passed both before the combined checks. All cited runtime records
  have complete source, reports where applicable, retained output through EOF and completed cleanup.

Skipped the full browser and Rust suites, installer, release and desktop-smoke checks: the identified integration risk
is the shared menu focus path, covered by the selected callers. The native event bridge itself was not changed. Real
desktop webview timing remains the coverage gap stated above.

## Review gate outcome

The required single fresh gpt-6.1-sol reviewer at high effort reviewed correctness, design, language idioms and the full
test-authoring checklist. Startup, same-target independent focus, immutable request epochs and initial-opening ownership
findings were corrected and rechecked by that same reviewer. The final Templates addition and careful rebase passed its
rechecks with no remaining findings. Commit and PR wording passed a fresh cold read after replacing unexplained focus
jargon. The delivery report passed its separate required cold read.

### Landing

Landed as #2438, with the same content the report describes, rebased onto this round's main. It was the fifth of six
plans in this landing round, after the four listed in the earlier plans' notes, most recently test-hygiene-oct19.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 108's sixteen plans and documentation and plan bookkeeping. A fresh-context
reviewer and I read them and the other plans of this round against this plan. This plan adds a check to every menu, so
the question was whether round 108's focus changes or the other plans here open menus or move focus in a way the check
would misjudge:

- Every place that builds a menu (session rows, host rows, the help and update menus, and the Templates number-key menu)
  passes the new information the check needs, and round 108 added no new menus.
- app-dialog-focus's handling of host dialogs and its update-ready focus handoff count as focus the user or app moved
  elsewhere, which correctly makes a late arrow press stop rather than pull focus back.
- template-editor-and-listing, earlier in this round, changes the same Templates number-key menu and its browser test;
  the combined test passed on both engines.

Two conflicts were resolved by keeping both sides: the Templates browser test's description above the number-key test,
where template-editor-and-listing added a sentence, and TODO.md, where round 108 had already removed neighboring entries
and this plan removes only its own.

#### Fixes made during the landing

None.

#### Checks

Every plan in this round was executed while round 108's sixteen plans were still unlanded, so none had been tested
against them. Together the six change a menu focus check that every menu uses, which session a page opens on load, the
desktop app's quit path, helm preferences and storage, and a browser-test helper many specs share. That is why this
round ran the whole Rust workspace and the full browser suite. All of the following ran on all six plans stacked
together on main, including the fixes made while landing them:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and
  `scripts/check-desktop-assets.sh`: all passed (formatting and the test-sleep check again after the last test fixes).
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `8c534e03-3c81-4f66-99e3-0e6ffdb526e8` (kept): 3951 passed, 3 failed. Two were
  helm database tests fixed during this landing (see select-newest-on-close's notes); the helm's checkout configuration
  test module then passed, 28 tests, run `faf4d80d-ec6d-4a36-9220-9d522e8f38d0`. The third was
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, a known open flake under load (FLAKES.md and
  the TODO deflake entry "Shallow pause resets under load"); no plan here changes terminal streaming or the supervisor.
  Five repeats of that test by itself, with the browser suite running at the same time, gave four passes and one failure
  (run `e34c4af0-729c-4f62-b1bb-db1c6d9e97e2`, kept).
- `cargo nextest run -p farhelm-ui --features desktop`, run `03589e5a-0f7e-423b-ab65-5c3bdd33c33c`: 720 passed; this and
  the desktop compile check above cover the desktop quit change, which lives in the UI crate's desktop code.
  `node --test` in `crates/farhelm-ui/js-tests`, run `c600b345-76c4-42a8-a879-fce21765c8d6`: 328 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, in two parts.
  The first run, `68484e27-6c23-45a6-aae5-5539e8c275b2` (kept), was cut short part-way by my own mistake: a narrow
  re-run I started in the same checkout removed files the running suite depends on, so every test it started afterwards
  failed before testing anything. Its results stand for the 64 projects (one engine and one spec file each) whose tests
  all started before that point and passed. Every other project, and the spec files changed by later landing fixes, ran
  again on both engines in run `49ead42e-f608-4e76-a833-6ff945dfb1f8`: 1538 passed, 12 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), and 5 failed. Those 5, and the
  4 the first run had shown before that point, were all tests that assumed how a page behaves on load (which session it
  opens, or how many preference reads and saves it makes) before select-newest-on-close changed it. They are 6 distinct
  tests, all fixed in that plan and passing in narrow runs on both engines (listed in its notes).
- Skipped: the workspace and desktop doctests (no plan adds or changes a documentation example),
  `cargo check -p farhelm-desktop` and the desktop smoke test (the desktop app's own crate is unchanged; its quit path
  change is in the UI crate covered above), the native Mac app (not runnable here) and the installer (unchanged).

The report's checks ran on a main from before round 108's landings, so they no longer describe what landed; the combined
runs above supersede them. Nothing else in the executor's report became untrue at landing.
