### What this was about

Templates previously had one direct-launch favorite, marked ⚡. The maintainer wanted ten additional favorites in the
Templates list: give each a number key, then start its session with Cmd+digit in the Mac desktop app or Ctrl+Shift+digit
everywhere else, including a browser on a Mac. The row's key button now assigns, moves or removes a binding. Number keys
and ⚡ are independent and shared across windows.

### Things you should know

A template has at most one number key; each key has at most one template. Rename keeps its key, delete frees it, and
Undo restores only the template. A template must choose an agent or a nonblank command to be assigned a key. An already
bound template that becomes incomplete still permits removal. The popup uses the existing menu keyboard and focus
behavior inside Templates, closes on movement and keeps Escape local to the popup.

Number shortcuts use the same launch path as ⚡, including remembered defaults, host readiness, YOLO confirmation,
launch refusal and selection of the new session. An unbound key opens Templates with binding help. A YOLO question,
refusal or incomplete template reveals the ordinary New form with its draft retained. An open dialog receives the chord;
repeats do not create more sessions, and a claimed launch chord does not reach the old terminal program. Only the
physical digit row counts, not the numeric keypad.

Giving a template a key also consents to creating any managed checkout it names without showing its path first, as ⚡
already does. Each press reads shared bindings and templates again. Simultaneous binding edits from different windows
can lose one edit because the helm replaces the whole map; this was explicitly accepted.

The helm database moves to schema 50. An older helm then refuses that database; there is no downgrade path. The storage
changelog fragment says so. This is the accepted schema change, not a supervisor-protocol change.

The required hover-help check found existing omissions in Settings. This plan also adds help to ntfy controls and the
terminal-link choices, and disables browser correction/remembered-value hints on the ntfy URL, topic and token fields.
It changes no alert delivery, saved-token, sound or terminal-link behavior. These narrow prerequisite repairs have a
separate fixed changelog fragment.

The Templates documentation screenshots omit the new row controls and are now stale. They were intentionally not
refreshed. Documentation was updated and built; nothing was deployed.

### Open questions and possible follow-ups

No maintainer decision blocks delivery. Refreshing the Templates documentation screenshots is a separate follow-up.
Chromium and WebKit automation delivered the web chord successfully. That does not establish whether every real browser,
operating system or keyboard configuration reserves it. Cmd+digit's Mac-desktop modifier logic has direct unit coverage;
this run did not drive a physical Mac desktop window.

### The PRs

- [#2008: shared number bindings](https://github.com/scode/farhelm/pull/2008/changes) — storage, validation, atomic
  deletion cleanup and the schema downgrade caveat.
- [#2013: launch templates with number keys](https://github.com/scode/farhelm/pull/2013/changes) — row controls, shared
  direct launch, preference recovery, specs/docs, focused tests, Settings hover help and removal of the covered TODO.

Both PRs remain drafts. The executor marked neither ready and merged nothing; the landing monitor owns landing.

### Checks run, reused and skipped

Runtime observations use the recorder. Rust selections used pinned nextest and tmux, four slots and zero retries;
browser selections used pinned Playwright, one worker, zero retries and both Chromium and WebKit. Counts below overlap
and are not a sum of unique tests.

- Storage: `cargo nextest run -p farhelm-helm --lib` filtered to schema, number bindings, quick-launch marks and
  preferences passed 22 tests, run `390c29e6-775e-43ef-819d-380ddc7cdd08`. Review then found a historical checkout
  fixture that retained the new column. Its exact two tests failed in `1d28c7c5-ed1e-4689-b2bd-e2a584f239fc`, then
  passed after repair in `a2e2eec2-d2b4-4147-8023-3a32591beedd`. Reuse the original 22 outside that fixture.
- UI helpers: the final stable selection
  `cargo nextest run -p farhelm-ui --lib -E 'test(api::preference_write_tests::) | test(list::templates::tests::) | test(ntfy_settings::tests::)'`
  passed 28 tests in `d4105acc-3b2a-4ce0-a36c-66b85002139a`. It covers binding transformations, rename, deletion
  retirement, acknowledgment, replay and token semantics. Later popup lifecycle/hover changes do not alter those
  helpers; their rendering is covered by the final browser run. Earlier UI run `fc13066c-16db-4150-8687-895371396a13`
  passed 27 but had source edits while compilation waited, so it is superseded by the stable observation.
- The UI asset JavaScript harness, `node --test`, passed 244 tests in `32f1da3f-fe87-4a67-97ca-d601574e875a`. The
  shipped physical-key matcher is exercised for both build conditions, all ten digits, wrong modifiers, composition and
  keypad exclusion. That asset and its tests were unchanged by the popup corrections and rebase, so the result is
  reused.
- Initial browser selection `quick-launch.spec.ts`, `templates.spec.ts`, `tooltip-coverage.spec.ts` ran 106 cases: 104
  passed, two Settings-tooltip failures, run `3bebdea1-50b6-448c-b544-d62de7e80cfd`. The unaffected passes are reused.
  Narrow tooltip reproduction `172f9b16-f408-48c8-8023-88bc193a0ee2` confirmed both failures. An earlier mistaken
  zero-case grep, `ff90438b-1dbf-4400-9340-750645d7cde0`, is retained as a selection failure.
- Review regressions reproduced six popup failures and two strengthened-modal passes on the original build in
  `e8bc566f-d8da-4a9c-b918-31b372f9de64`. After the first correction, the selected number launch, YOLO/refusal, modal
  and popup cases passed 16; two tooltip cases then exposed terminal-link hover omissions, run
  `7872a70e-9d54-4e8c-ae6c-1fc0f0e9f62a`. Its launch/modal passes remain applicable.
- Width-only movement reproduced in both engines in `6394c640-dea7-4010-a20a-e09a44d03167`; that run also contained a
  bad delayed-close locator and was interrupted once diagnosed. Corrected delayed-close reproduction
  `c5d4d537-fe0b-4fe9-a704-906da35534d8` failed the intended focus assertion in both engines. These remain failed
  observations, not passes.
- Final `templates.spec.ts`/`tooltip-coverage.spec.ts` selection for number popups, binding/rename/delete/Undo and
  sidebar hover help passed all 14 cases on both engines, run `0cb299c4-7f1d-4e74-aa97-7d1fe2f59c72`. This covers held
  pointerdown, scroll, size-changing resize, width-only centered movement, delayed-close ownership and repeated relay
  delivery. The executor inspected the retained number-menu image for clipping and readable controls.
- Passed targeted `cargo clippy -p farhelm-ui -p farhelm-helm --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `cargo check -p farhelm-ui --tests`, Rust formatting, changed-Markdown
  dprint, changelog format and the isolated source-only test-delay checker. The final checker found zero delays without
  a rationale. The explicit Dioxus web build and browser CLI prerequisite build passed. One intermediate web build
  rejected an attribute after an RSX child; the corrected build passed.
- Website `bun run build` passed for the changed docs, 32 pages with valid internal links. Subsequent source changes did
  not touch these pages. The result is reused.
- Careful rebase onto `db33611cb0df601da3bef7f319c5f7480101efaf`: read incoming release-notes menu and signed
  release-advisory diffs, plus review instructions, TODO and queue/report changes. They change help/version
  presentation, release verification and desktop updating, not shortcut storage, New submission or popup helpers.
  Resolved the TODO conflict by keeping both completed-item removals. Comparison confirmed the tested
  shortcut/backend/test sources are identical after rebase. Re-ran targeted UI/helm Clippy, desktop compilation,
  formatting, changelog and delay checks; all passed. Reused the runtime and website observations because no covered
  behavior or documentation changed.
- Skipped full workspace/runtime/browser batteries, desktop GUI smoke, installer/provisioning/release gates and hosted
  CI. Storage, preference recovery, direct launch, popup lifecycle and hover behavior have focused evidence; no
  installer, desktop entry point or release artifact changed. No new executable documentation requires doctests. No
  screenshot refresh or website deployment was authorized.

All failed/interrupted runs are retained privately. These are deterministic omissions or introduced test/product
failures from this session, not established latent flakes; no public flake-log entry was added.

### Review gate outcome

Required gpt-6.1-sol reviewers at high effort reviewed each PR independently. Storage review found and corrected the
historical fixture. UI review found the open-toggle pointer race, missing layout invalidation and weak modal assertion;
the correction review found capped-dialog movement and delayed focus ownership. All findings were accepted, reproduced
where applicable and corrected. The required scope reassessment reused the existing checkpoint and found no unnecessary
subsystem: observing the backdrop and enforcing existing menu identity were narrower than a new popup framework. The
native-select fallback remains available but was not needed. Final source review found no remaining findings, including
the Settings attributes and full changed-test authoring checklist. Reviews were source-only; execution and visual
evidence were collected by the executor.

A separate documentation pass covered every touched file. Commit/PR wording passed fresh cold reads. Report delivery
receives the required separate native cold read for product understanding and public-repository hygiene. Implementation
stayed local under no-workhorse; only required reviews and cold reads were delegated. Private galaxy-brain records
retain review outcomes and available per-turn external counters. Initial accounting was delayed; exact external model
attribution and native/orchestrator usage are unavailable. No cost comparison is claimed.
