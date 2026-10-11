## What this was about

File-path tooltips in terminal output flickered between checking and their result while the pointer stayed still. The
maintainer suspected directories; reproduction showed it affected downloadable files too. Live output redrew the hovered
row and restarted the lookup, for both plain paths and file hyperlinks emitted by a program.

The agreed behavior is one check for a resting pointer over the same text, file target and terminal cells. Pending
checks and answered tooltips survive redraws. Leaving, changing the text/target, or moving the path to different cells
ends that hover; a later hover checks afresh. Terminal redraw scheduling stays unchanged, and results are not cached
across hovers.

## Things you should know

Reproduction failed all eight plain/hyperlink × directory/file cases across Chromium and WebKit, with repeated lookups
and transient tooltip resets. The first implementation kept the result but lost a plain file's hand cursor; the browser
test caught that timing defect and it was corrected. Source review also caught a changed hyperlink label incorrectly
retaining evidence when its file target and cells stayed the same; a focused correction and regression cover that
boundary.

Earlier failed runs remain retained as separate observations, not replaced by later passes. They are the investigated
product bug and same-session implementation/fixture failures, not latent-flake claims. The working log is
`farhelm-plan-flickering-path-hover-log.md` beside the checkouts.

## Open questions and possible follow-ups

None for this plan.

## The PRs

- [#1879 — keep terminal path hovers steady through redraws](https://github.com/scode/farhelm/pull/1879/changes), draft;
  pushed commit `748608c9be9f5d61a6270ff673215500100a8a58`.

## Checks run, reused and skipped

Run: `node --test` for the complete UI asset JavaScript harness, 238 passed, run `1d64c45a-3666-41af-95c7-e75446ee8c81`.
The final browser redraw matrix and changed hyperlink-label cancellation passed ten cases across both engines, run
`fc573883-c245-4716-bfb7-1911a67c7fcd`, selected with `playwright test terminal-files.spec.ts` and a grep for those
cases. These validate one lookup and no transient tooltip hide/restart during real redraws, cursor restoration, fresh
checks after departure, and cancellation when only a displayed label changes.

Reused: existing file download/refusal/cancel and hyperlink selection/URL handoff scenarios, 16 passed across both
engines, run `6149a249-3b9b-4a3f-ab5f-11d50ba35c59`, covering revision `99763d6a`. The final correction adds
displayed-text identity only to redraw retention; those unchanged-label, departure and download branches stay the same,
while the final ten-case run validates the new identity comparison. Matching CLI/helm/supervisor binaries were reused
after comparing their source and fixtures from `baa90a5d` through the plan base; those inputs have no changes.

The final JavaScript/browser regression runs cover revision `eebacca0`. Careful rebase inspected all intervening changes
through `e9f381a2`: an unrelated TODO/plan addition, its claim, and executor cleanup instructions. A shared TODO
conflict was resolved by keeping the new entry and removing only this plan's covered entry. Every other touched file is
byte-identical to the reviewed and tested revision, so runtime and source-review evidence still applies to the pushed
commit.

The source-only delay checker inspected 287 delays with zero missing reasons. Changed Markdown passes dprint; the fixed
changelog fragment passes the layout check. Failed reproduction `c8bbe2d8-ffc9-4da3-ab50-638ce409ffe2`, fixture run
`917abece-22c7-4ab8-a648-55bc4fdc7924`, and cursor run `281589ea-ed49-416a-a363-b25170ab33ed` remain retained.

Skipped: Rust checks and the full browser battery, because no Rust source changed and the affected hover lifecycle is
covered by injected-clock/fetch tests and selected real browser scenarios. Browser runs use both Chromium and WebKit,
pinned tmux, one worker and zero retries. WebKit supplies automated coverage of the desktop engine family; no native
desktop run was needed for this asset-only hover change. Website/release/installer checks do not inspect these changes
and were not run.

## Review gate's outcome

Implementation and investigation stayed local under no-workhorse mode. The prescribed fresh-context GPT-6.1 Sol reviewer
ran at high effort, without a swarm. Its one material finding about changing hyperlink labels was accepted and fixed;
the same reviewer's correction check found no remaining material issue. Commit/PR wording passed the required
fresh-context cold reader on GPT-6.1 Sol at medium effort. Galaxy evidence is session
`a63f6b2c-45f7-4830-bf84-ea76de63472d` in the standard private state directory. Native actual-model and usage metadata
were unavailable; four review event timestamps were estimated incorrectly and explicitly recorded as an evidence gap.
