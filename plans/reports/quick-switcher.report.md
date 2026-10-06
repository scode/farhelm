### What this was about

Farhelm had no keyboard way to find a session across hosts or start a named draft from the same search. Cmd+K on macOS
and Ctrl+Shift+K elsewhere now open a quick switcher in the web UI and desktop app, including from a focused terminal.
It searches every host, ranks title matches before host or directory matches, and keeps recent activity order within
those groups. Picking a session uses the sidebar's ordinary busy refusals and makes a hidden host visible; Escape
restores the previous focus.

Plain typing also offers a pinned New row with the typed name and ordinary New defaults. `tl:` replaces those results
with templates using the launcher's existing search. Both open a draft for review; only pressing Launch starts it. The
keyboard quick-switcher TODO entry is removed; suspended sessions remain separate wanted work.

### Things you should know

The agreed chords, session scope and pinned template hint are preserved. Session searches use one snapshot per opening
and the helm's existing 500-session limit, with a visible truncation notice. Matching uses Unicode lowercase without
normalization or full case folding. Session loading has no keyboard selection, including New; after a failed session
read, New and template search remain usable.

Review caught focus and close races, clipped ended-status badges, and a switcher choice leaking into later Clone/Replace
drafts. These were corrected. A template choice now shows a cancellable loading state until the launcher has its
templates, model catalog and hosts; this prevents late application from overwriting user edits. Name choices need only
the normal mount-time defaults to finish. Clone/Replace supersedes any pending choice, and closing discards its opening
intent.

The stack is based on ce82a101. The intervening feedback-contact and stale-notification changes were read for functional
interactions; they change independent preferences, notification state and controls. No interaction was found, and the
final fetch found no newer main changes.

### Open questions and possible follow-ups

No decision blocks delivery. Real Firefox interception was not exercised: no Firefox executable or native keyboard
control was available. The agreed plan explicitly permits skipping a difficult or unavailable real-Firefox check while
keeping the chosen chords. This is not evidence that the chords succeed or fail there. Native Mac Cmd+K behavior remains
on the manual checklist; WebKit browser tests and desktop compilation do not prove native shortcut handling. No
per-browser chord was introduced.

### The PRs

- [#1691: keyboard session search and jumping](https://github.com/scode/farhelm/pull/1691/changes), draft,
  `plan/quick-switcher/01-session-jump`, c531d651fddd71a54a9f3d44c6f1712d7409ba5a.
- [#1692: named New drafts and templates](https://github.com/scode/farhelm/pull/1692/changes), draft,
  `plan/quick-switcher/02-new-and-templates`, 8ec4f21896bce44546abc7abbfab80e1d327821d; based on #1691.

### Checks run, reused and skipped

The final UI changes passed `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-desktop`. Fleet binaries and the release web
bundle built successfully. Targeted dprint checks, changelog format lint and the isolated source delay checker passed;
269 delays have reasons. The changed docs page passed the frozen website build and internal-link checks. All retained
phone/desktop switcher and template/launcher images were inspected.

Recorded Rust execution used pinned nextest and tmux, four slots and zero retries. Matcher run
a193cd39-c60c-4f34-b190-bf5a3b73653a passed all three selected cases on the rebased first PR. Run
66055a9c-8407-4a7b-bfd7-e4ccb5c59d81 selected `list::create_form`, `list::quick_switcher` and `launch_composer`: 83
passed, 350 selection exclusions, no runtime skips. These pure search, matching and template-application checks are
reused from PR 2's pre-correction tree, 64dc1881: their code is unchanged by the opening-lifecycle and loading-render
fixes, whose behavior was checked in the browser.

Recorded Playwright execution used Chromium and WebKit, one worker and zero retries. On the first PR, run
6a30592e-d9f9-43a2-8497-0a9b9d0b42fe passed 14 cases but had two new busy-fixture failures; the test assumed the wrong
disabled control. After correcting that premise, exact reproduction bc4e56c0-50d9-4230-9e1c-d0e1bd6e87e6 passed both
engines. This covers terminal chord ownership, focus handoffs, host-filter reset/refusal, modal interception,
loading/error behavior, layout and tooltip coverage.

On PR 2's pre-correction tree, run e591b218-9ac0-4840-bd6a-834d4aeab38c passed 28 cases across the switcher, composer
search and existing template acceptance. Two new template-fixture cases failed before reaching the UI because the
permission reset used an invalid wire value. The fixture now uses the endpoint's explicit reset. Exact delayed-template
reproduction 71530e78-bd3a-44f7-9a9f-827cad8a579a passed both engines on the final code, checking independent
template/catalog gates, applied fields including permissions and trust, and no implicit launch. Run
c38eb357-2f0b-4e7d-90c1-a7129a49ff50 passed both engines for ordinary New defaults and Clone after canceling a name
pick. The other successful search and switcher cases are reused because their exercised paths did not change. All failed
runs were retained; these were newly authored same-session fixture errors, not latent flakes.

No full workspace or browser battery was run: focused checks cover the changed search, modal and draft lifecycle
behavior, and the stack does not change supervisor execution, provisioning, installer or release contracts. Existing
builds and website evidence were reused where the final diff changes only representation, comments or independent
lifecycle code. TypeScript is excluded by the repository's dprint configuration. Firefox and native Mac execution were
skipped for the limits above.

### Review gate outcome

Each PR received two fresh-context source reviews: GPT-6 Astra at high effort and Claude Opus 5.5 at high effort. Both
received the plan charter and full test-authoring checklist. Confirmed findings were addressed locally, including the
lifecycle leak and delayed-edit window. No review swarm or implementation delegate was used. The optional pure Rust
gate-predicate test was declined in favor of browser proofs of actual resource ordering and reopening behavior. The
stronger session-loading keyboard behavior was retained and documented. No substantial departure or repeated corrective
review round required scope reassessment.

Commit and PR wording passed fresh cold reads. Separate documentation passes covered every touched file. The reviewers
initially found an unsnapshotted jj diff empty and used the equivalent diff against the first PR; this was reconciled
without reviewer VCS writes. Private delegation evidence is retained under session 909fe8bc-c1a2-41fe-b106-57780c627c93.
Claude terminal usage is recorded; native delegate and orchestrator usage is unavailable. This report has its own
independent cold read before delivery.

### Landing

Landed on 2026-10-06 (UTC) as two squash commits on main: #1691 (the keyboard session switcher) and then #1692 (New and
template drafts from the switcher). Nothing else reached main while they merged.

#### What else was on main

Nothing that could interact: between the commit the stack was built on and the landing, main gained only the planning
queue's own bookkeeping.

#### A fix made while landing: the switcher covered approval cards

A separate reviewer that had not worked on the plan found that the switcher's dimmed backdrop was layered above
everything else on the page. The stylesheet keeps one list of these layers, and in it the approval cards (an agent's
request waiting for your answer) sit above every dialog's backdrop, with hover text above them, so that an agent waiting
on you is never hidden behind a dialog. The switcher's backdrop was set far above both. While the switcher was open, an
approval card was dimmed and clicking it only closed the switcher, and the hover text on the switcher's own rows and
buttons drew underneath it. The landing put the switcher's backdrop on the same layer as the other dialogs and added it
to that list, in #1691 before anything merged. The switcher's browser tests passed on both engines with the change
(below).

#### Review before merging

Otherwise the reviewer found nothing that breaks. It confirmed:

- The new chord does not conflict with the Mac app's Cmd+N or the terminal text-size shortcut, and both shortcuts stand
  down while a dialog is open. Programs in the terminal lose no input: the terminal sends nothing for Ctrl+Shift+K or
  Cmd+K, and plain Ctrl+K still reaches them. On the Mac the only thing given up is the habit, from native terminals, of
  Cmd+K clearing scrollback, which Farhelm never had.
- #1692 only makes the Templates dialog's summary text reachable from the switcher; the dialog itself is unchanged. No
  switcher choice leaks into a later Clone or Replace, and Clone's fresh-checkout naming applies only to a clone.
- None of the screenshot, README image or demo video scripts presses the chord or depends on the sidebar's new hidden
  control.

It also found three smaller points the landing did not change:

- The report says the hover-text coverage test covers the switcher; it does not. Every new control does carry hover text
  in the source, but no test checks the switcher's.
- With the switcher, New or Templates open, a second press of the chord goes to the browser instead. From general
  knowledge, not tested: Firefox opens its Web Console on Ctrl+Shift+K and focuses web search on Cmd+K, and Edge
  duplicates the tab on Ctrl+Shift+K. Ignoring the chord while the switcher is open would avoid this.
- The chord matches the physical K key, not the letter the keyboard layout types (unlike Cmd+N, which matches the
  letter). On a Dvorak layout the key labelled K does nothing and another key opens the switcher. The terminal text-size
  shortcut documents the same choice; the switcher's code and SPEC_impl.md do not.

#### Checks

- Run now, on the final stack after the fix, through the test-run recorder: the quick switcher's browser spec on
  Chromium and WebKit (run `ee236fe6`, 16 passed), with the app rebuilt.
- Reused: the report's other checks. The fix changed one style value and a comment, and main brought nothing else.
- Skipped: Firefox and the native Mac checks, for the report's reasons.

The landing made one thing in the report above untrue: its claim that the hover-text coverage test covers the switcher.
