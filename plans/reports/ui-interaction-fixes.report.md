## What this was about

Three small UI interaction failures were grouped into one PR during triage. When Mark seen disappeared from an open
session menu, the next arrow could step from an obsolete position: Down from Replace could reach Delete instead of Stop.
In a terminal whose program handles mouse input, a plain click could re-copy an earlier forced selection and overwrite
something copied elsewhere. After an agent failed to start during Resume, Replace could discard the retained
conversation without warning. The maintainer accepted all three fixes with a small-change complexity gate.

## Things you should know

Session-menu navigation now preserves the selected action when the menu's item order changes. The latest requested
keyboard action is remapped by action identity, ahead of delayed focus events; if that action disappears, the existing
withdrawal/dismissal behavior applies. The planner preferred clearing the request, but review showed that this loses
rapid-arrow protection around item withdrawal. The triage decision allowed re-aiming or clearing, so the fix re-aims
using the existing menu state and lookups, with no new focus mechanism.

The terminal's copy decision now uses the mouse press's existing tracking and forcing information. A program-owned plain
gesture cannot copy retained local text; forced selections and deliberate reselection without tracking still copy.
Review also found that an old selection could suppress guidance for an uncopied plain drag after this fix. That obsolete
suppression was removed; clicks remain silent, forcing and OSC 52 exclusions remain, and the implementation
specification now describes the press-ownership condition already required by the product specification.

Replace on an errored session now says that any conversation it could resume will be discarded. The warning is
unconditional and qualified because an agent's failure to start does not establish whether a previous conversation
exists. No Resume-availability plumbing was added. All three outcomes are complete; their feedback files/index entries
are removed and their execution records name this change, bookmark and draft PR.

## Open questions and possible follow-ups

No decisions are outstanding. The host menu has the analogous obsolete requested-position pattern; the plan expressly
excluded it, so it remains a possible follow-up. The menu test composes pure decisions and does not execute the row's
effect or real DOM focus. Those wiring paths were inspected in source review; no interactive reproduction or browser
integration run was performed, and legacy mouse-reporting usage frequency was not measured.

## PRs

- [Draft PR #1812](https://github.com/scode/farhelm/pull/1812/changes) — the three UI interaction fixes, one
  commit/bookmark, left in draft.

## Checks run, reused and skipped

- Run: focused
  `cargo nextest run -p farhelm-ui --lib -E 'test(menu_panel::tests::) | test(replace_consequence) | test(every_status_warns_the_conversation)'`
  through the recorder, four slots and zero retries, `--tmux none`: 22 passed, 422 excluded by the filter, no runtime
  substrate skips; run `64910bf5-7ae9-4ae4-832b-bafa365ef28b`. Covers withdrawal combined with stale focus, ordinary
  arrow bursts and Replace consequences.
- Run: `cd crates/farhelm-ui/js-tests && node --test` through the recorder: 228 passed, zero skipped; run
  `b68a6ad4-b90a-4239-a116-03f1d4ab0593`. Covers press ownership, forced selection, retained-drag guidance and click
  silence against the shipped module.
- Run: an isolated scratch mutation removing tracking/forcing admission made the focused clipboard regression fail as
  expected (one failed assertion); run `f19911e1-c1ef-4fa9-a477-2c349e03b4ce`. This intentional negative control is
  retained privately and is not a flaky product-test failure.
- Run: `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, changed-Markdown
  `dprint check`, `python3 releasing/check-changelog.py format` (43 fragments), and the isolated-interpreter
  `python -B scripts/check-test-sleeps.py` (282 delays, zero missing rationales): all passed. Pinned nextest and tmux
  were prepared before Rust execution; these pure tests use no tmux.
- Reused: `node --check` on terminal.js from the initial implementation; later terminal edits were comments only.
  Earlier focused Rust run `cf363fd3-f313-4d08-8d23-26e0beedfe8b` and JS run `9f247438-75b4-48ac-a616-4dc2814f7968`
  passed before the review corrections; the fresh runs above cover the corrected behavior.
- Skipped: Playwright and broader workspace, desktop, installer and release runtime gates. The changes stayed in
  existing decision functions and menu reconciliation; focused tests plus source review address the identified risks,
  with the effect/DOM limitation stated above. Main's intervening docs/capture changes and plan approvals do not alter
  these interaction contracts; the rebase was clean and required no additional runtime coverage.

## Review gate outcome

The final fresh source review found no actionable issues. The earlier source review identified the focus-burst and
retained-selection notice interactions described above; both were corrected, and the final reviewer inspected the
resulting menu effect, terminal wiring, tests and specification directly. The scope checkpoint also caught the stale
withdrawn focus and unchanged-position bookkeeping cases, which were corrected within the existing effect.

Implementation remained local under no-workhorse mode. The prescribed source reviews requested native gpt-6.1-sol at
high effort; the wording cold reader requested gpt-6.1-sol at medium effort and its paraphrase matched the intended
failures. A fresh inherited-model scope checkpoint confirmed the corrections used existing facilities and stayed within
the complexity gates. Actual native model identities, reasoning settings and usage counters were not independently
exposed; private evidence remains under session `5c9810ab-dcb4-4a62-9a79-bfb239f2d810`. The executor did not mark the PR
ready or merge it.
