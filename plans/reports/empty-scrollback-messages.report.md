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
