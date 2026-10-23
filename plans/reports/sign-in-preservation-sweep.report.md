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
