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
