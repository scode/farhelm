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
