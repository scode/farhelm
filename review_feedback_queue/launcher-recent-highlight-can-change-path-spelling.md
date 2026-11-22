# A recent setup can change its literal folder under a retained highlight

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A refreshed recent setup can keep the same highlight while changing the literal folder spelling Enter will use.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d:crates/gui/farhelm-gui/src/launcher/recents.rs:73–83,96–124,171–186,245–251;
crates/gui/farhelm-gui/src/launcher/view/search/start.rs:107–115;
crates/gui/farhelm-gui/src/launcher/view/recents.rs:111–119; crates/gui/farhelm-gui/src/launcher/request.rs:627–632

Recent setups group by the remembered canonical folder and choose a recent representative. The representative can have a
different literal path. The retained keyboard key still matches the group, and Enter uses the representative's current
literal folder. The supervisor resolves that string when it creates the session. A different actual folder or lost work
would also require the aliases to resolve differently, which remains unverified. See
`launcher/recents.rs:73–124,151–186`, `launcher/view/search/start.rs:107–115` and `launcher/view/recents.rs:111–119`.
Either keep the presented representative bound to the action or require a new choice if its literal folder changes.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-match-actions/recent-alias-representative-drift`.
