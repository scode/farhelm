### What this was about

Farhelm's release notes were reachable inside the app only through What's new when an installed desktop update was
waiting for restart. The web UI and a desktop app with no waiting update had no link to them. The sidebar's ? menu now
offers release notes after documentation, opening the same general release notes page in your browser.

### Things you should know

The item is available on the web and desktop alike. The desktop-only check for updates follows it where an updater is
available. What's new remains in the update menu. This adds a link to the existing website page; it does not add an
in-app notes page or select the running version's section.

The Send feedback documentation page now mentions the new item. No website deployment or screenshot publication was
requested or performed. The landing monitor will merge the draft after its landing review.

### Open questions and possible follow-ups

The browser tooltip sweep found seven existing notification controls in Settings without hover text: the server, topic
and token fields, two switches, the priority choice and Save. Both engines fail that same check. Those controls and the
Settings component are unchanged from main, and inspection confirms the missing hover text predates this plan. Giving
those controls hover text is a separate follow-up; it does not prevent release notes from working.

### The PRs

- [#2006: release notes in the help menu](https://github.com/scode/farhelm/pull/2006/changes) — one draft PR, including
  the specification, docs, changelog fragment and removal of the covered TODO entry.

### Checks run, reused and skipped

- Passed `cargo fmt --all -- --check`, `cargo clippy --offline -p farhelm-ui --all-targets -- -D warnings`,
  `git diff --check`, changed-Markdown `dprint check`, and `python3 releasing/check-changelog.py format`. The UI Clippy
  selection covers the changed Rust menu and its test configuration; other crates are unchanged.
- Passed the isolated interpreter's `python -B scripts/check-test-sleeps.py`: 298 delays inspected, zero without a
  rationale. No test delay was added.
- Passed `bun install --frozen-lockfile && bun run build` in the website, including its internal link validation. Built
  the web UI and the CLI/fixture binaries for the real browser stack. The first web build could not download a
  prerequisite in a network-disabled sandbox; enabling prerequisite downloads resolved it without a product change.
- Recorder run `d436eec5-e385-435b-b589-4caaf7db5f16` ran
  `npx playwright test 'feedback\.spec\.ts' 'tooltip-coverage\.spec\.ts'` on Chromium and WebKit, with one worker, zero
  retries and pinned tmux 3.7c: 26 passed and two failed (13 passed and one failed per engine), no skipped or flaky
  cases. All feedback cases passed, including the new item's order, keyboard focus and release-notes URL through the
  shared opener. Each sidebar tooltip case passed its help-menu sweep, including the new item, before failing on the
  unrelated Settings controls described above. The selected run as a whole did not pass; its evidence is retained.
- The earlier run `5ec258ac-f2cf-4ca1-a718-ddbb024b0476` failed because the sandbox's standard browser-cache path did
  not expose its installed browsers; it reached no UI assertions. Its evidence is also retained. The corrected run is a
  separate observation.
- Reused successful checks across rebases that brought in another executor's queue claim, instructions for a separately
  requested trailing review, and a repository-protection TODO. Read every intervening diff; none changed the menu, its
  tests or their dependencies. The latest pushed revision preserves those additions.
- Skipped Rust runtime suites, workspace-wide Clippy, the full browser suite and desktop smoke: no backend, persistence
  or desktop-specific code changed, and the targeted menu/link/tooltip browser coverage exercises the changed behavior.
  WebKit covers the renderer family, not the actual desktop shell; the existing desktop external-page opener is reused
  unchanged.

### Review gate's outcome

A fresh gpt-6.1-sol reviewer at high effort found no actionable correctness, design or idiomatic-code issues, against
the full plan and the changed-test authoring checklist. The commit and PR title also passed a separate fresh cold read.
The documentation pass covered every changed file. The PR remains a draft; it has not been marked ready or merged.
