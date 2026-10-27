## What this was about

A terminal-link refusal could cover the warning in a session's Delete confirmation while its focused confirming button
remained usable. The Delete shortcut could open that question without moving the pointer off the refused link. The
maintainer's decision was: “It may not cover the warning.”

The refusal now stays below an open session confirmation, whichever opens first. The same rule covers Delete, Restart
and Replace, which share the confirmation panel. The layer-order explanation was updated, and the feedback item was
removed with its execution recorded in the ledger.

## Things you should know

The browser regressions exercise Delete through its keyboard shortcut in both opening orders, establish that the warning
and refusal really overlap, and compare their painted order. Restart and Replace share the checked markup and CSS rule;
they were inspected in source rather than driven separately. The approval-card exception in SPEC.md is preserved, with
its existing browser coverage passing.

## Open questions and possible follow-ups

None within the recorded scope.

## The PRs

- [#2740](https://github.com/scode/farhelm/pull/2740/changes) — keeps terminal-link refusals below session confirmation
  warnings; draft, not merged.

## Checks run, reused and skipped

Browser runs used `python3 scripts/record-test-run.py --runner playwright`, development mode, three configured worker
slots with one private stack each, zero retries, Playwright 1.64.0, and required repository-built tmux 3.7c with a
matching binary hash. Successful reports, source identity, output through EOF and cleanup were complete, with no skipped
cases.

- `npx playwright test 'terminal-link-clicks\.spec\.ts' -g 'header confirmation stays above a link refusal'`: 4 passed,
  two per engine. Run `2d90a292-a712-4eaf-bdf1-b6e174b9f657`.
- `npx playwright test 'terminal-link-clicks\.spec\.ts' -g 'covered terminal disclosures'`: 18 passed, nine per engine,
  including both new cases and existing approval, tunnel, pill and raised-refusal behavior. Run
  `091d475b-60f9-4993-a865-5c98959c3bfa`.
- Negative control restoring the refusal's old raised layer: all four new cases failed at the painted-order assertion,
  as intended. Run `5c3333e7-7d99-4da0-9225-7d011d46aa31`. The temporary test-only override was removed before the
  successful 18-case command.
- The first version of the test incorrectly required the warning to be topmost even where the staged approval card
  overlapped it. All four cases failed in run `53db3f32-93ef-4dad-9a5e-d7ba29b8bf4c`. The corrected oracle compares the
  warning and refusal directly. This failed command is retained and supplies no passing evidence.
- `cargo build` and `dx build --package farhelm-ui --platform web --release`: passed, preparing the real application
  used by the browser runs.
- `python -B scripts/check-test-sleeps.py` with the isolated pinned parser environment: 322 delays inspected, none
  missing a rationale. Changed Markdown passed dprint; changelog format passed.
- `python3 scripts/check-agents-md.py`: zero errors and two existing citation warnings outside this outcome.

The successful browser commands covered the final product and test patch on base `ec74803a`. They and the builds are
reused individually after rebasing onto `660fb5b5`: the full upstream diff contained only queue state and another plan's
report, and this patch's executable inputs were unchanged. The later ledger PR link is bookkeeping only.

Rust checks and the JavaScript unit harness were skipped because no Rust or JavaScript behavior changed. The CSS paint
contract needs browser integration evidence. The unrelated full browser suite and native desktop checks were skipped;
the selected Chromium and WebKit group covers this layer interaction. The repository's dprint configuration has no CSS
or TypeScript plugin, so an initial attempt to format those files returned “no files found”; their formatting was
inspected manually.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reported no findings. It rechecked the corrected
paint-order probe and acknowledged the test issue missed in its first pass. The documentation pass, resume check and
commit/PR wording cold read are complete.

### Landing

Landed as built. When a terminal link click was refused (the link was covered by another Farhelm surface), the refusal
notice could cover the warning in a session's Delete confirmation while its confirming button stayed usable, and the
Delete shortcut could open that question without the pointer leaving the refused link. As you decided, the warning may
not be covered: while a Delete, Restart or Replace confirmation is open, the refusal drops to the ordinary layer below
it, whichever of the two opened first. The new browser tests drive Delete; Restart and Replace use the same panel and
the same style rule, and were checked in the source rather than driven.

- #2740: keep a link refusal below an open session confirmation.

#### What landed since the stack was based, and whether it interacted

The plan was built and tested before the Steel foundation landed. Steel restyled the confirmations' buttons and changed
their text to Inter, which changes the sizes the plan's new tests depend on (they check that the warning and the refusal
really overlap before comparing which is on top). The reviewer checked that Steel did not change the confirmations'
markup or any of the layering rules this plan adjusts. This round ran the plan's spec file on both engines on top of
Steel, and all of it passed, including the new tests (below). A fresh-context reviewer and I read all four plans
together and against what main gained, and found nothing else that interacts.

#### Fixes made at landing

None.

#### Checks

This round landed four plans together, in this order: no overwrites during the Mac installer's command link placement
(installer-link-noreplace), folder identity for Restart with's automatic trust (restart-with-trust-identity), a link
refusal kept below an open session confirmation (link-refusal-confirm-layer) and clearing a stale Resume target when a
Pi or OMP reporter cannot be prepared (named-restart-reporter-clear). All four were built before the Steel foundation
landed earlier the same day. This round ran the following on all four stacked together on main with the Steel
foundation. The review queue's index and the triage record conflicted mechanically between these plans and entries main
had added; the resolutions keep main's entries, apply each plan's own update to its triage entry, and remove from the
index exactly the five findings these plans address (two for the installer plan, one for each of the others).

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason, `dprint check`
  across the repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two
  warnings, both about citations main already had), the workspace debug build and the UI's release web build: all
  passed.
- The tests of the UI's own JavaScript and stylesheet (`node --test` in `crates/farhelm-ui/js-tests`), run
  `ff182543-0d7a-46fb-a5f8-3b3c6214ad44`: 358 passed.
- The terminal link-click browser spec, `npx playwright test tests/terminal-link-clicks.spec.ts` in `e2e/`, on Chromium
  and WebKit, three workers each with its own helm and supervisors, no retries, run
  `eff1253f-b467-4151-bf3b-9f5081e7c1b3`: all 34 passed, including the new tests that keep a Delete confirmation's
  warning above a link refusal in both opening orders, now with the Steel restyle's buttons and fonts.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, four tests at a time, no retries, run `8433a77d-dde8-4a20-95c7-b1bd78c1f791`: 4455 passed, 40 skipped by design
  (helper entries that only run as child processes of other tests, and tests that need a real signed-in Claude or
  Codex), and 1 failed: the terminal backpressure test "shallow pause resumes without reset or replay", a known flake
  with three entries in the flake log (the latest records it failing 2 of 3 times when run alone). Run alone on the same
  code three times, it passed each time (runs `a3b3ea0f-39c8-45ee-be09-e64b81ed92a3`,
  `76989247-159f-4240-9c5b-325259d1109c` and `b525f8e0-98e9-41b4-a217-3984ca31bf47`). No plan in this round touches the
  terminal's flow control. The deflake sweep skips this test on behalf of a TODO.md Deflake entry that does not exist;
  either add the entry or drop the skip.
- The Mac installer fixture, `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` after
  `cargo build -p farhelm` (it runs the installer against fake releases on Linux, standing in for macOS), run
  `a4f7acf6-7f4f-490e-b15f-e76c5143a4cf`: all 603 checks passed.
- Reused: none. Skipped: the full browser suite (only one plan changes the UI, and its spec file ran on both engines),
  the desktop app checks (no desktop code changed), real Pi, OMP, Claude and Codex runs (the reports say what their
  tests stand in for), and a real Mac installation (not runnable here).

The report's checks were this spec file before Steel; this round ran it again on top of Steel. Nothing in the executor's
report became untrue at landing.
