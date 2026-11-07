# Since the last review

The existing draft, #3129, was corrected and rebased; no PR was added or dropped. The container-link failure that
returned this plan is fixed, together with a second container-hover regression found by the required reviewer.

# What this was about

A terminal program can insert its own line break in a printed file path. Farhelm previously offered the two fragments as
separate paths, so downloading either failed. The maintainer chose to join one qualifying pair of rows, accepting some
false matches between unrelated lines for the improvement in usability. Either half now discloses and downloads the
whole path. Ordinary terminal wrapping keeps its existing behavior.

# Things you should know

The returned container test failed because recognition joined an ordinary word on the preceding history row to an
absolute path on the screen. The preceding word was prepended to the valid absolute target, so the host correctly
rejected an invented relative target. A first hover on the valid path also failed; hovering the rejected path first was
incidental. The correction requires the first piece to already qualify as a path under the ordinary rules, as the plan
specified. Both specifications now state that boundary explicitly. A plain word, version or URL cannot become a path
head merely because the following line contains a slash.

The reviewer separately found that an unrelated neighboring redraw could retire a container file hover and start
asynchronous host admission again. That could repeatedly remove download permission during live output. The correction
keeps the existing hover when only neighboring rows change and fresh recognition still returns the same path and cell
range. An edit that creates a continuation still withdraws the old permission. Changes within the linked rows keep the
existing recheck behavior, including hyperlink precedence. Every download independently checks recognition again.

The deliberate exception remains one isolated hard break, a four-column right-margin allowance, and optional space
padding/indentation. It does not reconstruct every possible split: a bare-word first piece is excluded. Relative paths
still resolve against the session's directory. Host admission and file resolution were not relaxed. The original
container browser test and fixture were preserved. The addressed TODO entry is removed and the fixed changelog fragment
is included.

# Open questions and possible follow-ups

No required decision remains. Unrelated path-like lines can still satisfy the intentionally permissive join rule; that
is the accepted tradeoff. This work does not add reconstruction across chains of hard breaks or filenames with spaces.

# The PRs

- [#3129: fix: link paths split by terminal programs](https://github.com/scode/farhelm/pull/3129/changes) — the complete
  fix at `e6f32d624cd36eae5e160267deb70348f590f0fc`, still a draft for the landing monitor.

# Checks run, reused and skipped

All runtime attempts used the test recorder. Browser engine claims below refer to Chromium and Playwright WebKit, which
supplies engine-family coverage rather than a native macOS application run.

Run in this round:

- `node --test terminal-files.test.js grid-buffer.test.js` from the UI JS harness: 77 passed, run
  `727dbb84-81de-446a-8f2a-82f8d8f0513e`. This checked the head-eligibility correction, original hard/soft breaks,
  Unicode cells, cached endpoints and history/screen mapping.
- `node --test` from the same directory after both corrections: 528 passed, run `8de87c15-6798-450f-9c24-f24381d3d778`.
  No skipped cases or runtime `SKIPPED` marker; retained source/output and cleanup are complete.
- After rebasing onto main's row-tooltip and background Git cleanup changes, `node --test` passed all 530 cases, run
  `dd72ef6f-2d66-485b-acd9-ed2e764e778c`, with no skips or runtime `SKIPPED` marker.
- `npx playwright test --config grid-client/playwright.config.ts --workers=3 --retries=0 --reporter=line,json` from
  `e2e`: 192 passed, 96 on each engine, no skips, failures or retries, run `2610ad3c-a257-47d6-ba65-bed1b2f5d189`. This
  is the full grid-client suite required by the latest Decision, including the unchanged container test and the new
  neighboring-redraw cases. Generic recording was used because strict browser mode does not accept this configuration;
  the retained JSON supplies the per-engine counts.
- That full run's recorded source is `86ba2fa68c24e318a82ff6139cc70bf05903152b` plus the corrections captured as
  `42ae7c55d4508a5926d05ca6456daf97e4c59e58`; the retained complete source diff matches that revision exactly. The full
  result is reused after rebasing onto `ea1919b7` and folding into `e6f32d62`: the grid/recognizer implementation and
  assertions are unchanged. The new sidebar-tooltip behavior needs explicit row opt-ins absent from the static fixture,
  and its notch correctly marks itself as a disclosure cover. A fresh integrated selection passed 16 cases on both
  engines, run `72afd2b5-0837-40b8-b7fe-e2731381cb48`, using
  `npx playwright test --config grid-client/playwright.config.ts -g 'container|a printed path hovers|a redraw that changes|a click whose file hover is covered' --workers=3 --retries=0 --reporter=line`.
- Focused Chromium container tests after the hover correction: four passed, run `ccacebea-c35c-401d-8d79-bc3a4bdc0903`,
  with
  `npx playwright test --config grid-client/playwright.config.ts --project chromium -g 'a container file hover survives unrelated|a new neighboring continuation|a container session offers only' --workers=1 --retries=0 --reporter=line`.
- `dprint check` on the changed recognized Markdown and stack-browser spec,
  `python3 releasing/check-changelog.py format`, and the isolated interpreter's `python -B scripts/check-test-sleeps.py`
  passed. The delay check inspected 420 calls with zero missing rationales. Asset JS, JS unit files and grid-client
  specs are excluded by dprint; no formatter pass is claimed for those files.

`cargo build -p farhelm -p farhelm-fixtures` and `dx build --package farhelm-ui --platform web --release` passed on the
rebased sources. Against those matching builds, the strict recorded selection
`npx playwright test 'terminal-files\.spec\.ts|terminal-links\.spec\.ts'` passed 46 cases, 23 per engine, run
`12c1d698-7466-4e5b-ad3d-e6fab0b8689c`. It used three workers with one owned stack each, zero retries, the pinned tmux
and ordinary host-terminal mode. All three stack cleanups completed. Both halves of the hard-broken path disclosed the
whole target and downloaded the expected bytes on both engines. This also covers local and SSH file downloads, refusals,
redraws, hyperlink precedence and link gestures.

Retained failures established the causes before correction. The exact original container test failed in run
`dee4fda5-d8ec-4d97-91e7-75531c5139e4`; request logging in `8a3cacc1-3aab-4547-93b3-f9b1dae75888` exposed the invented
target. A first-valid-hover diagnostic also failed (`50424833-67a0-4899-a483-454a3700bd5b`). The new pure recognition
regression failed before its fix (`a8f820b2-f189-46b8-ba24-4d535aba0662`). Both ordinary and cut-path redraw cases
failed before the second correction (`d326a935-e96b-43e6-a637-a17858182c24`), issuing four stat requests instead of
preserving the original two. Later passes do not replace these observations; their private records and browser artifacts
remain.

Reused: the original old-terminal link-click run, `f30819bc-f7a8-46a4-8459-348d309c2b52`, passed 36 cases, 18 per
engine, using `FARHELM_E2E_SUITE_TERMINALS=tmux` and `npx playwright test 'terminal-link-clicks\.spec\.ts'` with the
pinned tmux. Its retained source diff matches commit `6e414d09` exactly. Reuse is limited to the unchanged old-terminal
gesture, disclosure and transfer behavior. Current recognition is checked through both line adapters by the fresh JS
run; the new recovery hook belongs only to grid terminals. This does not claim a fresh old-terminal run of the amended
revision.

Skipped: broad Rust, desktop, installer, provisioning and release suites. The plan changes client recognition and grid
hover handling; it changes no Rust behavior, host path resolution or installation behavior. No live installation was
touched. No website content changed, so no preview was needed.

# Review gate's outcome

The required fresh-context native `gpt-6.1-sol` reviewer at high effort found the neighboring-redraw regression. It was
reproduced and corrected. The same reviewer's complete-diff addendum found it resolved with no new findings. Fresh
own-model scope checks supported the narrow head criterion and evidence-only recovery. The unchanged commit/PR wording
passed its cold read.
