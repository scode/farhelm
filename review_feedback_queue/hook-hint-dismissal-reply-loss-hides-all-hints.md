# A discarded dismissal reply can leave all of an agent's hints hidden

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

A lost dismissal reply can hide every hint for that agent until the window closes.

## Details

**other.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b crates/gui/farhelm-gui/src/session_pane/hook_band.rs:59-75,78-109,131-137

When the window disconnects before Don't show again settles, a local saving flag can hide every eligible hint for that
agent until the window closes. This is an operation-delivery outcome defect. The click uses the preferences operation
key; success starting it sets saving at hook_band.rs:89-105, while the callback is the only code that clears it.
Crates/gui/farhelm-gui/src/session.rs:203-218,484-492,534-548 retires the old UI session, and operations.rs:1005-1011
drops its waiting tasks. A reconnect creates a new store at session.rs:224-247 while the pane and band survive
(main_view.rs:99-101,140-143; session_pane.rs:377-380,392-394). HookBand::follow reacts to the new preference but saving
still vetoes draw even when that preference says to show the offer. The request may have reached the helm or not, as
SPEC.md at pinned tip 1a53d9d35889d7dde96e31320cc1681dd013737d:5500-5513 explicitly allows. If the write did not
persist, the next snapshot cannot reveal the hint; if it did persist, a later successful restoration still cannot reveal
it. This defeats SPEC.md:3098-3101 and does not require a later run or a refused response. The scope is the pane's
optimistic flag; the Settings controls and launcher each have their own state. Reset or derive the flag from the active
GUI session's operation. The reviewer suggested other; confidence confirmed by source. Reviewed at
bb31959c2f22c96bbbb72da2e60eee4a0b288a6b and still present on pinned main.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-session/HS-02`.
