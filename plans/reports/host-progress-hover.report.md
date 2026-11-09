# Host progress hover card

## Since the last review

Draft #3272 now resolves the failure that stopped its landing. A short window can make the host list scroll to reveal
its working host. That scroll sometimes arrived after the test entered the host button and intentionally dismissed its
pending hover. The test now reveals the button before a fresh pointer entry. It still checks the visible card, readable
current step, diagnostic preview, and live changes while the card is open.

Review also found a separate product sizing error: the step list counted its padding twice, clipping the full-details
notice. The correction preserves that notice in full. A run whose steps have not arrived yet now uses the same preview
rule. No PR was added or dropped; #3272 remains one draft, at `82d8fb85`.

## What this was about

Following a running setup, update or uninstall used to give different detail in Hosts and the sidebar. The sidebar's bar
gave only a count, and the Hosts hover omitted the connection diagnosis. Both now show the same card with the host,
operation, count, elapsed time, every step and explanation, the current step, and available diagnostics. The sidebar
also names the current step; keyboard focus on its host button opens the card.

## Things you should know

The card stays non-interactive. Only a diagnosis too long to fit beside a readable current step becomes a preview, with
`Full details in Hosts` visible in full. A message that fits stays whole. The host identity opens Hosts; turn on Details
there to read the complete connection diagnosis. The current step follows progress changes while the card remains open.
Ordinary scrolling still dismisses hover help.

The cloud failure was reproduced locally with the browser processes restricted to two CPUs. Event records established
the late reveal-scroll ordering; loaded fonts, a replaced host button, and a missing card source did not explain those
failures. A separate empty computed-style result was consistent with the elapsed clock replacing a clone, but that
particular detached-node state was not recorded. Its assertion now observes one connected, displayed clone. The
established test failure and retained evidence are recorded in FLAKES.md.

## Open questions and possible follow-ups

The landing monitor will repeat the checks on Tensorlake sandboxes, on both engines, as the latest Decision requires.
This executor performed local verification only. No product choice remains unanswered.

## PRs

- [#3272: share host progress details on hover](https://github.com/scode/farhelm/pull/3272/changes) — the existing draft
  covers both surfaces, the specification, changelog fragment, original TODO removal and these corrections.

## Checks run, reused and skipped

Run in this round, on Chromium and WebKit where both engines are named:

- Exact unchanged Chromium overflow case, six repetitions with one worker, two allowed CPUs and zero retries: 5 passed,
  1 failed (`cebeb182-cf4e-4038-b139-1b3cd476bcf3`). The same case with bounded diagnostic event logging, ten
  repetitions under the same restriction: 6 passed, 4 failed (`27210f18-b266-44b8-bbd2-f85f1e85dbce`). These reproduced
  the landing's missing/hidden card and empty style symptoms. Both failed runs and their traces remain retained. They
  tested `aeb2b4d5`, with only diagnostic logging added in the second run.
- After correcting pointer preparation and the style observation, ten repetitions per engine under that same CPU and
  worker restriction passed: 20 observations (`420fd204-7912-479b-897f-8c2d0f6c562c`). This preceded the stronger
  full-notice text check and sizing fix, so it is evidence for the hover correction, not for the final notice geometry.
  These three repetition commands used the recorder's generic mode and Playwright's explicit repeat/project options.
- The stronger full-notice glyph assertion failed before the sizing correction on both engines: 0 passed, 2 failed
  (`67ac982e-d699-4e5d-b622-e2da369f048a`). In both cases the current step was readable and the notice was clipped. The
  failed run remains retained; the CSS correction resolved this product bug.
- Final selected `host-updates`, `provisioning` and `tooltip` browser cases: 18 passed, nine per engine, with no skips,
  failures or retries (`7bf1e625-dc89-4b44-8021-97fffb47bb00`). This used the recorder's controlled Playwright mode,
  three workers and the rebuilt web bundle. It covers both surfaces and keyboard entry, three-step and zero-step long
  diagnostics, complete notice/current-step glyphs, whole-text restoration, live diagnosis changes, fitting one-step
  messages, inline progress in both themes, current-step following and the existing session card.
- `node --test crates/farhelm-ui/js-tests/tooltip.test.js`: 23 passed, none skipped
  (`9cf4a257-2f2b-4a51-b7a8-8bd8c14e184c`). This and the final browser command captured the same complete source
  fingerprint at `aeb2b4d5` plus the final four-file correction. Folding that correction into `82d8fb85` changed no
  inputs.
- `cargo build -p farhelm` passed on the rebased draft. The pinned
  `dx build --package farhelm-ui --platform web --release` passed again after the final CSS/JavaScript correction. These
  were builds, not recorded runtime tests. The CLI's inputs did not change during this round.
- Targeted Markdown formatting, JavaScript syntax, whitespace, changelog formatting and the isolated test-sleep checker
  passed; 429 delay calls had a rationale. No runtime `SKIPPED` messages occurred in this round's retained output.

Reused from the previous execution: the four focused Rust card/summary/attention tests at `cb163bb8` with its recorded
working diff (`1713d0a7-cb87-4bb9-8d1c-5f31adb2c34d`, four passed, 918 selected out). Their Rust inputs remain unchanged
through `37fe8462` and `82d8fb85`; this round changed no Rust. The earlier 58-case host-update/tooltip pass at
`e91db5b3` (`fdb1ab98-3da1-4462-9eaf-84fc30b429ce`) remains evidence for unaffected interactions, while the changed
sizing and pointer boundaries were checked again above. The unchanged Tensorlake hover cases passed in the prior
selected run `1b9326ce-fe65-44d2-bb29-c327fd0bcb97`; that command had seven passes and one unrelated WebKit
text-readiness failure, corrected and passed narrowly in `014556d8-71fd-443b-98b7-e76f8538f4f9`. It remains a failed
command, not a retroactive pass. Earlier blocked-round overflow failures (`614128e5` and `a06eeb91`) also remain
retained; the current preview and complete-glyph cases replace their missing product coverage.

The browser and focused Rust child selections were as follows. Each ran through the recorder in the mode and with the
run ID described above; the repetition commands' private output destinations are redacted.

```sh
# cebeb182: unchanged Chromium, six repetitions
taskset -c 0,1 npx playwright test --project=chromium-host-updates \
  -g 'host progress previews only diagnostics that cannot fit beside its current step' \
  --repeat-each=6 --workers=1 --retries=0 --output='<private-output>'
# 27210f18: the same selection with diagnostic logging, ten repetitions
taskset -c 0,1 npx playwright test --project=chromium-host-updates \
  -g 'host progress previews only diagnostics that cannot fit beside its current step' \
  --repeat-each=10 --workers=1 --retries=0 --output='<private-output>'
# 420fd204: pointer/style correction, ten repetitions per engine
taskset -c 0,1 npx playwright test --project=chromium-host-updates --project=webkit-host-updates \
  -g 'host progress previews only diagnostics that cannot fit beside its current step' \
  --repeat-each=10 --workers=1 --retries=0 --output='<private-output>'
# 67ac982e: complete-notice regression before the sizing correction
npx playwright test 'host-updates\.spec\.ts' \
  -g 'host progress previews only diagnostics that cannot fit beside its current step'
# 7bf1e625: final controlled selection, both engines
npx playwright test 'host-updates\.spec\.ts|provisioning\.spec\.ts|tooltip\.spec\.ts' \
  -g 'host progress (cards agree|previews)|a one-step host card|an open Update card follows|single Update shows inline|a card follows only|a session.s title and open area'
# 1713d0a7: reused focused Rust tests
cargo nextest run -p farhelm-ui --lib \
  -E 'test(folded_progress_card_keeps_peer_diagnostics_and_live_connection) | test(decoded_progress_keeps_name_fallbacks_and_raw_summary_diagnostics) | test(update_progress_summary_counts_steps_and_requires_the_tracked_live_run) | test(progress_wins_and_display_order_keeps_registry_order_within_groups)'
# fdb1ab98: earlier full host-update/tooltip specs, both engines
npx playwright test 'host-updates\.spec\.ts|tooltip\.spec\.ts'
# 1b9326ce: earlier selected provisioning/Tensorlake cases, both engines
npx playwright test 'provisioning\.spec\.ts|tensorlake-hosts\.spec\.ts' \
  -g 'an open Update card follows|single Update shows inline progress|adding a Tensorlake host confirms billing'
# 014556d8: exact WebKit readiness correction
npx playwright test --project=webkit-provisioning \
  -g 'single Update shows inline progress without opening its row in dark' --retries=0
```

The careful rebase read every incoming main change. The landed approval-card layout and its styles are separate from
host progress; there was no feature interdiff. Main did not move again before the final push. The full workspace,
remaining browser suite, desktop smoke, installer and real cloud provisioning were skipped: these corrections concern UI
sizing and test preparation, and the focused checks cover their integration risks. Hosted CI is manual-only and was not
dispatched.

## Review gate's outcome

The required fresh, single gpt-6.1-sol/high reviewer reviewed the full PR and found the padding/minimum-height mismatch.
It was reproduced, corrected and verified by that same reviewer, which cleared the final correction with no outstanding
finding. Own-model scope reassessment confirmed the reveal boundary and empty-step correction fulfill the existing
Decisions without a new mechanism. Resume and commit/PR wording checks passed. Every file touched in this round received
a separate documentation pass. No rejected finding or omitted required behavior remains.
