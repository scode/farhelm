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

### Landing

Landed last of three in its round, above stop-start-message (Stop during an agent's startup now says so) and
upgrade-epochs (the minimum version an installation or host can update from); since its base, main had gained the
fixed-height Settings dialog, review-queue bookkeeping and a file-mode fix for the Tensorlake test runner. A
fresh-context review read all of them against each other and found no interaction: the Settings change does not touch
terminal panes, and no other test relied on the old sizing that depended on a pane's resize history. The landing made no
fixes, and the executor's report still holds.

You decided while this plan was blocked that new terminals are sized the same way for the same pane and font, whatever
their resize history, and that the size already sent to the program is kept when a session moves from the old terminal
display to the new one. The code and SPEC_impl.md follow that.

#### Things you should know

- Taken literally, keeping the already-sent size means the new display's first fit after that move does not correct a
  size that went stale during the move itself; a later resize does. That is the decision as you made it, noted so it is
  not a surprise.
- From the arithmetic, not observed: the fixed sizing can make a wide or tall pane draw about two pixels past the space
  it was fitted to, which a freshly opened terminal already did before.
- The executor asked for a check in the native Mac app: switch between new-terminal Claude sessions and look at the last
  word of full-width lines and the prompt's horizontal rules, including after resizing a pane. No landing check covers
  the native app.

#### Checks

The three plans were checked together, stacked in queue order on main (`db0750fb` at the top), on this development
machine and, at the maintainer's request, on Tensorlake sandboxes.

Run on this machine:

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, the test-sleep checker, and
  `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh` passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-desktop` passed.
- Recorded `cargo nextest run -p farhelm-ui --features desktop` (the desktop app's own code, including its updater):
  1111 passed, run `7ea46bc3`.
- `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` (the macOS installer driven against fake releases):
  601 checks passed. This and the next two items are their own harnesses, run directly rather than through the test
  recorder, so they have no run id; their output was read in full.
- The release brick-test harness's unit tests
  (`python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`) passed.
- The UI's JavaScript unit tests (`node --test` in `crates/farhelm-ui/js-tests`) and the grid terminal component's own
  browser tests (`npx playwright test --config grid-client/playwright.config.ts`, Chromium and WebKit, no stack): 200
  passed.

Run on Tensorlake sandboxes:

- Twelve terminal-related browser spec files on Chromium and WebKit, 24 file-and-engine pairs over ten sandboxes, using
  the release web build: `terminal`, `terminal-switch-geometry`, `grid-terminal`, `terminal-font`, `sidebar-resize`,
  `terminal-reconnect`, `terminal-migration`, `terminal-tabs`, `terminal-restart`, `terminal-attachments`,
  `terminal-multihost` and `interface-fonts`. 502 passed, none failed, skipped or retried; runs `c1719814`, `bdda5312`,
  `abf8c3cb`, `480bc664`, `83a819ff`, `91906e53`, `4306834f`, `44a7f61a`, `04e28cba` and `da840416`, in the recorder's
  plain command mode (its Playwright mode cannot pick one engine per file) under the sandbox's slightly older Node 24.
  Every sandbox reported its per-user systemd manager and its SSH to itself working.
- Every Rust test in the workspace except the desktop app's, from one test archive split over nine sandboxes
  (`cargo nextest archive --workspace --exclude farhelm-desktop`, then a hash partition per sandbox, in the recorder's
  plain command mode because its nextest mode cannot take an archive): 5901 tests, 5900 passed. Runs `d42950f4`,
  `10ab214f`, `239bae3e`, `13916f28`, `efe37aea`, `55d9e4b4`, `830ede08`, `750b573b` and `dac5478e`. Rootless Podman and
  its image were present, so the container tests ran. The one failure, `attribute_compile_contract_matrix` in
  `farhelm-testtrace-macros` (run `10ab214f`), compiles small crates with a nested `cargo`, which the sandboxes do not
  have ("spawn: No such file or directory"); it passed here (below). Two tests reported themselves skipped for their
  substrate, as they do wherever it is missing, and neither concerns these plans: OMP live admission (no Bun on the
  sandboxes) and one tmux test that needs a tmux older than the supported one on `PATH`.

After the landing fixes to upgrade-epochs (the installer's refusal text and the changelog kind), run here: formatting,
`dprint check`, the changelog format check,
`cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`, recorded
`cargo nextest run -p farhelm-supervisor --lib -E 'test(/installation::/)'` (32 passed, run `39b375a6`), recorded
`cargo nextest run -p farhelm-testtrace-macros --test compile_contract` (2 passed, run `05991d87`), and
`test-install-sh.sh` again (601 checks passed). Main after the merges differs from the tested tree only by those fixes
and TODO.md entries other sessions landed meanwhile.

Reused: the executor's own runs for each plan, as their reports list them; this landing reran broader suites rather than
relying on them.

Skipped:

- The rest of the browser suite: the specs above cover the terminal, sizing and stop paths these plans change.
- The desktop smoke: it starts the desktop app and checks its supervisor and terminal, not its updater; the updater's
  refusal is covered by the desktop tests above, and the smoke cannot run a real update from an old release.
- The CentOS provisioning run: it sets up a fresh host, which the helm's new refusal (for hosts already running a
  too-old version) does not affect; the helm's own tests of that refusal ran in the workspace run above.
- The release workflow itself: it runs only on a tag.
- Hosted CI is manual-only and was not dispatched.
