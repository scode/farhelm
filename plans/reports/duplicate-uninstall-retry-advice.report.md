## What this was about

The triage outcome "duplicate uninstall advice points to a removed owner" (2026-10-21, decided without asking under your
instruction for clear-cut items). When two host entries reach the same machine, only one connects; the other is refused
as a duplicate, and choosing Uninstall on it is refused with advice to uninstall through the connected entry. If you
then removed that connected entry, the remaining one stays refused until you press Retry, but its uninstall refusal
still said "choose uninstall on another entry (host N), then remove this entry", pointing at an entry that no longer
exists. Triage decided to change only that advice, for only that case, without promising that uninstall then succeeds,
and to leave the name ("another entry (host N)"), the other refusal cases and the host-details status text in the host
list as they are. Size gate: about 100 lines; over it, block and bring it back.

## Things you should know

- The refusal for that case now ends its advice with: "That entry has since been removed: press retry so this entry can
  connect to the machine, then choose uninstall on this entry". The rest of the refusal is unchanged, so it still opens
  by saying this entry reaches the same machine as "another entry (host N)".
- "Can connect" rather than "connects", because Retry can still fail (the machine is unreachable, or the removed entry
  was added back as a new entry, which makes a fresh duplicate). It also makes no promise about uninstall itself: its
  usual checks, such as running sessions, still apply after the retry. This matches SPEC.md's duplicate remedy (remove
  the other entry or change this one's destination, then press Retry), and an existing helm test shows Retry connects
  the surviving entry once the other is removed.
- The new advice appears only when the helm's host list no longer holds the other entry; a failed read of the list is an
  error, not this case.
- The changelog entry this wording belongs to has not shipped in a release yet, so the PR adds a context paragraph to
  that entry's fragment instead of adding a new fragment.
- Size: about 70 changed lines of code and tests, within the gate.

## Open questions and possible follow-ups

- Possible follow-up: the refusal still opens by saying this entry reaches the same machine as the removed entry, which
  reads oddly next to "That entry has since been removed". Triage scoped that sentence out, so I left it.

## The PRs

- #2605 fix: suggest retry when uninstall refuses a removed entry's duplicate

## Checks

Run now:

- Recorded `cargo nextest run -p farhelm-helm --lib` selecting the uninstall refusal module, both duplicate uninstall
  tests and `retry_clears_a_duplicate_once_the_other_entry_is_gone`: run 1a429932-e5bd-41ae-973e-21cc7f52570a, 7 of 7
  passed.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check`: pass.
- `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown files: pass.
- `python -B scripts/check-test-sleeps.py`: 314 test delays, none without a rationale.

Reused: none. Skipped: other helm, UI and browser tests, because only the refusal's wording and its two tests changed,
as the plan's validation section says.

## Review gate

A fresh-context Opus 5.5 review at high effort found no blocker. I applied its two low findings: the live-entry case now
asserts that the new advice does not appear, and "connects" became "can connect". I left two nits: the refusal's
present-tense opening sentence before "has since been removed" (out of the triaged scope), and merging two adjacent
conditionals in a test (I kept the clearer order).

### Landing

Landed as #2605, with the same content the report describes, rebased onto this round's main with one triage-record
conflict resolution. It was the second of two plans in this landing round, after test-hygiene-oct21.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained bind-gui-answers (GUI confirmations and prompts, host Remove and the setup, update
and uninstall confirmations included, now act on what the user saw when answering) and documentation and planning files.
A fresh-context reviewer and I read this plan against them and against the earlier uninstall work that reworded the
uninstall refusals and added the Remove offer (uninstall-refusal-wording) and refused uninstall when a home alias hides
kept data (uninstall-mixed-spelling):

- bind-gui-answers' changes to host Remove and the setup, update and uninstall confirmations are all in the GUI; the
  refusal text this plan changes comes from the helm and reaches the GUI unchanged.
- The host menu offers Retry whatever state the host is in, and the host details already say to press Retry, so the new
  advice matches what the user can do and SPEC.md's remedy for duplicate entries. The Remove offer and the other
  duplicate cases are unchanged.
- The plan extends an existing changelog fragment rather than adding one, which is right: the refusal it changes has not
  shipped yet.
- test-hygiene-oct21, the other plan in the round, changes only tests.

#### Fixes made during the landing

None to the plan's code. Its triage-record update conflicted with main's new triage entries; the landing kept main's
entries and this plan's completed Execution line.

#### Possible follow-ups

- Already raised by the executor and its reviewer, and left out of scope by triage: the refusal still opens with "it
  reaches the same machine as another entry" in the present tense, just before saying that entry has since been removed.

#### Checks

The two plans change about 200 lines: tests and fixtures in Rust, five browser specs and the start-stack cleanup script,
plus the helm's uninstall refusal text. This round ran every test they change, and the rest of the helm crate, on both
plans stacked together on this round's main (which already held bind-gui-answers), including the triage-record conflict
resolution:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the isolated test-sleep check (tests changed), `dprint check` on the
  changed Markdown files, `python3 releasing/check-changelog.py format`, `shellcheck --severity=warning` on the changed
  script, the workspace debug build and the UI's release web build the browser tests drive: all passed. Shellcheck's
  informational notes on that script predate these plans.
- `cargo nextest run -p farhelm-helm -p farhelm` with the filter
  `package(farhelm-helm) | binary(spare_builder) | test(agent_listing_real_stack::)`, through the test recorder with
  pinned nextest and tmux, four slots, no retries, run `e468ec4f-d6fb-428f-8478-cec9a9249547`: 1307 passed, none failed,
  with no runtime `SKIPPED` message in the retained output, so the end-to-end tests ran for real. This covers the whole
  helm crate (the uninstall refusal), the spare-builder tests and the agent-listing end-to-end tests this round changed.
- `bash scripts/test-start-stack-cleanup.sh` through the recorder, run `f458427a-72d1-4cd8-b7dd-a867843b5e41`: every
  case reported ok.
- `npx playwright test` in `e2e/` for `clone`, `custom-permissions`, `notifications`, `quick-launch` and `settings` on
  Chromium and WebKit, three workers each with its own helm and supervisors, no retries, run
  `44d59b3c-d844-46f4-8645-cdcdcf3a7ae6`: 192 passed, none failed. The "owned fixture cleanup failed" errors in its
  output come from a notification test that injects cleanup failures on purpose and passed.
- Reused: none. Skipped: the rest of the Rust workspace and browser suite (only the tests above and the helm's refusal
  text changed), the UI, desktop and JavaScript tests (no UI or asset code changed), the website build, the doctests and
  the installer (unchanged).

The report's checks ran the helm's uninstall refusal tests on its base; this round ran the whole helm crate on both
plans stacked on this round's main. Nothing in the executor's report became untrue at landing.
