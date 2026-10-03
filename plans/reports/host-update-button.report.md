## What this was about

An older remote host showed “old version” or “needs update”, but starting its update required opening the host menu. The
agreed change makes that status actionable: an outlined ↑ update button, amber when updating is optional and red when
the host must be updated to connect.

## Things you should know

The button appears only for older SSH hosts whose existing Update action is available. It replaces the visible status
words, preserves the status dot and the words announced to screen readers, and has its own accessible name. Hover names
both builds, both protocols when incompatible, and whether updating is required. Clicking or pressing Enter starts the
same update as the menu, without another confirmation. Progress replaces the button, and keyboard focus moves to that
host's menu toggle so it does not fall back to the page.

Current, newer, local and busy hosts keep their existing presentation. The menu still offers Update as before. The specs
and host-management guide describe the button, the changelog has an Added fragment, and the completed TODO entry is
removed.

The implementation follows the agreed color distinction: both buttons have the same visible text and arrow. Required
versus optional is conveyed by color, the hover explanation and the accessible status. The hover explanation is not a
visible keyboard-focus or touch tooltip.

## Open questions and possible follow-ups

No implementation decision is needed to finish this plan. A future accessibility change could add a visible non-color
cue for required updates; that would revise the explicit design choice in this plan, so it was not added unilaterally.

The button follows the latest known host state, like the menu. The reviewer suggested it could briefly reappear after an
update completes but before the refreshed host version arrives; no such failure was demonstrated. No extra retained
state was added to suppress that hypothetical window.

## The PRs

- [#1518 — update buttons on outdated host rows](https://github.com/scode/farhelm/pull/1518/changes), one draft PR; not
  marked ready or merged.

## Checks run, reused and skipped

- Rust host-row tests: `cargo nextest run -p farhelm-ui -E 'test(hosts::tests::)'`, through the recorder with pinned
  nextest and tmux, four slots and no retries. Final run `19c92791-0d1d-41be-a7a1-547901489300`: 33 passed. This covers
  eligibility, tooltip escaping, fleet-update filtering and unchanged-row rendering.
- Chromium and WebKit, through the recorder with one worker and no retries: phase table, two passed in
  `1ce065b2-6279-4a78-8b12-4f4d34038cb9`; pointer and keyboard activation/layout/focus plus the existing menu-update
  progress case, six passed in `b7259f02-2661-412e-b277-52dcc83b255e`. Eight selected cases passed across both engines
  with no skips. Runs used an owned Linux container and simulated update execution; no live host was updated.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, Rust formatting, changed-Markdown formatting, diff
  whitespace, the test-sleep source checker and changelog format lint passed. The sleep checker inspected 274 delays and
  found no missing rationale. CLI and release web builds passed; the docs-site build passed with every internal link
  valid.
- Earlier browser attempts are retained. `f55b4a41-4178-4db2-b881-bcb7085906ed` and
  `c3d9df02-6aa4-4166-ba57-dcdf9e4f2b95` could not launch browsers because of the private container's cache path and
  missing system libraries. In `17dbb3f4-4554-44f1-ba79-7b33a76b2dbc`, three Chromium cases passed, the new phase-table
  premise used the wrong rendered setup token, and WebKit still lacked a compatibility library. The selector and
  container were corrected before the successful runs above. These were setup and same-session test-authoring failures,
  not established latent flakes.
- Evidence from the tested tree at `5cce47de` remains applicable to pushed commit `b211d336`: the rebase onto `830f55c2`
  only brought in a future Mac-update plan and its TODO/index entries. No runtime source or changed test differed; TODO
  formatting was checked after the rebase. The website content also stayed identical after its successful build.
- Full workspace/browser suites, desktop-shell runtime checks and release/installer gates were skipped: this change
  reuses the existing request path and changes only shared host-row presentation. The focused tests cover its concrete
  behavior and both browser engine families; no desktop-shell or installer code changed. Hosted CI was not requested,
  and the documentation site was not deployed.

## Review gate outcome

A fresh-context Opus 5.5 reviewer at high effort performed the required adversarial review, including the full
test-authoring checklist. Its eligibility and request-path assessment found no blocking defect. Its focus finding and
concrete test/documentation findings were corrected and covered by the final checks. The deliberate color treatment and
the speculative post-update window are disclosed above. A proposed enum for two fixed presentation tokens was declined
as unnecessary indirection; the explicit phase cases and their edge tests remain small.

Two fresh GPT-6.1 Sol medium cold readers checked the title. The final title identifies the host-row button feature; the
PR body is empty because the diff is self-explanatory. A separate documentation pass covered every touched file.
