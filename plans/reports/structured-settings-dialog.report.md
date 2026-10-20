# Settings sections

## What this was about

Settings had become a long scrolling page. Related choices were hard to find, and reaching the trust controls meant
scrolling past unrelated settings. The agreed design was a section list like Templates, with drafts preserved when
switching sections and a one-section-at-a-time layout on phones.

The dialog now groups choices into sessions, hosts, terminal, checkouts, agent trust and notifications. Desktop apps
with an active updater also show app. The gear always opens sessions; trust hints open agent trust at the relevant
switch and its explanation. The setting labels, help, save behavior and storage scopes remain the same.

## Things you should know

The dialog is at most 760px wide, with a 148px section list. Header and footer stay pinned around one scrolling body. At
620px and below, it starts on the chosen section; **‹ settings** returns to the list. Section entries are ordinary
buttons using the platform's normal keyboard behavior. Selection focuses the new section heading, and returning to the
phone list focuses its selected entry. Escape and both close controls still return focus to the gear.

Switching sections keeps the existing controls mounted. Unsaved checkout and notification fields, save outcomes and
pending saves therefore survive navigation. Closing discards unsaved drafts as before; an explicit save already in
flight keeps its existing completion behavior. Device sound controls initialize even while their section is hidden. No
remembered-section preference, draft store, generic navigation component or new save mechanism was introduced.

The 14 affected guide pages, including the Manage hosts tour, describe the new locations. Its screenshot now shows hosts
beside the section list. All five locally emitted images were inspected, and the capture reviewer accepted the final
annotation placement. Nothing was published: the production image remains the preceding layout until the next full
screenshot refresh on latest main after landing. README, Overview, release images and screenshot metadata were left
alone. The completed TODO entry is removed in the final PR.

Main advanced during execution. The full intervening changes were inspected before rebasing onto bc6f5799. Shared dialog
framing and deferred host-dialog admission were the relevant interactions; the final browser selection covers those
interactions. Host-save refocus belongs to the separate host dialog and cannot take focus from Settings. The rebase
preserved upstream launcher changes and all completed TODO removals.

## Open questions and possible follow-ups

No product decision remains open. The next full docs screenshot refresh must publish the updated image after landing.
Native desktop controls and updater gating were checked in source; this run provides browser-engine coverage, not a
native desktop runtime verdict.

## The PRs

- [#2447](https://github.com/scode/farhelm/pull/2447/changes) — section navigation, preserved controls, affected browser
  coverage, specifications and changelog. Draft head `552d86af4c683dec4a85f37f24bff071b77820ac`, based on main.
- [#2448](https://github.com/scode/farhelm/pull/2448/changes) — updated guide locations and local capture specification,
  with the completed TODO entry removed. Draft head `6c3ffb85b8fd88d6af3afaf31832bb7415beedb9`, based on #2447.

## Checks run, reused and skipped

All runtime attempts used the test recorder. Rust used nextest 0.9.148, four slots and zero retries; browser checks used
Playwright 1.64.0, Chromium and WebKit, one worker and zero retries. Tmux 3.7c was required and verified. Successful
evidence has complete source identity, stream EOF, no retained-output truncation and no forced-cleanup limitation. No
runtime SKIPPED marker was found in the inspected output.

- `cargo nextest run -p farhelm-ui --lib`: 555 passed in run `77d5c3a6-3135-4901-8dad-c12e24f3fb30`, source 250edb69
  plus fingerprint `22fed872a33e243b76b2ac96a377b1dd652e40a1298ed39ab09bfb804a3e28bb`. These passes are reused for the
  unchanged Settings behavior; later Rust changes were formatting and restoration of two existing comments. Newly landed
  upstream unit cases are not claimed to have run here.
- Core browser command:
  `npx playwright test 'settings\.spec\.ts' 'claude-trust\.spec\.ts' 'workspace-trust\.spec\.ts' 'sounds\.spec\.ts' 'ntfy-settings\.spec\.ts' 'tooltip-coverage\.spec\.ts'`.
  Run `44ac4aa5-4287-4125-b823-c780dbb52201` executed 70 cases: 69 passed and one WebKit case failed before opening
  Settings, while waiting for the initial host row. Its trace showed host/session requests still loading at that wait;
  this is one startup observation, not an established latent flake or proven product regression. The failed run and
  trace were retained. Source 250edb69, fingerprint `185cbc35cbe18b721eaf9803a1f45c734e7355237f5a22676f771f053af939b4`.
- The exact unchanged failing case was then selected with the unique suffix
  `-g 'host browser permissions persist independently through the settings switches$'` in settings.spec.ts. Run
  `f6653b00-3321-43fa-a3b7-5aef9b04e8cc` passed both engine cases without increasing a timeout or changing product code;
  source 250edb69, fingerprint `4a360da9ea2f03f6a8b5e431b92b00bc6a95a0e0527b6e746faa615dcdb53894`. An earlier retry,
  `a629bebb-f255-4451-9b88-c8090abebfdb`, matched no tests because its grep anchored the short title at the beginning;
  it remains a retained selection failure, never a pass.
- Final browser run `31d47cb0-a5e7-4517-9ffe-4c72a0dd3b54` passed all 44 cases, 22 per engine, on rebased source
  bc6f5799 plus fingerprint `f51a909eabf15673b2d084bfb663f7c71e4a11028b2aff2759c514f5feb70dff`. The command was
  `npx playwright test` selecting provisioning, terminal-link-clicks, session-confirmations, quick-launch, terminal,
  session-action-shortcuts, settings and tooltip-coverage specs, with a grep limited to the moved-control, delayed-host
  dialog, modal shortcut, pinned-close, navigation and tooltip cases. It covers the 30 outstanding affected cases plus
  14 rebased frame/modal/navigation cases. All cases executed; none failed, skipped, timed out or remained unstarted.
  The earlier 69 core passes and two exact retry passes are reused for their unchanged test paths. The new navigation
  sizing assertion and shared-frame/modal interactions were executed again in this final selection.
- `scripts/docs-screenshots.sh --only manage-hosts --no-build` reused the matching rebased app builds. Final run
  `f9f0db09-095e-41d7-82c6-9d22dc788d9a` passed all 6 staging/capture steps and emitted five inspected PNGs. Source
  bc6f5799, fingerprint `f51a909eabf15673b2d084bfb663f7c71e4a11028b2aff2759c514f5feb70dff`. The preceding run
  `02853113-cae3-4b9e-9da4-16260f26255b` also passed its 6 steps, but its settings image failed visual inspection
  because an arrow crossed help text. That image and run were retained; they are not the final visual pass.
- Rebased `cargo build -p farhelm`, dx 0.7.10 release web build, `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, scoped dprint and `python3 releasing/check-changelog.py format` passed. The isolated
  test-sleep checker passed 305 delays with zero unannotated. Frozen Bun install and the website build passed, including
  internal links. These cover the final implementation; subsequent capture-only annotation offsets do not alter the app
  builds, Rust checks, website links or delay inventory.

The full Rust/browser batteries and CI were skipped because the selected checks cover the changed controls and their
concrete interactions. JavaScript units were skipped because this plan changes no JavaScript. Native desktop execution,
installer, provisioning-distribution and release gates were skipped because their mechanisms are unchanged. No live
installation was modified.

## The review gate's outcome

Each PR received the required single fresh gpt-6.1-sol/high review for correctness, design and idiomatic code. The
structure reviewer checked the rebase, navigation sizing and test assertions, and accepted restoration of comments that
a formatter had misplaced. The docs reviewer found that side callouts would exceed the screenshot's fixed crop; moving
them above the dialog and inspecting the final images resolved that finding. No findings remain. Both message cold reads
passed. Separate documentation passes covered all touched files.
