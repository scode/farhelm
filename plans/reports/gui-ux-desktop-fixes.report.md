## What this was about

The 2026-11-18 desktop triage accepted nine fixes. Keyboard users could skip past clipped approval facts. Changing
sessions could carry a confirmation to the next session or interrupt Close all for tunnels. The other fixes concern
template contrast, text direction where names appear alongside app wording, and messages that described the wrong action
or problem.

## Things you should know

Tab reaches a clipped one-line approval fact. Left, Right, Home and End scroll it. The extra Tab stop appears only when
there is something to scroll.

A confirmation opened at the top of one session closes when another session takes its place. A Suspend prompt also stays
away if its host read completes after the switch. Close all keeps attempting the original session's tunnels after you
switch away. If you return while it is working, you can see progress and any refusal in the same tunnel list.

The selected template has a stronger outline in both appearances and its quick-launch hint uses the same readable ink as
the summary. When Restart with or Replace with keeps a field fixed, a search result that tries to set that field opens a
refusal. The standalone message used to start with a lowercase letter and now starts with a capital.

The sidebar's host tooltips preserve the reading direction of the host name and connection address separately. The
duplicate-name message does the same for the template name in both the editor and Save as template.

Mark read and Mark unread refusals used to say “Seen,” and Suspend and Resume refusals said “Sandbox” even for a
container. The refusals now name the chosen action. When the launcher's model list fails, Retry explains that it reloads
Farhelm's model catalog. Open add dialog explains that you review the host's details before adding it. Dropping a file
on a terminal covered by a warning now says it cannot take attachments right now. The old message told you to reconnect
even while the terminal was still connected. A tab whose font cannot be measured now reports a font problem. The advice
to close other tabs still applies past the tab limit.

## Open questions and possible follow-ups

When a host runs a newer or incompatible Farhelm build, the sidebar host tooltip includes build-version help. The source
review found that this help still joins the version and surrounding words without preserving separate text directions.
The fix here covers the alias and address. A reading-order problem in the version help was not reproduced in a rendered
tooltip.

## Pull requests

- Keyboard scrolling for clipped approval facts. [#4238](https://github.com/scode/farhelm/pull/4238/changes)
- Keep header confirmations with the session that opened them.
  [#4240](https://github.com/scode/farhelm/pull/4240/changes)
- Finish Close all across session switches. [#4242](https://github.com/scode/farhelm/pull/4242/changes)
- Give the selected template a visible outline. [#4245](https://github.com/scode/farhelm/pull/4245/changes)
- Make the selected template's quick-launch hint readable. [#4246](https://github.com/scode/farhelm/pull/4246/changes)
- Capitalize held-field refusals in the launcher. [#4247](https://github.com/scode/farhelm/pull/4247/changes)
- Preserve the reading direction of host names and addresses in tooltips.
  [#4248](https://github.com/scode/farhelm/pull/4248/changes)
- Preserve the template name's reading direction in duplicate-name messages.
  [#4249](https://github.com/scode/farhelm/pull/4249/changes)
- Match desktop refusal and help text to the action or cause.
  [#4251](https://github.com/scode/farhelm/pull/4251/changes)

## Checks

- All 428 tests passed on the rebased stack. The recorded `cargo nextest run -p farhelm-gui --lib -E …` selected
  operations, environment actions, the session list's read-status and bulk marking, session confirmations, terminal
  attachments and tabs, approval models and cards, launcher catalog retry, and template collision. Tested head
  `46dcbf63b59c` plus the wording change, source fingerprint `e1eb15aeb083`, recorder
  `dc608f7d-b583-418e-a96e-a207799c0494`. The recorder has the exact selector.
- Ran `cargo fmt --all -- --check`, `cargo clippy -p farhelm-gui --all-targets -- -D warnings`,
  `python3 releasing/check-changelog.py format`, `dprint check` for the triage ledger and index,
  `python -B scripts/check-test-sleeps.py` and `python3 scripts/check-agents-md.py`. The instruction check reported four
  existing warnings and no errors.
- Reused all 235 passes from `cargo nextest run -p farhelm-gui --lib -E …` for host rail, host filters, attention rows,
  template fields, editor, save, dialog and launcher search. Tested head `f64dfb838e50` plus the tooltip and collision
  edits, source fingerprint `b50b69aecfad`, recorder `5380da0a-7d67-47e5-8d9a-339690045a5e`. The rebase preserved those
  product diffs. The rebased run includes the final template collision fixture.
- Reused the 266-picture Linux comparison for the approval change, `python3 scripts/check-gui-goldens.py` at
  `934cf3b0c070` plus the approval fix, source fingerprint `01bc777edebe`, recorder
  `b8d3a481-db34-43ae-8d64-5689d9b814d9`. The relevant pictures matched and the approval fix carried through the rebase
  as tested.
- Captured all 266 Linux pictures using `python3 scripts/check-gui-goldens.py --update` at `bd0db7c34ca9` plus the
  selected-template outline, source fingerprint `c49440f298f7`, recorder `9061a84e-3767-43e3-b754-74a469781f1b`. Only
  the template editor and its leave question changed, each in Dark and Light. Inspected all four and published their
  references. The rebase preserved the four template references and main's separate Settings changes.
- Skipped the wider workspace battery because the changes are confined to the desktop surfaces covered above. Validation
  ran on Linux. Native macOS rendering was outside that environment.

## Review gate

All nine drafts passed independent source review and commit wording checks. Review findings led to a narrower keyboard
stop, a guard for a delayed confirmation, preserving Close all when a session is reopened, and corrections to test
setups and expectations. The final wording change keeps the existing timing for clearing attachment refusals. The
requested wording cases and their checks fit 106 changed lines, counting code, tests and comments.

### Landing

#### What landed with it

It landed in a round of two plans, gui-ux-desktop-fixes and then gui-ux-backend-fixes. Since its base, main gained
exited-session-no-terminal, which shows a recovery card with Restart and Replace in place of the terminal when a
session's terminal is gone. This plan's changes to confirmations at the top of a session and to Close all do not reach
that card: the card has no tunnel list, takes no dropped files, and asks its Replace question inside the card. A Close
all already running keeps going when the card replaces the terminal, and when Restart brings the terminal back, its
tunnel list shows that same Close all. The independent reviewer found no other interaction, and the two plans touch
different code apart from the triage ledger, where their entries were kept side by side.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The nine fixes are each small
and match their triage outcomes. The one that goes further than its outcome asked is Close all: the outcome asked only
that the run keep going after you switch sessions, and the fix also keeps the tunnel list with its "Closing…" state and
refusals so that you see them when you come back. That came from the plan's own review, is documented and tested, and I
left it.

#### Fixes made while landing

- **A dropped file's refusal gave no reason.** When the helm cannot reach a session's host, its terminals stay on screen
  under the "Session unavailable" card (a different card from the recovery card above), and dropping a file on them said
  only "This terminal cannot take attachments right now.", although this plan's changelog entry says these messages now
  explain why. That card is the only thing that blocks a drop this way, so the line now reads "This session is
  unavailable, so nothing was attached", using the card's own word; it has no closing period, like the dropped-file line
  for a disconnected terminal beside it.
- **A code comment was left false.** It still said every refusal appears in the template editor's status line; this plan
  moved the duplicate-name message to its own line.

#### Things you should know that the report does not say

- The report says the selected template has a stronger outline "in both appearances". In Light the new edge is only
  slightly darker than the selected fill, and the triage outcome had said Light was not affected; the visible change is
  in Dark. Nothing needs doing unless the Light selection looks too faint to you.
- Two tests are weaker than their names suggest: the quick-launch hint contrast test checks the color helper's choice
  but would still pass if the hint stopped using that helper, and the approval keyboard-scroll checks sit inside an
  unrelated layout test, where a later change to that layout could drop them by accident. Possible follow-up, not
  urgent.

#### Checks

The two plans change the desktop app's GUI crate (with reference screenshots), the helm, the supervisor, the protocol
crate and one of the command line's end-to-end tests, so the landing ran the whole Rust workspace suite, the desktop
app's own tests, the app's end-to-end smoke and the reference screenshots (goldens). Builds ran locally; the workspace
suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main with the triage ledger's entries kept side by side:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy --locked -p farhelm-gui --all-targets --features agent -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`: passed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats
  (`5342216e-bb7d-42fd-9b91-ca4e678c1f5f`, `ca1387dd-299b-4699-b411-c79e800d0a81`,
  `4c95deaa-83c0-4d00-9b38-8c3b8914fd93`): 8084 of 8087 passed, with no runtime skips. The helm's session-list contract
  test failed because of the placeholder quoting fixed above. The macro compile-contract test failed as it always does
  on a boat, since it resolves crates from the local cargo cache, and passed locally
  (`cargo nextest run -p farhelm-testtrace-macros`, `87c5f9c4-0890-4393-ae5c-9de9ccb0463f`). The test that a scoped Stop
  leaves jobs in a stock tmux server alone failed its own setup check once: the tmux pane's job was still in the
  session's launch scope when the test looked. Nothing in this round touches tmux or launch scopes, and the test passed
  when run again on a boat (`52b91ef2-a4fe-45cd-a7b0-cebe8fb916e8`) and three times locally
  (`5535b0da-c395-498e-a062-aa0ab352c2ff`, `cf0e04f5-5376-41e4-9cd0-1171cc8128e7`,
  `af18a590-134f-478a-a5e5-d5511b49c2ed`), so I treat it as a one-off and have not logged it as a flake.
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`b081f5b1-dce7-46b0-af3e-7d7e0622e433`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, and the clipboard
  paste step was skipped, as always on Linux (`05a7cdf6-eb82-4289-bd54-05d1011e5ca6`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`e46d848a-0c97-43e0-be7a-47b091f995f8`).

On the fixed tree (bare placeholders in displayed commands, the dropped-file line, a comment and a ledger note):

- `cargo fmt --all -- --check`, `dprint check` and `cargo clippy --all-targets -- -D warnings`: passed.
- The protocol crate's launch display tests and the helm's HTTP contract tests,
  `cargo nextest run -p farhelm-proto -p farhelm-helm --lib -E 'test(/session_launch::/) | test(/^http_contract_tests::/)'`,
  locally: 10 passed, the session-list contract test among them (`fc861d2e-ae20-4beb-9a23-2369e95761d3`).
- The GUI's terminal attachment and template tests,
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^terminal::attachments::/) | test(/^templates::/)'`, locally: 343
  passed (`f74ba7dc-fbe0-4a14-85eb-b4e078f493ca`).

The rest of the workspace suite, the desktop app's tests, the smoke and the goldens were reused for the fixed tree.
After them, only the displayed-command rule, the dropped-file line, a comment and a ledger note changed; the focused
runs above cover the first two, and no reference screenshot draws the dropped-file line or a command with a placeholder
(the goldens passed on the delivered tree, which still quoted placeholders).

Skipped:

- The Mac-only checks: no code that only builds for the Mac changed. The fix to the wait when the Mac app replaces its
  supervisor is shared code, and its test ran on Linux with a stand-in for the Mac app's layout; nothing ran on a Mac,
  and the desktop changes were not rendered on macOS.
