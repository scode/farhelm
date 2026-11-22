# A recent setup can inherit the previous occupant’s hover help

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A recent setup can briefly show hover help for the previous setup in its list position.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d:crates/gui/farhelm-gui/src/launcher/view/search/start.rs:251–320;
crates/gui/farhelm-gui/src/components/option_row.rs:86–89,205; crates/gui/farhelm-gui/src/components/mod.rs:345–368;
crates/gui/farhelm-gui/src/layers/tooltip.rs:560–591,648–650

When the recent list changes under a stationary pointer, the visible or pending tooltip can describe a different setup’s
full folder, wrapper and permissions. Recent IDs are based on position. They supply the owner ID for help, and an
already pending or visible tooltip keeps the words it started with when the owner stays the same. Live history
completion and draft filtering can change a row without changing its id (launcher/view/history.rs:129–142,
launcher/view/recents.rs:37–66 at the reviewed hash). Stable content IDs are an explicit component contract. The
reviewer suggested other for incorrect detail; confidence confirmed for owner and pending/visible help retention, impact
conditional on a row changing while it is hovered; ;

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-match-actions/recent-positional-option-id`.
