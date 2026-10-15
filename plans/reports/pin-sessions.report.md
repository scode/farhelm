## What this was about

The sidebar had no way to keep a session above other work. The maintainer chose shared pins from each row's menu, with
pinned sessions following the same selected sort as the rest, a muted pin icon and a separator, and Replace carrying the
pin over. The quick switcher was to keep its existing order.

The menu now offers **Pin to top** / **Unpin**, including for unreachable hosts and ended sessions. Pins appear first
when the pointer leaves the list, or stays still there for five seconds with the menu closed. Until then rows stay in
place, and the separator follows only the displayed leading pin group. Every connected window sees the shared change,
and deleting a session clears its pin. Replace and Replace with preserve it on the new session.

## Things you should know

The helm database advances to schema 52. As accepted in the plan, an older release's helm then refuses that database;
there is no data loss or supervisor database/protocol change.

Replace copies a pin before deleting the original. If deletion fails and both sessions survive, both can be pinned. If
the pin copy fails, the error identifies the already-created replacement and says the original was not removed. Pins
cannot recover sessions omitted by the existing per-host list cap. No manual pin order or per-device pin state was
introduced. The session-list guide, specifications and changelog fragment were updated, and the TODO entry removed.

## Open questions and possible follow-ups

No maintainer decision is needed. Existing list-cap limits and bounded stray pin rows from deletion through another helm
remain within the agreed scope.

## The PRs

- [#2064](https://github.com/scode/farhelm/pull/2064/changes): shared session pins and sidebar presentation, with the
  storage, lifecycle handling, tests and documentation together. It remains a draft for the landing monitor.

## Checks run, reused and skipped

- Passed 183 selected Rust tests covering pin storage/routes, both listing sources, delete/replace, schema migrations,
  HTTP contracts, row ordering/holds and menu behavior. Recorder run `1cd2cb49-76cc-4b75-92a4-7573b39f69e1`: four slots,
  zero retries; 1,407 tests deliberately selected out and no runtime `SKIPPED` markers. No tmux/systemd/SSH execution
  was needed for these store and scripted-peer tests.
- Passed 20 focused fixture tests in run `73ef9103-a831-4572-b07f-f52e5c5d6a8f`. An earlier selected run
  `f90cd9f7-ce6b-4308-b946-7f73d721985c` had four fixture failures: historical-schema reconstruction retained the new
  table, and row memoization fixtures supplied fresh callback handles. Both were corrected without weakening their
  assertions, then the full bounded selection above passed. Earlier compile attempts also caught an incorrect fixture
  builder return assumption; a recorder invocation refused an unsupported selection flag before executing tests.
- Passed the new pin/unpin browser test in Chromium and WebKit, two cases, one worker and zero retries: run
  `dbd96ebb-a93a-41ed-856c-7749baab60b0`. It checks menu writes, icon and accessible text, muted color, reordered rows
  and separator after leaving the list, another client's updates, and unchanged helm title order. Both captured sidebar
  images were inspected. The CLI/fixture and release web builds passed before that run.
- Passed `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `dprint check`, the isolated
  `python -B scripts/check-test-sleeps.py` check (299 delays, none unexplained), whitespace checking, and
  `python3 releasing/check-changelog.py format` on the final change.
- Passed `bun install --frozen-lockfile` and `bun run build`: 32 pages and valid internal links. Reused that build
  because the page did not change afterwards. The validated PR head is `0e6959f79f0c873138e8f294301f8e589e36ad19`.
  Intervening main changes inspected during execution were queue claims, a delivery/report and a landing claim, so
  rebasing introduced no product interaction or need for another runtime battery.
- Skipped unrelated JS, installer, release and supervisor runtime suites, the full workspace/browser batteries and a
  native Mac run: this changes helm metadata and shared sidebar rendering, covered by the targeted Rust tests and both
  browser engines. No deployment, release promotion or live-install mutation occurred.

## Review gate outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found one P3 visual issue: a pin inherited a brighter
foreground than a stale title. It was changed to the muted token, with a browser assertion. The reviewer then checked
the exact final fix/fixture delta and found no new actionable issue. It reviewed the changed tests against the full
repository test-authoring checklist. The separate wording cold read understood the title and found no contradiction or
convention issue. Implementation stayed local under the plan's no-workhorse requirement. Private execution evidence
retains review usage; native wording-reader and orchestrator token counters were unavailable.

### Landing

Landed as #2064, the first of the round's two plans; ordered-gui-helm-requests (#2062) landed after it, and both were
tested stacked on top of main. Main had gained mid-session YOLO detection (#2063), which rewrote the same SPEC.md row
paragraph; the conflict was resolved keeping both the pin sentence and the observed-mode text.

#### Review

An independent fresh-context review found the helm side sound: the schema 52 migration is idempotent and matches a fresh
database; the pin route is authenticated, refuses a missing field or unknown session without notifying, and notifies
only on a real change; delete and Replace clear or copy the pin only after the supervisor confirms, with the documented
failure outcomes; pins are partitioned only in the sidebar's own copy of the list, before the hold, and the quick
switcher's order is unchanged. Older releases refusing schema 52 needs no changelog entry under the release rules.

#### Fixes made while landing

- The new menu item sits between rename and clone in every row menu, which broke six existing browser tests that walk
  the menu by keyboard or count its items (in the sidebar, clone and replace specs); they now include it. It also had no
  hover text, which the tooltip-coverage sweep requires; it now explains pinning and unpinning.
- The row menu is one item taller. Measured in both engines, the full menu is now 368.6px, while the space kept free
  below it was 380px, sized with about 38px of slack for font differences; the slack was down to 11px. The reserve is
  now 407px, and the tests that depend on it say so.
- The pins spec now resets the shared sort preference it changes, so later specs do not inherit title order. Its comment
  claimed to prove the pointer hold; a diagnostic showed that closing the last row's menu leaves the pointer below the
  list, so the hold is already released there. The comment now says what the test proves and points to sort.spec.ts,
  whose tests cover the hold.
- The store test now checks what copying a pin returns, which Replace uses to decide whether to notify other clients;
  the two new warnings name their session; the docs give pins their own section with links, and the docs and changelog
  say pins are shared by every browser and the desktop app on the same helm.

#### Left as is

- The labels "Pin to top" and "Unpin" are capitalized, while every other row-menu label is lowercase. The plan specified
  them that way, so they were kept; a one-word decision for the maintainer.
- Test fixtures that rewind the helm database to older schemas do not drop the new table, as with earlier tables; they
  pass because the migration tolerates it.
- Suspected: pinning from the keyboard moves the focused row straight to the top, which may drop focus to the page, as
  re-sorting by activity already can.

#### Checks

- Run on both plans stacked on main:
  - format, lint, changelog and sleep checks; `cargo clippy --all-targets -- -D warnings` and the other clippy
    configurations; `cargo check -p farhelm-desktop`;
  - the helm package in full through the recorder with pinned tmux, run `90aa4784-5c92-44bc-a321-bd3ccedf81b1`: 1103
    passed;
  - the JS harness and the website build;
  - the pins, sidebar, quick-launch, templates, settings, session-confirmations, ntfy-settings, github-checkouts,
    replace, auth, feed, terminal-reconnect and tooltip-coverage specs on Chromium and WebKit, run
    `99fa749b-2ba2-41cb-b452-7efdf838832b`: 521 passed, 5 failed, the failures being the pins and menu-reserve problems
    fixed above plus one session-confirmations timeout described below.
- After the fixes: the UI crate in full with the desktop feature, run `e482a9d7-033c-4e99-9b34-ce99c43a0bee`: 648
  passed; the pins, sidebar, sort, clone, replace, quick-launch, settings, session-confirmations and tooltip-coverage
  specs on both engines, run `969a625e-943d-45d9-8695-6d72c0327f0a`: 409 passed, 1 failed.
- The session-confirmations test "skip session confirmations walks row header interrupted card and tab actions" timed
  out once in each long run, on Chromium in the first and WebKit in the second, with no unfinished browser call in its
  trace. Run alone three times on both engines it passed every time (runs `ea56e8d3-aa60-44e1-b912-8deb5710f44d`,
  `23ded877-59ac-4e96-a6c7-bfcbffc896cb`, `b8ab5064-47c7-4392-bcd4-98a9f6b5a411`). Nothing in either plan touches what
  it exercises; it appears load- or order-dependent and is not logged as a flake on these observations.
