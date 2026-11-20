# A mute menu can display an earlier end than it saves

Reviewed commit: `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`

## TLDR

A mute menu left open across the morning cutoff can still say the mute ends today. Picking that option can silence
routine alerts until the next morning.

## Details

The row and bulk menus build the exact Ends text when the submenu opens, then calculate the request from the time the
user selects the item. The submenu keeps its opening items, so the text can keep naming a past cutoff. The click-time
rule in `SPEC.md:2898` is intentional. Refresh the exact expiry while the menu is visible or use wording that describes
the click-time rule. Source: `crates/gui/farhelm-gui/src/session_list/mute.rs:70` and `:94`.

Introduced by `e7587f3b2`. The wrong date follows from the clock logic. The affected selection requires the menu to
remain open across the cutoff, and manual Unmute restores alerts.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g044/f1.
