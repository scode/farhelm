# A Codex dialog with its last option highlighted shows as idle

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

When Codex shows a dialog such as "trust this folder?" and the user has moved the highlight to the last option, Farhelm
shows the session as idle instead of waiting on the user, so it does not draw the user back to it.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F13 / COR-CODEX-LAST-OPTION`, tagged **definite**. Anchor and title:
`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:473` — a Codex dialog with its last option highlighted reads
as Idle instead of Waiting.

The supervisor decides whether a Codex session is working, waiting for the user, or idle partly by reading the visible
screen text. Codex shows its dialogs, such as "trust this folder?" and permission requests, as numbered menus in place
of the input box, with a `›` marking the highlighted option. Codex's input box also starts with `›`. To tell the two
apart, the reader treats a `›` row as a dialog only if it is itself a numbered option and at least one more numbered
option appears below it (`codex_dialog_may_be_open`,
`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:473-495`). Only when that test passes does the reader look at
the bottom rows for dialog footers such as "enter continue" or "esc to cancel", which mark the screen as Waiting.

That test fails when the user has moved the highlight to the last option. The pinned trust-dialog capture
(`tests/fixtures/screens/codex/0.159.0/waiting-trust.txt`) ends with `› 1. Trust and continue`,
`2. Back to Agent Command Center`, and `enter continue · esc back`. After the user presses the down arrow, the `›` sits
on option 2, and the only thing below it is the footer, which is not a numbered option. So the reader decides no dialog
is open and never looks at the footer. It falls through to the input-box check (`codex_has_composer`, around line 508),
which accepts any `›` row followed only by indented rows, and reports Idle. Codex's permission dialog escapes this
because its pane title starts with "Action Required", and the reader checks titles separately. The trust dialog's title
is just the host name, so nothing rescues it.

The user sees a session that is actually blocked on them shown as idle, so the attention cue that should bring them back
never fires. The suggested fix is to also accept a numbered option directly above the `›` row as proof of a menu. A
stricter alternative is to require every non-blank row from `›` down to be either an option or a known dialog footer.
Either way, add a fixture with the last option highlighted.
