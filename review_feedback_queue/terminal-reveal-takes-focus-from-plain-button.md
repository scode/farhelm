# A terminal finishing its load takes focus from a button, so the next Enter goes to the agent

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

When a session's terminal finishes loading its history, it takes keyboard focus even if you just moved focus to an
ordinary button in the session view with the keyboard. If the terminal appears between you focusing that button and
pressing Enter or Space, the key goes to the agent instead of pressing the button. If the agent is showing its own
yes/no prompt at that moment, the stray Enter can accept that prompt's default answer, which for a tool-permission
prompt is usually yes.

## Details

Found by the 2026-10-22 prioritized persistent review, group `132-terminal-script-5of6` (finding f1).

Suggested bucket: high. It could arguably be highest if an unintended approval of an agent's destructive tool call
counts as loss of user-owned work or a trust-boundary consequence; flagged so triage decides.

Confidence: likely. The focus surrender is confirmed from the code. Unverified premises: that a focused non-owner button
is plausibly present in the session view while a catch-up is pending, and that the gap between focus and keypress is hit
in practice.

Mechanism (line numbers rechecked at the reviewed commit; the reviewer's were a few lines off):

- `crates/farhelm-ui/assets/terminal.js:4476` (`reveal`) calls `term.focus()` whenever `takesFocus()` allows it. The
  reveal fires at a time the user did not choose: when the catch-up replay ends (marker, idle, size bounds), or late
  through `noteAttachProved` (terminal.js:4332).
- `takesFocus` (terminal.js:4550, doc comment just above) refuses focus for editable controls, another island's
  terminal, open modal dialogs, and the cases covered by `keyboardHeldElsewhere` (terminal.js:1118: approval cards, the
  sidebar separator, the bell flyout, menus, `[data-terminal-keyboard-owner]` panels). Any other focused button is
  deliberately surrendered; the doc comment names the tab-strip selector's select-then-type flow as the reason.
- The same rule governs `focusAfterAvailability` (terminal.js:4571).
- Trigger: the user focuses a non-owner button by keyboard (Tab) in the selected session's view while its terminal is
  still catching up, and the reveal lands before the keypress. The keystroke goes through `term.onData`
  (terminal.js:5227) to that session's agent.
- Consequence: an Enter or Space meant for a Farhelm button becomes agent input in the session the user selected. The
  harmful case is an agent TUI showing a permission or confirmation prompt whose default is accept (Claude Code's
  tool-approval prompt defaults to Yes), so the stray Enter approves an agent action, possibly a destructive one, that
  the user never saw or approved.

How to verify: hold the replay with the existing `__farhelmTestReplay.holdMarker` seam, Tab to a header button, release
the catch-up, press Enter, and check that the pty received a carriage return and the button did not fire.

Fix sketch: on reveal, take focus only when the active element is the body, the island itself, or the tab-strip selector
that selected this island, and treat any other focused button as a keyboard owner. Or ignore the first keystroke after a
programmatic focus that arrives within a short interval.

Reviewer's proposed drop, kept for triage: FILTER.md "Races a person would have to win inside a sub-second window" ("The
trigger needs a person to act (click, confirm, submit) inside a window of about a second or less that opens and closes
on its own"). The keystroke stays in the session the user selected, so the filter's exclusion for acting on the wrong
session does not apply. Whether its exclusions for "loss of user data, credentials, processes, or other user-owned work"
or "a security or trust-boundary consequence" apply depends on how triage reads an agent prompt accepting a stray Enter.
Recording did not apply the filter: the gap runs from the user's own focus to their keypress, which need not be
sub-second, and the consequence may fall under those exclusions.

Possible cover, for triage to judge: the code comment above `takesFocus` records the plain-button surrender as a
deliberate choice, but that is not a spec acceptance. SPEC_impl.md (around line 576) says recovery preserves "deliberate
focus in fields, dialogs and other keyboard owners", without saying whether a plain button counts. TRIAGE_OUTCOMES.md
"terminal-recovery-steals-confirmation-focus.md" (fix code, executed in PR #2357) fixed the same focus theft for header,
sidebar and interrupted-card questions; it did not cover ordinary buttons.
