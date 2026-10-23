# Sweep out browser sign-in and failed-save preservation

## What this was about

On 2026-10-22 you decided two rules (SPEC.md, "Signing in again" and "One GUI at a time"). First, when a browser's
sign-in expires, recovery is a literal page reload, and nothing the helm had not accepted is kept. Second, a failed GUI
save is reported and discarded, never kept or resent. The code still did the opposite. After a new browser token was
accepted, the app put the interface back inside the same page, with extra code to survive that. The shared settings
(list order, selection, layout, confirmation choices, hint dismissals, the quick-launch mark and so on) kept a failed
value, showed it over later reads, and sent it again after signing in. This plan removes that machinery, as the TODO
entry "Sweep out browser sign-in and failed-save preservation" asked, and the code is smaller for it.

On your choices from the planning page:

- A best-effort setting whose save fails stays in effect on screen until the next read from the helm, is only logged,
  and is never resent. SPEC.md's best-effort exception now says so in one clause.
- Settings outside SPEC.md's best-effort list that already fail silently are now discarded like the rest, with no new
  reporting.
- Browser recovery keeps the token form first and reloads the page once the new token is stored.
- Dialogs that keep what you typed after a refused save are untouched, and so is the way Create and Replace recognize a
  second click after an unknown outcome, so that it does not launch twice.

## Things you should know

- After signing in again in the browser, the page reloads. Open dialogs, drafts and in-flight edits are gone. The
  session you had open normally comes back, because the last selection is remembered on the helm.
- If the browser cannot reload, the token form says "signed in; reload the page to continue" instead of hanging.
- A failed setting save now lasts only until the app next reads settings from the helm. Before, it came back after the
  next browser sign-in and was sent again. That resend could also undo a newer choice made in another window. A queued
  review finding about exactly that case (a failed confirmation opt-out replayed over a later restore) is closed by this
  work and removed from the review queue.
- Renaming a template whose quick-launch mark or number key fails to move now shows them where the helm has them, not on
  the new name. When toggling the quick-launch mark fails, the next quick launch uses the helm's mark. One browser test
  asserted the old opposite behavior and was inverted.
- One case still rebuilds the interface inside the same page instead of reloading it: a build-mismatch notice appearing
  while the sign-in form is showing. State that used to be carried across such a rebuild (whether the welcome card had
  already been shown or skipped, the list's count of operations in progress) no longer is. That is acceptable because
  the notice already tells you to reload.
- Production code shrank: about 35 lines added and 150 removed across the stack, and about 400 more lines of tests and
  comments removed.

## Open questions and possible follow-ups

- Several shared settings still fail silently, although SPEC.md asks every failed operation to be reported: the welcome
  card being seen, the trust-hint dismissals, the terminal link-click choice, frame colors, naming sessions on quick
  launch, and toggling the quick-launch mark. You chose to leave that gap for a follow-up.
- No browser test covers a template rename whose quick-launch mark or number-key save fails. The code that puts them
  back where the helm has them is one line each, so a test for it is a possible follow-up.
- On this machine, under heavy load, the browser case "the sidebar app bar explains the helm and window builds" failed
  with Playwright's "Route is already handled!" on this stack and also on unmodified main. FLAKES.md already records
  that error class for this and other tests. It may now fail reliably rather than as a flake; it is worth a look.

- The Templates browser case "templates busy root stays keyboard-scrollable" fails intermittently on Chromium: PageDown
  does not scroll the editor while a template save is held. It failed in 1 of 4 runs on unmodified main (run
  `2135ea16-ec9c-43bd-9032-d5b76aded30a`) as well as on this stack, so it is not from this work. It may be a new flake
  worth recording.

## PRs

- #2604 docs: say failed best-effort preference saves are not resent
- #2614 fix: reload the browser page after signing in again
- #2623 fix: discard failed GUI preference saves instead of resending them

## Checks

The stack was rebased onto current main after the work was done, because the plan that ties GUI answers to what the user
saw had landed meanwhile and touched the same list and Templates code. That rebase was textually clean apart from the
TODO list, and one comment about the key-press clean-up that plan added was corrected (that clean-up itself stays).

Run now, on the final, rebased stack:

- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, the same clippy for the
  wasm32 target, and `cargo check -p farhelm-ui --features desktop`: all clean.
- Recorded `farhelm-ui` lib tests: 752 passed (run `cdd3289d-9bf6-4afa-9c4c-278d1032a8ae`).
- Recorded browser run of quick-launch.spec.ts, templates.spec.ts, auth.spec.ts and welcome.spec.ts on Chromium and
  WebKit: 220 passed and 2 failed (run `c811966d-8a63-403f-9d36-cfcc94f6770a`). Both failures are the Templates case
  named under the follow-ups, which fails the same way on unmodified main.
- `dprint check` on the changed Markdown, the changelog format check, and the agent-instructions check passed.

Reused, from before the rebase:

- PR 1 (#2604): `dprint check SPEC.md`; the rebase did not change it.
- PR 2 (#2614), on its pre-rebase revision: recorded browser run of auth.spec.ts and welcome.spec.ts on Chromium and
  WebKit, 32 passed (run `a0583e8d-3f96-47e9-af9c-543ef4889971`), since rerun above on the rebased stack.
- PR 3 (#2623), on its pre-rebase revision: the twelve specs that fail settings saves on purpose (session-confirmations,
  feedback, workspace-trust, modal-layout, launcher-template-shortcuts, goose-pi-composer, launcher-tabs, welcome,
  quick-launch, sort, provisioning, templates) on Chromium and WebKit, 522 passed and 2 failed (run
  `13c5580c-1cdf-47d4-a545-ebb708fa3633`), and sidebar.spec.ts on both engines, 245 passed and 1 failed (run
  `967fd865-a488-4ade-ba84-87c1fb20a290`). The rebase changed none of the code these exercise except the list and
  Templates code, which the rerun above covers. The sidebar failure is the build-information case named under the
  follow-ups, on Chromium; run `0d60ea1c-2150-40bc-a3df-35740f680e28` on main fails the same case the same way. The two
  provisioning.spec.ts failures were "cancel and a permanent answer on a host row's setup question change no preference"
  (Chromium, a host row's menu did not open on Enter, before any setting is saved) and "delayed host dialogs wait for
  Settings: two plans" (WebKit, the 60-second test timeout). Neither matches a recorded flake. They are blamed on load,
  not on this change: neither failing step saves a setting, the machine's load average stayed between about 30 and 47
  all session, and a narrow rerun of all four variants of both cases passed 8 of 8 on both engines (run
  `6c5388d1-7dab-4a04-9037-bc731bb501fe`).

Skipped: the rest of the browser suite and the other Rust suites. The changes are confined to how the app saves shared
settings and to the browser sign-in form, and the chosen specs are the ones that exercise them.

## Review gate

Each PR had one fresh Opus 5.5 review at high effort. None found a correctness bug in the final code. PR 1: the clause
was reordered so "exception" attaches to the part that departs from the rule. PR 2: stale documentation of the in-place
rebuild was fixed, one more piece of code that only served that rebuild was removed, and the browser test now waits for
the reload reliably. PR 3: a stale SPEC_impl.md sentence and comments were fixed, a redundant piece of bookkeeping in
the settings-saving code was removed, and the review-queue item this work fixes was deleted.

### Landing

Landed as #2604, #2614 and #2623, merged bottom-up, with the content the report describes and two small documentation
fixes described below. It was the only plan in this landing round.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained only a documentation commit and queue bookkeeping. The plan builds on several recent
UI plans already in its base, so a fresh-context reviewer and I checked it against them:

- gui-saves-last-write-wins, which made a failed number-key save be discarded, is now covered by this plan's general
  rule: its special case in the shared preference-saving code is gone with nothing left referring to it, number keys
  behave as before, and the Templates dialog still locks during a key save and puts the helm's keys back on failure.
- bind-gui-answers (the plan that ties GUI answers to what the user saw) clears queued quick-launch sequences when the
  sidebar list goes away; that is kept. The prompt it changed for leaving the template editor with unsaved edits, which
  lets the user save the same draft again after a failed save, falls under the dialogs this plan deliberately left alone
  (those that keep what was typed after a refused save), per your choice on the planning page.
- The page reload after signing in again runs right after the new credential is stored, and nothing in the app asks the
  browser to confirm leaving the page, so no "leave site?" prompt appears. Modal dialogs, agent approval cards, queued
  quick-launch sequences and the welcome card start fresh after the reload, as SPEC.md now requires. The desktop app's
  own authentication is untouched; it never shows the token prompt, so it never reloads.
- The SPEC.md wording agrees with the 2026-10-22 rules on signing in again and failed saves.

#### Fixes made during the landing

- SPEC_impl.md said that "a failed shortcut save" is reported and discarded. "Shortcut" there covers both number keys
  and the ⚡ quick-launch mark, but toggling the mark in the Templates dialog still fails silently (the gap the report
  lists as a follow-up); moving the mark during a rename is reported. The landing reworded the sentence to say that a
  failed number-key save or a failed move of either shortcut is reported and discarded, and that a failed mark toggle in
  the dialog is discarded without a report. This is in #2623.
- A review-queue item, `welcome-recovery-counts-unfinished-selection-read.md`, concerned a welcome-card test about
  recovering from sign-in inside the same page. #2614 deletes that test and the in-page recovery it exercised, so the
  landing removed the item and its index entry in #2614.

#### Things you should know

- The report says a queued review finding about a failed confirmation opt-out being resent over a later restore is
  removed by this work. That item had already been removed on main by the 2026-10-22 triage, so this plan's PRs no
  longer remove it; the behavior it described is still fixed by #2623.
- In the one case that still rebuilds the interface inside the same page (a build-mismatch notice appearing while the
  token form shows), besides the welcome card's state and the count of operations in progress that the report names, two
  more things are no longer reset: rows being deleted can stay shown as deleting, and activity times can be stale until
  their next refresh about 30 seconds later. Only a browser whose credential was already refused can reach this case,
  every request fails there anyway, and the notice already says to reload.

#### Possible follow-ups

- Found at landing, minor: two test comments still describe the old in-page recovery: the `authenticate()` helper's
  comment in `e2e/tests/auth.spec.ts`, and the first line of `e2e/tests/ntfy-settings.spec.ts` ("replayed preferences").
- For changelog curation: the `browser-sign-in-reload.md` entry presents losing open dialogs and drafts as new, but the
  old recovery already lost them; what is newly lost on reload is the rest of the page's state.

#### Checks

The plan adds about 330 lines and removes about 790 (counting tests and comments), in the UI, an asset script, three
browser specs, SPEC.md, SPEC_impl.md and two changelog fragments. Browser sign-in recovery now reloads the page and
every preference save goes through the changed code, so this round ran the full browser suite. No helm or supervisor
code changed, so the rest of the Rust workspace did not run. All of the following ran on the stack before the landing's
two documentation fixes, which change only SPEC_impl.md and the review queue:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (one warning, about a citation main already had), the workspace debug build, the
  UI's release web build and `scripts/check-desktop-assets.sh` (which also builds the desktop app): all passed. After
  the documentation fixes, `dprint check` on the changed files and the instructions check passed again.
- `cargo nextest run -p farhelm-ui` through the test recorder with pinned nextest and tmux, four slots, no retries, run
  `bf6f6c48-2829-462a-85b9-49bc97795e8c`: 752 passed. `cargo nextest run -p farhelm-ui --features desktop`, run
  `6f3e09f5-5383-42c6-b522-df2f7c8d9dfd`: 917 passed. `node --test` in `crates/farhelm-ui/js-tests`, run
  `171dd59a-31e6-4197-bc8e-271c65700811`: 333 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `aeb02db0-f5f1-4f00-a74f-9c81df1c2294`: 2479 passed, 14 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), 3 failed, all on WebKit and
  none in code this plan changes:
  - "a template shortcut preserves editor focus when clearing a quick-launch YOLO question", the flake found at the
    previous-but-one landing (failing about 3 in 10 on WebKit on main), whose record is in draft PR #2621;
  - "a wrong-host UPDATE acceptance warns on its source row and claims nothing", which failed in the test's own network
    interception ("Response has been disposed"), as it did once on Chromium three landing rounds earlier; ten WebKit
    repeats here (run `d0f245cd-37fc-4b13-bfd1-a296993fef07`) failed once more. It is a pre-existing flake I am
    recording separately;
  - "a file dropped on a tab inserts its path in that tab", which did not repeat: ten WebKit repeats (same run) and five
    Chromium repeats (run `c27091ce-0ae2-4ec7-b747-2b3837205e40`) all passed. I treat it as a single observation. The
    test exercises the browser's drop handling and the live terminal, and the plan's only change to the browser's
    event-feed script, which carries that terminal's updates, is a comment.
- Reused: none. Skipped: the rest of the Rust workspace (no helm or supervisor code changed), the website build (no docs
  page changed), the doctests (no documentation example changed), the desktop smoke test (the desktop app's own
  authentication is untouched and covered by the desktop-feature unit tests above), the native Mac app (not runnable
  here) and the installer (unchanged).

The report's checks were its own selections; this round ran the UI unit tests, the JavaScript harness and the full
browser suite on the plan as it landed. Nothing else in the executor's report became untrue at landing.
