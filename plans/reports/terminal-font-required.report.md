## What this was about

Farhelm ships its terminal font, but previously opened a terminal in a fallback font after about three seconds and
swapped in the bundled face later. That kept recovery and resize paths for an asset the maintainer decided is required,
just like the bundled scripts. This plan removes those paths and the corresponding TODO entry.

## Things you should know

Terminals now wait for both regular and bold JetBrains Mono before appearing. Over a slow link, the terminal area stays
empty until both arrive. A failed download leaves it empty until reload after the problem is fixed; one browser console
error records the failure. There is no loading indicator or in-terminal error, as decided during planning. Other page
text retains its ordinary CSS fallback behavior.

The short retry for a stylesheet that has not registered the font yet remains. Font loading starts on the first terminal
mount attempt. Terminals no longer subscribe individually to a font promise, and there is no late family swap or bold
repaint.

Mercury 3 through omp produced the implementation and honored both checkpoints, then hit a provider credit refusal
during its final turn. The executor retained its patch, took over as the plan permits, inspected the code and decisions,
and reran validation. The delegate's report alone was not treated as a pass.

## Open questions and possible follow-ups

No maintainer decision is needed. The browser tests do not deterministically force the brief empty-face registration
race; they cover held and rejected real font requests. WebKit clipboard assertions remain unavailable because Playwright
cannot grant its clipboard permissions. The changed clipboard cases execute on Chromium.

## PRs

- [#2700](https://github.com/scode/farhelm/pull/2700/changes) — require the bundled terminal font; one draft containing
  the implementation, browser coverage, updated comments and specification, changelog fragment, and TODO removal.

## Checks run, reused and skipped

- Passed `cargo build` and `dx build --package farhelm-ui --platform web --release` to prepare the actual web
  application.
- Passed: 249 cases, with 9 expected WebKit clipboard skips and no failures, flaky outcomes or interrupted cases.
  Chromium passed 129; WebKit passed 120. Command:
  `record-test-run.py --runner playwright --kind development --tmux required -- npx playwright test tests/terminal-font.spec.ts tests/terminal.spec.ts tests/terminal-clipboard.spec.ts tests/terminal-keys.spec.ts tests/terminal-links.spec.ts tests/terminal-reconnect.spec.ts`;
  run `0a865750-428e-411a-9df7-4f3ee72bc750`, three browser workers with one stack each and zero retries. Both engine
  families are selected. This covers the mount gate, removal of obsolete font waits, input, clipboard, links and
  reconnect behavior.
- Passed the isolated source delay checker: 315 test delays, none missing a rationale. Its first pre-correction run
  found two split annotations; those were fixed before the final pass.
- Passed `node --check crates/farhelm-ui/assets/terminal.js`,
  `dprint check SPEC_impl.md TODO.md e2e/tests/fixture-contracts.md`, `python3 releasing/check-changelog.py format`, and
  `git diff --check`.
- No previous-plan test evidence was reused. Rust runtime, full browser, desktop-shell, installer and provisioning
  suites were skipped: the executable change is confined to the shared terminal JavaScript, covered by the selected
  browser integration run; the Rust edit is a comment. Real vendor-agent turns were unnecessary for comment-only helper
  edits.

## Review gate outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found one test gap and no implementation defect.
Holding both font downloads together could not distinguish waiting for regular alone from requiring both weights. The
test now releases regular first, proves it loaded while bold remains held, observes no terminal beyond the old deadline,
then releases bold and checks real output and the configured family. The same reviewer verified the correction closes
its sole finding; none remain. The separate cold read of the commit and PR wording also passed.
