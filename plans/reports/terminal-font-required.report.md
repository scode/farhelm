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

### Landing

Landed with its behavior as built: a terminal now appears only once both bundled JetBrains Mono weights have loaded, the
way it already waits for its bundled scripts. It no longer falls back to another font after about three seconds, and
there is no late font swap. In the web UI over a slow link the terminal area stays empty until the font arrives, and if
the font download fails it stays empty until the page is reloaded. It was the only plan in this landing round.

- #2700: require the bundled terminal font.

#### What landed since the stack was based, and whether it interacted

Nothing: since the plan was based, main gained only plan bookkeeping. A fresh-context reviewer and I read the change
against the rest of the code. The reviewer found no blocker and nothing that needs your decision. Only the terminal code
itself used the removed fallback and its three-second deadline, and only the plan's own font tests used the measuring
hook the plan removed; nothing in the other browser tests, the screenshot and video tooling, the desktop app or the docs
relied on any of them. The other browser tests that wait for fonts wait for the rest of the page's text, which is
unaffected. The TODO.md entry removed is the one this plan addresses, and every release so far has the fallback this
removes, so a new changelog entry is right.

#### Fixes made at landing

- A comment on the bundled fonts in the UI code said that fonts missing from the bundle show up only as terminals that
  never mount, "not a wrongly-rendered page". The rest of the page then renders in a fallback font too, so the comment
  now says both.
- A comment in the terminal font tests claimed that both font downloads reaching the point where the test holds them
  back proves a terminal tried to mount. The rest of the page uses the same font and can start those downloads first;
  what proves a waiting terminal is that it appears once the test lets the font through, later in the same test. The
  comment now says that.

#### Possible follow-ups

- In the desktop app the font comes from the app itself rather than over the network, so a slow download is not a
  concern there, but a webview that never reports the font loaded would now leave every terminal empty where it used to
  fall back after three seconds. This round ran the desktop smoke test on Linux's WebKitGTK, where the relaunched app's
  terminal attached to its session, which it now does only once the font has loaded. No Mac app was tried; the next
  devbuild or release test is the first to show the same on macOS.
- For release curation: the commit is titled `refactor:`, as the executor chose, although the change is visible to users
  (its changelog fragment is `kind: changed`). The fragment says a terminal stays empty over a slow link instead of
  showing a fallback font after about three seconds; that applies to the web UI, not the desktop app.

Already decided in the plan, for completeness: if the page's stylesheet never registers the font at all (it failed to
load, say), the terminal waits silently with no console message; only a failed font download logs one.

#### Checks

Run on the plan rebased onto this round's main, before the comment-only landing fixes:

- `cargo fmt --all -- --check`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`, the check that every test
  delay states its reason, `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `node --check crates/farhelm-ui/assets/terminal.js`, the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh`: all passed.
- The asset script tests (`node --test` in `crates/farhelm-ui/js-tests`), run `881493a4-1909-4275-8589-4fa727995005`:
  333 passed.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `98269cae-d516-44c5-9a36-bdd9eb88cc87`: 2566 passed, 14 skipped by design (WebKit
  cannot be granted clipboard access), 4 failed. Two are in the hosts panel's setup tests, which open no terminal:
  Chromium's "accepted ADD registers before execution and releases the page lock while running" and WebKit's "offer
  retirement releases only its owner: waiting behind another host". Both are open flakes from earlier rounds, the first
  also seen failing on main, recorded in draft PRs #2684 and #2673, which have not merged yet; they were not repeated
  this round. WebKit's "a-tab-opened-unaware-of-a-takeover-does-not-take-the-session-back" is an open flake in TODO.md's
  Deflake list; it passed 3 of 3 repeated alone (run `778d0118-f1a2-4b2a-a069-c2f19e2532b0`), a small sample, but it was
  a recorded flake before this plan existed. Chromium's "templates busy root stays keyboard-scrollable at width 390"
  failed once while the machine was heavily loaded and passed 5 of 5 repeated alone (run
  `ee25a5cd-f020-4b15-9d70-3c9b44d827aa`); it does not involve terminals, and one failure under load is not recorded as
  a flake.
- After the landing fixes: `cargo fmt -p farhelm-ui -- --check` and the test-delay check passed. The desktop smoke test
  (`scripts/desktop-smoke.sh` with the pinned tmux and its own cargo target directory), run
  `9c2e12d3-df2d-4744-a8b4-efb45647a045`: passed on WebKitGTK under Xvfb, including its check that the relaunched app
  attached a terminal to the remembered session, which needs the font to have loaded in the desktop webview.
- Reused: none. Skipped: Rust tests (the plan's only Rust change is a comment), the doctests, and the native Mac app
  (not runnable here).

The report's checks were the six terminal browser specs on both engines; this round ran the whole browser suite with the
change, since every session view mounts its terminal through the changed code. Nothing in the executor's report became
untrue at landing.
