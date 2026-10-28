## Since the last review

All four existing draft PRs were revised after the landing run exposed browser and stylesheet failures. None was added
or dropped. The layout and keyboard decisions remain: native Tab follows the visual reading order, the YOLO question
stays inline, and Trash keeps its pinned header without a footer.

The returned failures were diagnosed individually. The 34 launcher timeouts happened while an open directory-suggestions
popup covered the next control, before the consent or held-request behavior began. Their setup now dismisses that popup
with Escape and verifies the typed path stays unchanged before continuing the original checks. Dedicated popup
selection, blur, Tab and typed Enter tests remain.

Fresh code review also found a separate product bug the earlier cases missed: at short phone heights just above the
compact-layout breakpoint, four stacked launcher actions squeezed the YOLO answer region until Cancel was clipped. A
narrow real-question reproduction failed on both engines at 481 and 500 pixels high. The footer now wraps its existing
actions through that intermediate phone range, retaining normal-sized buttons above the smallest-height layout and
preserving the existing notice scrolling and focus handling.

The required whole browser run completed with 2,764 passes, four failures and 14 skips. All original returned failures
passed. The four remaining failures came from a build-help response interceptor, the losing window's new-tab retry
fixture on both engines, and a host-menu scenario exhausting its overall time budget. Those fixtures were corrected
using existing facilities; the affected cases and shared-helper callers then passed 28 checks, followed by two checks of
a final cleanup correction. The failed whole run remains separate evidence; no clean whole-suite pass is claimed.

## What this was about

Settings, session setup, confirmations and menus had inconsistent layouts. This plan applies the shared Steel and Steel
Light design: section icons and headings in Settings, consistent menu descriptions and destructive grouping, shared
dialog headers and answers, and labeled launcher fields with actions pinned below them. Existing host dialogs already
using that structure were reused.

The launcher keeps its operations and fields. Its footer shows what will launch where beside a compact Launch, Replace
or Restart button. Long forms scroll above it; long warnings and the inline YOLO question have their own bounded
scrolling area. Initial focus, Escape, focus return, native choice controls, Enter-to-launch and request guards retain
their existing owners.

## Things you should know

The footer's complete target now uses Farhelm's shared hover help instead of the browser's native tooltip. Updated tests
check the visible target beside the compact action and its accessible name. The affected documentation callouts point at
that separate target summary.

The remaining returned browser failures were test updates: the new exact Trash name, primary Settings heading color,
compact Browse label, the named-conversation field's actual label-and-input grid, and warning elements that exist only
when there is something to warn about. The live Replace warning still has its full text and accessible association with
the main action. All original request, archive, preference and launch assertions were retained.

The two WebKit host-switch failures followed a timed-out directory-popup test that failed to restore its host
preference. Fresh unchanged switch cases passed after fixing that setup. The file-drop case had attempted the drop after
the tab mounted but before its terminal attachment was ready; it now waits on the existing tab-specific readiness check,
requires the real upload response, and retains path, file-content and agent-terminal isolation assertions. These
observations do not establish a new latent flake.

The later build-help failure came from removing interception while a response was still being handled. Its fixture now
uses the existing callback drain, including cleanup after an assertion failure. The takeover test deliberately hides the
notice from a losing window; its 30-second retry also prevented a new tab from retrying a legitimate temporary refusal
before a 20-second assertion. It now withholds only future agent connections and lets the tab retry normally, retaining
the unaware-window premise and proof that the winner still accepts input. The matching recorded flake and sweep
exclusion are resolved. The host-menu test had completed movement, dismissal and reopening before its 60-second case
budget expired; only that scenario's overall budget increased, with individual readiness and geometry checks unchanged.
No production terminal or host-menu behavior changed for these three corrections.

Tab follows header Close, fields in their existing relative order, then Cancel-first/main-action-last footer buttons, as
approved. Trash keeps its totals, progress, Delete all and Close in its header. Long target summaries may truncate
visually, with their full content in shared hover help and the action's accessible name.

Appshots use the real staged web UI in both palettes. Earlier native tunnel-consent review used matching markup/CSS
preview and source, with separate native compilation/controller tests; images do not prove native callbacks or AppKit
behavior.

## Open questions and possible follow-ups

No required decision remains. The existing TODO entries revisiting keyboard order and Trash remain available for later
review. The broad sentence-case pass and removal of the umbrella Steel TODO entry belong to the next plan.

Rename still keeps a complete pasted name containing a line break for correction after rejecting it; its existing
one-row display can look cramped. This plan preserves that field sizing. Existing per-host Trash spacing and unrelated
label capitalization remain outside this change.

On a short phone, the bounded YOLO explanation and its answers may need scrolling or Tab rather than fitting together. A
persistent scroll cue remains a possible shared improvement; native scrollbar visibility depends on the platform. The
second footer line can also truncate late choices, including a permission word after a long model name, and has no
full-text hover help. The full choices remain editable in the form and any required YOLO confirmation still applies. The
target's shared help uses the existing single prose font, including its path; markup-aware path styling is another
optional shared improvement.

## The PRs

- [Settings navigation and headings, #2756](https://github.com/scode/farhelm/pull/2756/changes).
- [Command-menu descriptions and destructive grouping, #2757](https://github.com/scode/farhelm/pull/2757/changes).
- [Dialog layouts and confirmation answers, #2761](https://github.com/scode/farhelm/pull/2761/changes).
- [Session launcher rows and pinned actions, #2762](https://github.com/scode/farhelm/pull/2762/changes).

All remain drafts; the landing monitor owns merging them.

## Checks run, reused and skipped

Runtime commands used the recorder and pinned tmux where needed. Browser behavior ran on Chromium and WebKit with three
workers, zero retries; appshots used the normal single-worker staged fleet. Counts from overlapping selections are not
added together.

- `npx playwright test`: run `1c66d7f4-79b2-41bf-b6f1-225f1fc10cf6` completed in 109 minutes with 2,764 passes, three
  assertion failures, one timeout and 14 skips. Chromium had 1,387 passes, two failures and two skips; WebKit had 1,377
  passes, one failure, one timeout and 12 skips. All cases started. The recorder retained complete source identity,
  output with EOF and clean cleanup, but its case-summary collector refused the 19,452,814-byte JSON at its 16 MiB
  limit. A separate audit reused the same policy/count validator with an explicit 32 MiB bound and reconciled the raw
  reports. Original report SHA256: `8fceec953c29640c7c1073392fe6c0343bc7085ec3d1d9b031db042cc6acb4a1`. The original
  manifest remains incomplete and exit 1; the audit does not turn it into a pass. The 14 skips supply no coverage.
- `npx playwright test <affected specs> -g <affected cases>`: the original returned-case selection passed 66 checks
  (`8793d734-c986-4fb5-b13a-cbc2eb30a5ad`). A whole-run attempt stopped for visual corrections exposed two setup
  regressions from an overbroad folder helper (`04755902-4d46-4edc-8312-6860766f33e2`, interrupted); its use was
  narrowed to ordinary retries. All destination-authority consumers and footer geometry then passed 28 checks
  (`8e420231-fd4e-4b97-b272-4e16cc2c32b3`).
- The four later full-run failures were reproduced with three exact titles on both engines: five passed and build-help
  failed once (`b215889b-1d19-49b0-a522-9b287c8b42c2`). After the fixture corrections, all eleven build-skew helper
  callers, both adjacent takeover cases and the late host-menu case passed 28 checks
  (`f7e0bf2f-a13f-48c0-bc16-b7beb93179bc`). A final review found another caller using unsafe route cleanup; its
  corrected exact case passed both engines (`8a67a3b8-5454-424c-82ce-d39874864984`). These are later observations, not a
  replacement for the failed full run.
- The short-phone reproduction failed all four 481/500-height cases (`70b02464-5ef2-4376-a520-7d94b7620f90`). After the
  action-wrap fix, the 18-case boundary/geometry selection had 17 passes and one later name-edit focus-premise failure
  (`fa5662e1-19f7-4832-9cb3-5c324d1a02e2`); every answer-visibility check passed at heights 480, 481, 500, 600, 601
  and 844. Waiting for the existing initial focus handoff before the ordinary edit corrected that test; its original
  case passed both engines (`a1d0b433-4b08-4005-925f-627cdd8dbe5e`).
- All UI JavaScript tests, `node --test`, passed 358/358 (`8885b05e-de95-4146-97c8-4cb4ea5b1a51`). The final web build
  passed (`dad25275-1fe7-4500-8eb7-4eac8073b80a`). These cover product source fingerprint
  `a214363af51126e765745cd7ddf4c419e719742dd4dc707e8b72a187659393c7`, also used by the whole browser run. Subsequent
  changes affect browser fixtures and their records only; product bytes are unchanged.
- Normal `scripts/docs-screenshots.sh --only visual-review --no-build` staging and captures passed five cases and
  produced 72 launcher PNGs (`d7af20ab-256d-4a9d-9442-4f371c11d002`). The two changed docs annotations passed staging
  plus both shots (`08d8c628-76c1-40df-a442-c51a3197ef57`). Earlier template-page capture exposed an obsolete effort
  selector; its corrected exact shot passed (`f6fe643d-6780-4697-9f3f-27c478b5a48b`). The images were inspected locally
  and not published.
- UI all-target Clippy, applicable Rust and Markdown formatting, TypeScript parsing and the test-delay checker passed.
  The latter inspected 322 delays with none missing a rationale. Native/backend build
  `1ac54cb7-5ea8-4d75-b88c-7c8381473e7f` is reused because this return changes no backend source.

Earlier UI library 805-test evidence (`8177fa16-a156-4da9-b519-d9fa67857d00`), six desktop-feature tunnel tests
(`728ebe9f-0b39-42ea-a442-4f1df70b34fd`), 52 pure launcher tests (`780e0381-c6f0-481b-95a7-99d9d4bcbc72`) and
Settings/menu/confirmation visual gates are reused for their unchanged logic and surfaces. They covered the previously
published four-unit stack ending at `618e6d0013b90b1c3909c7b61d1cdd5918f26ea9`. Its rebase onto claim main had empty
per-unit interdiffs; intervening main changes were instructions and queue/review records. This return's new CSS, tooltip
and fixture risks have the fresh checks above. Retained failed runs remain separate; no same-session authoring failure
was recorded as a latent flake.

The final fixture selection covered source `497acae5f91c41aaa81c26f3d8f4e388c658e693ed20d4d29b8ca51d69582cfa`; the final
header-cleanup run covered `5a8d854e5d0e2f62de017dd9f14bc298e238e34e363fd676a1f5b04e9003bc59`. Only that cleanup and
evidence bookkeeping changed between them. The complete reviewed stack was compared byte-for-byte before and after
folding the corrections into the original units.

Verified pushed heads: Settings `1bde3e179e911c8e8faa4b2fc3832998929e1177`, menus
`52e7051ab9c0b5292f15860b01eeab48ee97251d`, confirmations `b1040dee0816346949d1f3bc5fedd72358730cf7`, launcher
`430eadd860b4950883d47ffd3403563f11719993`. The working copy is clean and GitHub shows the same four draft heads and
linear bases.

Another full browser run was skipped after the final fixture-only corrections: the affected callers and boundaries had
focused coverage, and no new product or interaction gap was identified. Fresh scope review checked this choice against
the actual returned Decision. Unrelated Rust, installer, release and CI batteries were skipped because this remains a
presentation change with bounded fixture repairs. No macOS/AppKit runtime capture was performed and no live installation
was modified.

## Review gate outcome

Each original unit passed its fresh single gpt-6.1-sol/high code review. The launcher reviewer followed its accepted
corrections through the final route-cleanup and evidence-record changes; no required finding remains. Fresh own-model
scope reviews supported the local CSS and existing fixture facilities, without a new controller or production terminal
change.

The visible units retain their earlier claude-opus-5-5/high multimodal gates. This return's launcher gate inspected 15
actual images, then the same reviewer inspected eight corrected/new views and reported CLEAR. Actual successful image
delivery was audited for every requested image, with no truncated text reads. It confirmed tooltip anchoring, full
long-target help on a phone, readable short-phone answers and corrected docs arrows/copy. Optional findings declined
with reasons remain in the working log and the follow-ups above. No native callback or AppKit behavior is inferred from
web images.
