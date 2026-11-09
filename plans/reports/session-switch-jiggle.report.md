### What this was about

Switching back to a new terminal could move the last word of a full-width line and the horizontal rules around a prompt.
The sizing calculation rounded from its previous answer. An unchanged pane could therefore change column count on a
duplicate notification, or retain an answer from an earlier pane size that reopening discarded.

The earlier investigation captured the first cause in Linux WebKit at the default text size: the session appeared at 99
columns, then a duplicate fit committed 98 while visible. Its failed geometry run was retained
(`b24d16cd-b85d-477f-85fa-ee5171cb4093`). The initial draft corrected duplicate notifications, but review found the
resize-history case. The maintainer then chose deterministic sizing, accepted rounding-boundary differences and ruled
out retained sizing records between visits.

The completed correction gives new terminals the same rows and columns for the same pane, character metrics and pixel
ratio, regardless of resize history. Drawing still follows the dimensions committed by the supervisor. The requested
TODO entry is removed.

### Things you should know

Near a rounding boundary, a pane may fit a row or column differently than before. A compatibility handoff from the old
display component preserves the size already sent to the program until the new component observes a sizing input change.
That inherited size can differ from ordinary new-terminal sizing; the exception lasts only for that mounted view, so a
later fresh view uses the deterministic answer. No sizing records survive session visits.

### Open questions and possible follow-ups

Native Mac confirmation remains outstanding. The relevant check is switching between new-terminal Claude sessions,
including after resizing the pane, while watching the final word of full-width lines and the horizontal prompt rules.
Linux WebKit reproduced the cause and the final browser regressions pass, but they do not exercise the native Mac app.

### The PRs

- [#3234: keep terminal text still on session switch](https://github.com/scode/farhelm/pull/3234/changes) — amended the
  existing draft to finish both identified sizing cases and preserve the compatibility handoff.

### Checks run, reused and skipped

- Before the deterministic correction, the new resize-history component regression failed on both engines: a fresh pane
  fitted 83 columns while the resized pane fitted 84. Recorder `59651ff8-3cb3-463b-8e33-10327c8a4d34` and its browser
  artifacts remain retained. This was a deliberate pre-fix reproduction, not a passing gate.
- Focused fitting/handoff selection reported 20 passes (`a8d7e285-3be7-4400-8c64-d00e22be53c7`); the complete grid
  component suite reported 200 passes, 100 per engine (`5b1dfe60-c781-4318-9533-51d8ef2b9ef1`). These generic recordings
  use complete console totals rather than validated per-case recorder counts.
- Strict recorded Playwright selection of `terminal-switch-geometry.spec.ts` and the existing remount, early-switch and
  real-resize cases in `terminal.spec.ts`: 10 passes, five per engine, no skips or retries, recorder
  `c3e0a5eb-3c4c-4c3f-8bbc-841c316e167e`. The four geometry cases cover 13px and 14px text after genuine pane resizing,
  comparing the last-column word, every rule span, row and screen throughout the visible return. Pinned tmux 3.7c
  matched; all three owned stacks cleaned up.
- The web bundle was rebuilt with pinned Dioxus CLI 0.7.10. Existing CLI and fixture binaries were reused from earlier
  development builds; their exact build revisions are unavailable. This correction changes the browser layer, and the
  inspected upstream changes do not alter the terminal stream or fixture paths this selection exercises. The binaries
  were not rebuilt at the final PR head; this limits provenance, though the real terminal behavior was exercised.
- JavaScript syntax, touched Markdown formatting, changelog format, whitespace and the instruction checker passed. The
  instruction checker retains one pre-existing missing-heading warning in the triage ledger. The isolated source-delay
  checker inspected 430 delays with no missing rationale.
- The complete component suite and application selection covered working tree `45545298` above rebased partial commit
  `4d5b3cb1`. The subsequent careful rebase onto `3504e0b7` preserved the complete feature diff. Upstream added
  planning/watch tooling and TODO edits, with no product or browser-input interaction; both sides of shared bookkeeping
  were preserved. Those successful commands remain applicable without repetition. No earlier partial-fix pass is used to
  certify the new behavior.
- Rust runtime, native desktop, provisioning and the full application browser suite were skipped: Rust is unchanged, and
  the selected component and real-session checks cover the changed sizing and mounting paths. No live install was
  changed.

The recorded browser commands below ran from `e2e/`, in the order baseline, focused fit, complete component suite and
application selection. Both configured engines ran in every command. Angle-bracket values redact private tool and
evidence locations, retaining the actual selection and execution policy.

```sh
python3 ../scripts/record-test-run.py --kind development --selection 'residual grid resize-history regression before deterministic fix' --concurrency '2 browser workers; both engines; retries 0' --tmux none --timeout 120 -- npx playwright test --config grid-client/playwright.config.ts grid-fit.spec.ts -g 'resized and freshly mounted panes' --workers=2 --retries=0

python3 ../scripts/record-test-run.py --kind development --selection 'deterministic grid fitting and renderer handoff; both engines' --concurrency '2 browser workers; retries 0' --tmux none --timeout 180 -- npx playwright test --config grid-client/playwright.config.ts grid-fit.spec.ts grid-attach.spec.ts -g 'unchanged pane|inherited size|resized and freshly|documented|renderer swap preserves' --workers=2 --retries=0

python3 ../scripts/record-test-run.py --kind development --selection 'grid component suite after deterministic sizing; both engines' --concurrency '2 browser workers; retries 0' --tmux none --timeout 360 -- npx playwright test --config grid-client/playwright.config.ts --workers=2 --retries=0

PATH="<pinned-tmux>:$PATH" python3 ../scripts/record-test-run.py --runner playwright --kind development --tmux required --timeout 240 --output-root '<private-evidence>' --selection 'real session geometry after resize plus existing switch and resize; both engines' --concurrency '3 browser workers, one stack each; retries 0' -- npx playwright test 'terminal-switch-geometry\.spec\.ts|terminal\.spec\.ts' -g 'session switching keeps full-width|switching sessions tears|switching sessions before|resize reaches the real terminal'
```

Static commands ran from the checkout root; formatting and instruction checks were repeated after the final rebase. The
web build ran from `crates/farhelm-ui` with
`CARGO_BUILD_JOBS=4 <pinned-dx> build --package farhelm-ui --platform web --release`.

```sh
node --check crates/farhelm-ui/assets/grid-terminal.js
node --check crates/farhelm-ui/assets/terminal.js
dprint check .agents/validation.md SPEC_impl.md TODO.md e2e/grid-client/grid-fit.spec.ts e2e/grid-client/grid-attach.spec.ts e2e/tests/terminal-switch-geometry.spec.ts releasing/changelog.d/session-switch-jiggle.md
python3 releasing/check-changelog.py format
python3 scripts/check-agents-md.py
git diff --check
<isolated-python> -B scripts/check-test-sleeps.py
```

### Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no defects in the complete correction. An
independent scope assessment accepted the fixed sizing reference and existing component-local handoff exception; no
cache, retained view or new lifetime mechanism was needed. Commit and PR wording passed its cold read.
