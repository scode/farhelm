# What this was about

A broken or malicious host could send valid terminal-history updates containing no rows. If these arrived while
scrollback was waiting to be processed, trimming old output could give history rows the same numbers as live screen
rows. Clicking a URL painted in scrollback could then open the live screen row's different URL, without displaying that
destination. Ordinary Farhelm supervisors do not send these empty updates.

The maintainer chose to drop empty scrollback messages on arrival. They now pass validation and settle their numbering
without taking a place in the waiting queue. Real rows alone count toward trimming. The regression checks an initial
empty message and an empty message inside a backlog, overflow of the history cap, yielding between real rows, exact
history numbering, and the displayed history row's own URL through the same screen-first lookup links use.

# Things you should know

The code, tests and documentation diff is 78 lines, within the agreed approximately 100-line gate. The protocol and
supervisor encoder are unchanged. The existing unreleased grid-terminal changelog fragment was updated, rather than
adding a second entry for a fix to that work.

The feedback item and its index entry are removed, and its decision moved to the archive with this PR's change id,
bookmark and URL. The ledger entry's original assessment remains unchanged; it records the source-only evidence
available at triage. This execution adds model-level runtime evidence, not a browser-click reproduction. No ordinary
sender, credential exposure, code execution or measured resource exhaustion is claimed.

# Open questions and possible follow-ups

None for this outcome. Separate queued findings about extreme history numbering and other scrollback behavior remain
outside this plan.

# The PRs

- [#3289: keep scrollback links on their displayed rows](https://github.com/scode/farhelm/pull/3289/changes) — draft,
  based on main; the single PR for `empty-history-queue-bypasses-bounds.md`.

# Checks run, reused and skipped

All runtime commands ran from `crates/farhelm-ui/js-tests`. The portable recorder commands were:

```sh
python3 ../../../scripts/record-test-run.py --kind repetition --selection 'empty deferred history regression before fix' --concurrency 'one node test' --tmux none -- node --test --test-name-pattern='empty appends cannot shift' grid-history-storage.test.js
python3 ../../../scripts/record-test-run.py --kind repetition --selection 'empty deferred history regression after fix' --concurrency 'one node test' --tmux none -- node --test --test-name-pattern='empty appends cannot shift' grid-history-storage.test.js
python3 ../../../scripts/record-test-run.py --kind development --selection 'asset JavaScript unit harness including empty history and protocol golden fixtures' --concurrency 'node default discovery' --tmux none -- node --test
```

- The new regression failed as expected on the original model: one failure, run `dbed0abc-4127-4a8b-82c3-56804b82f7e5`.
  That failed record is retained. After the fix, the focused regression passed, run
  `b910941e-45d0-4bdd-9755-e9829e3b8f47`.
- The complete asset JavaScript harness, `node --test`, passed 551 tests with zero failures or skips, run
  `0ca65b8b-a787-41f2-b8da-2fe8f058f7fd`. This includes the unchanged grid protocol golden fixtures. Its recorder
  identified main `596dd2ad` plus the intended model/test/spec diff, fingerprint
  `ec2324e5691c068e619a03a66c07e8973ee2af80e28d6e73caf3d2f4aa467d24`; source collection and output were complete, with
  no forced cleanup. This evidence is reused for the final PR: subsequent changes were one explanatory module comment
  and ledger, feedback and changelog bookkeeping, leaving the tested runtime behavior and tests intact.
- Configured Markdown `dprint check`, `python3 releasing/check-changelog.py format`, and
  `python3 scripts/check-agents-md.py` passed. The instruction checker reports one existing archive citation warning and
  zero errors. JavaScript has no configured dprint plugin, so a JS-only invocation matched no files; no JS formatter
  pass is claimed.
- Rust builds/tests and browser suites, including grid-component tests, were skipped because the change stays in the
  model and its unit tests, with no renderer, scheduling, transport, protocol or Rust changes. The unit regression
  directly exercises the history coordinates and link-source integration affected here. No Rust or browser tests were
  edited, so the test-sleep checker was not applicable.

# Review gate's outcome

The required single fresh GPT-6.1 Sol high review returned CLEAR, including a final comment-only addendum. The review
included the full goal and test-authoring checklist. A separate fresh reader understood the PR text, and the full ledger
and feedback index audit found no duplicate or contradictory decision. No review finding was rejected.

### Landing

Landed alone, as #3289. Nothing reached main between the plan's base and its merge apart from planning bookkeeping, and
no other plan landed in the same round, so nothing could interact with it.

#### Fix made by the landing

The landing changed the changelog, and nothing else. The executor added a sentence about this fix to the unreleased
changelog entry for the smaller, cheaper terminal transfers, treating the fix as an amendment to that unreleased work.
It is not: the code that let an empty scrollback update take the place of real rows is the same in the v0.37.1 release,
so the bug is in a stable release, though only a broken or malicious host can trigger it. The landing took the sentence
back out of that entry and gave the fix an entry of its own, as a fix, saying that such a host could make a link in
scrollback open a different address from the one shown, and that links now always open what they show. This makes untrue
the report's statement that the existing unreleased fragment was updated rather than adding a second entry.

#### Review

The landing's fresh-context review found the code change correct, and nothing in it needing a change. It went through
every order in which empty updates, real scrollback, resets and trims can arrive, whether scrollback is loaded at once
or in slices: for anything an ordinary Farhelm host sends, the result is the same as before, and for a hostile host the
numbering is now exact, with no overlap with the live screen and no gap. The count of scrollback rows waiting to load
now matches the rows actually waiting (it could go negative before, which also switched off the limit on how much
scrollback is kept), and the page stays responsive while scrollback loads even under a flood of empty updates. The new
test fails on the old code and passes on the new one.

#### Things you should know

From the review; none of it affects users:

- One harmless difference, reachable only from a hostile host: an empty update that arrives right after a scrollback
  reset, with no real rows after it, now leaves the internal "first scrollback line" at zero rather than at the empty
  update's number. With no scrollback lines there, nothing reads that number (every reader first checks that scrollback
  is not empty, and ordinary hosts already reach the same state), and the next real row sets it exactly as before.
- The reviewer also ran the new test and a comparison against scratch copies of the old and new code, outside the
  recorder. That supports the review's reasoning but is not part of the landing's checks.

#### Checks

As the maintainer asked for this session's landings, the browser specs that need a real helm and supervisor ran on
Tensorlake sandboxes (cloud test machines), and everything else ran on this development machine.

Run now, on a tree that differs from what merged only in the changelog fix described above, which no test reads; the
changelog and formatting checks were run again after that fix.

- Here: the test-sleep checker, `dprint check`, `python3 releasing/check-changelog.py format`, `cargo build` and the
  release web build (`dx build --package farhelm-ui --platform web --release`) passed.
- Here, the asset JavaScript unit harness (`node --test` in `crates/farhelm-ui/js-tests`, through the recorder): 551
  passed, none skipped (run `eb367aed`). It includes the plan's new test.
- Here, the grid terminal component suite on Chromium and WebKit
  (`npx playwright test --config grid-client/playwright.config.ts --retries=0`, through the recorder's general command
  mode): 204 passed (run `19d306e8`). The executor skipped this suite. It runs the terminal component against recorded
  streams, including scrollback applied in slices and links in scrollback, so it covers the changed code inside a real
  browser.
- Tensorlake, the browser specs whose sessions attach to, flood, reconnect to and click links in new terminals, on
  Chromium and WebKit, on eight sandboxes, using the release web build
  (`scripts/tensorlake/sandbox-tests.ts run --template <template> --shards 8` with the terminal, terminal-links,
  grid-flood, terminal-flood, terminal-reconnect, terminal-replay-rename and terminal-attachments projects of both
  engines): 330 passed, none failed (runs `c6cf6b17`, `6b660e87`, `005e970e`, `ccae467f`, `10248aa6`, `d67754a6`,
  `7b030666`, `638beed7`). The floods trim queued scrollback, which is the code the plan changed for every stream, not
  only hostile ones. The runs went through the recorder's general command mode, because its Playwright mode always runs
  both engines and these runs split the engines across sandboxes. The sandbox image runs Node 24, a slightly older
  release than this machine's. Every sandbox reported that the services the tests rely on (a per-user systemd and SSH to
  itself) worked.

Reused from the executor: nothing; the runs above cover its unit harness run.

Skipped: Rust tests and lints (no Rust changed), the desktop compile check (no desktop code changed, and the set of
bundled asset files is unchanged), the rest of the browser suite (the change is confined to how the terminal component
queues scrollback, which the runs above exercise), and hosted CI (manual-only, not dispatched).
