# The host refusal dialog's Remove reads the skip-confirmation setting live at the press

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When Farhelm refuses an action on a host and offers Remove in the refusal dialog, pressing Remove decides whether to
show the "Remove host" confirmation from the "Remove hosts without asking" setting as it is at the press, not as it was
when the dialog was drawn. If that setting was turned on in the meantime (from Settings, another client, or another
host's "Remove, don't ask again") and the dialog has not redrawn, the press forgets the host straight away instead of
asking. The same commit fixed this for the host menu's Remove, but not for this second Remove button.

## Details

7c2cfe02db ("fix: keep host actions tied to what was shown", #4849) fixed this for the host menu; the refusal-dialog
path is pre-existing and was left unchanged. Locations: `crates/gui/farhelm-gui/src/hosts/refusal.rs:249-264` (the
dialog's `host-refusal-remove` button calls `super::remove::start(host, ...)` at line 262); `start` in
`crates/gui/farhelm-gui/src/hosts/remove.rs` (now `start_shown(&view, asks_first(cx), ...)`, a live preference read at
the press). Contrast `hosts/menu.rs:513-542`, where 7c2cfe02db made Remove use the presented `(view, asks)` snapshot
through `remove_frame`.

7c2cfe02db's message lists `trailing-hosts-menu-remove-reads-live-skip-confirmation.md` as fixed: "Remove and Add can
also pick up a changed confirmation preference just before a press. Farhelm ignores the press until the frame carrying
the updated action or preference has been shown." It added `remove_frame` to the menu and a `start_shown(drawn, asks)`
entry point, but kept `remove::start(host, mark, window, cx)` as `start_shown(&view, asks_first(cx), ...)`, which reads
the preference when the click runs. The refusal dialog's Remove still calls that live-reading `start`, with no
ActionFrame and no captured preference, so a press handled after a preference push and before (or without) a redraw
skips the confirmation the dialog's last frame implied. The provider's Delete in the host menu also calls `start`, but
provider hosts always ask (`removal.deletes.is_some() || asks` in `remove.rs`), so that path is unaffected.

Verify by reading `refusal.rs:249-264` and `start` and `start_shown` in `remove.rs`. Fix: capture
`remove::asks_first(cx)` (and the host view) in the refusal dialog's render through an ActionFrame, as the menu now
does, call `start_shown` with the shown values, and ignore the press until that snapshot has been presented.

Bucket: other. An ordinary SSH host is forgotten without its confirmation in a narrow race with the user's own
preference change; the user did press Remove on that host. Removal disposes of the host's last-known sessions from the
registry (SPEC.md host registry paragraph: "Last-known sessions of a host that is permanently gone are disposed of by
removing the host from the registry"). Could be highest: unclear, leaning no. Forgetting the entry drops its last-known
session records; whether that loses user-owned work beyond what re-adding the host restores is not established, and the
press itself was an explicit Remove on that host. The sibling menu item was fixed under the same consequence, and this
records only the unfixed second path.

Confidence: confirmed from source (`refusal.rs` calls the live-reading `start`).

Possible cover: the queue item 7c2cfe02db fixed (`trailing-hosts-menu-remove-reads-live-skip-confirmation.md`) may have
named only the menu; if it was archived as fixed, this sibling path is not covered by that fix.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g22 record f3; claude-opus-5-5
reviewer, source analysis only, nothing executed.
