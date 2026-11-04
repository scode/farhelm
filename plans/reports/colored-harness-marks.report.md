## What this was about

The session list showed every agent mark in the same grey, making agents harder to identify at a glance. The maintainer
chose distinct colors where the owner's terms allow, preferring official hues and keeping the shapes unchanged. Seven
agent marks now have identity colors in both themes. Codex, Goose and the ordinary Terminal mark keep their existing
grey; every selected mark turns white.

## Things you should know

The source and terms re-check was made on 2026-11-03. These are the final dark and light colors:

| Agent    | Dark / light      | Source                                                                         |
| -------- | ----------------- | ------------------------------------------------------------------------------ |
| Claude   | #d97757 / #d25e39 | Terracotta from Anthropic's official palette, on Farhelm's original starburst. |
| Muse     | #456bff / #456bff | Farhelm blue; the community mark's source blue is unofficial.                  |
| Cursor   | #2fbfaf / #249084 | Farhelm teal; the checked official artwork is neutral.                         |
| Pi       | #4d9abf / #3e89ad | Blue from the press kit's primary logo.                                        |
| OMP      | #9b4dff / #9b4dff | The middle purple stop of the official favicon's gradient.                     |
| Grok     | #d8b839 / #987f1e | Farhelm gold on its original G; inspected official artwork is neutral.         |
| OpenCode | #83b94b / #638d37 | Farhelm yellow-green; official artwork is white and grey.                      |

The full source links and terms basis are in [the provenance record](../../docs/harness-marks.md). Pi's current press
kit now has a three-color primary logo, unlike the neutral favicon recorded during planning. The existing badge takes
one published hue, blue, under the plan's official-color fallback. It does not reproduce the primary logo's full
arrangement. OMP purple separates from that blue better than its cyan stop. Light-theme shades lower lightness where
necessary for contrast; the official hues are preserved.

The re-check also found current Grok brand guidelines and corrected the old claim that no kit existed. The kit ZIP
returned HTTP 403, so the color decision uses the direct favicon and official repository artwork. Farhelm's existing G
stays unchanged. Stencil's current OMP terms have broader scope than the old notes implied; the record now distinguishes
the MIT source and official monochrome implementation from an express trademark license. Cursor's referential-use
judgment remains the recorded basis; no express recoloring grant was found. No newly found prohibition required dropping
one of the seven colors. OpenAI and LF Projects restrictions still keep Codex and Goose grey, without claiming that grey
is an exact supplied black or white.

All seven colors clear 3:1 on both sidebar and hover surfaces in both themes; the lowest ratio is 3.113:1. Every pair
also meets the plan's CIEDE2000 separation target of 15: the closest is Muse/Pi in light mode, 15.995. The complete pair
table is in the PR description and working log. These measurements do not guarantee that every user can distinguish the
marks at 12px. Gold and yellow-green sit near permission-color families, and Claude terracotta is near warm host/status
colors; shapes and placement still distinguish those roles. Agent color means identity, never activity or permission.

I inspected both themes in Chromium and WebKit with all ten marks visible at their unchanged sidebar size. The seven
hues remain readable, the neutral Goose and Terminal marks remain grey, and selected Codex is white. Muse blue and Pi
blue are the closest pair, but their hue and shape remain distinguishable in these captures. The synthetic rows have no
real terminal behind them, so the capture's terminal-unavailable notice is expected; this checks row presentation, not
terminal attachment. No palette or geometry adjustment was needed after inspection.

## Open questions and possible follow-ups

No unresolved product decision remains. The docs website's session-list screenshots still show the old grey marks until
the next docs screenshot refresh. That published-media refresh was deliberately left outside this plan. The broader
distinction between an unchanged official shape and Farhelm's inherited grey treatment for Codex and Goose remains as
agreed; this change makes no new exact-color compliance claim.

## The PRs

- [#2961: color session-list agent marks](https://github.com/scode/farhelm/pull/2961/changes), draft at
  `a67d4bece15df0a7cfdc0a5acc5c6e8f1e0bb652`.

## Checks run, reused and skipped

Run on Linux:

- The final full asset JavaScript harness passed 508 tests, with no failures or skips: recorder
  `b417be3e-7bc6-4deb-9c23-8e4b40d6d9b1`. Child command: `cd crates/farhelm-ui/js-tests && node --test`. This covers
  token declarations, role routing, inherited neutral marks and the 3:1 floor against both surfaces in both themes. An
  earlier run passed 502 before main added six independent grid-link tests: recorder
  `f99a9ef6-8692-4066-903e-beab7690130b`.
- The focused ordinary browser run passed 10 tests, five per engine, with no failures, skips or flaky outcomes: recorder
  `1f45cee6-e9f6-427d-a3f2-58de47c9122a`. The final test source also passed the same 10 cases with terminal hosts:
  recorder `fdcd2f49-e3a6-4f49-845c-0a2f48955dc4`. Both used Chromium and WebKit, three owned stacks, three workers,
  zero retries and pinned tmux 3.7c. Both recorder reports are complete. The host run explicitly confirmed its terminal
  mode; neither output has a runtime `SKIPPED` marker.
- Browser coverage includes ordinary row metadata, compact marks, sidebar alignment, and the new dark/light cases that
  check every ordinary mark and actually select every row before checking white. The child command was
  `npx playwright test '(terminal|sidebar)\.spec\.ts' --grep 'agent marks keep identity colors|compact rows retain distinct ended and harness glyphs|list renders the session row with title|narrow rows align fixed facts'`.
  The host run retained the intentional `FARHELM_E2E_SUITE_TERMINALS=host` input through the recorder.
- `cargo check -p farhelm-ui -p farhelm-supervisor`, `cargo build` and the Dioxus 0.7.10 release web build passed.
  Markdown dprint, Rust formatting, changelog format and whitespace checks passed. The source delay checker inspected
  397 delays with no missing rationale. The instruction-anchor checker had zero errors and three existing stale-citation
  warnings outside this change.

The final JavaScript run covered the source tree committed as `a67d4bec`. Both browser runs and compilation covered the
same agent-color behavior on base `be255bc2`. The ordinary browser run preceded a capture-only refinement: the
screenshot now selects neutral Codex so all seven colored marks remain visible. Assertions, fixtures and product
behavior were unchanged; the host run executed the final version and supplied the inspected captures. Subsequent main
changes were inspected in full: grid-link joining and its tests, distinct specification sections, and TODO/plan
bookkeeping. They preserve the row rendering, CSS and selection contracts these checks exercise, so browser and compile
evidence was reused without another build or runtime run. Source checks were also reused because their covered edits and
formatting were preserved. No earlier-session runtime evidence was reused, and no runtime test failed during this plan.

The full Rust runtime, desktop native, website and unrelated browser suites were skipped because the behavior changes
only session-list CSS. The targeted browser checks exercise the actual cascade and selected-row behavior; compilation
covers the Rust comment edits and web bundle. Published screenshot capture and publication were outside the plan.

## Review gate outcome

The required single fresh-context gpt-6.1-sol high adversarial review found no actionable issues. It independently
checked the source records, recomputed all 28 contrast ratios and 42 pairwise distances, and inspected the CSS cascade,
unchanged shapes and markup, and test premises against the plan. Browser results were pending during that review and are
reported separately above. Commit and PR wording passed a separate fresh-context cold read.
