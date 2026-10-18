# Automatic notification dismissal loses keyboard focus

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Resizing the window or changing the sidebar layout while reading a session's notifications can close the list without
returning keyboard focus to its bell.

## Details

The list focuses its dialog on mount (`crates/farhelm-ui/src/list/bell.rs:257–261`), but the bell's open-state effect
handles only opening (`132–138`). Escape and Clear explicitly restore focus (`152–164,218–227`). Automatic closures at
`list/view.rs:1091–1104,1196–1201,1442–1447` do neither. Row-menu teardown (`list/row.rs:1405–1474`) observes menu and
rename state, not bell state; the notification read-on-close effect (`list/view.rs:3131–3215`) only records what was
read. Terminal synchronization focuses on a selected-terminal change (`assets/terminal.js:3050–3053`), which resizing a
stable selected session does not require.

Give automatic closure a focus handoff when the disappearing notification list owned focus, using the surviving bell or
a defined fallback if its row disappeared. Preserve focus independently moved to another control and avoid scrolling.

Suggested bucket: **other**. Confidence: **likely**; missing handoff is traced statically, but browser fallback and the
next Tab destination are untested. No permanent keyboard trap or fixed next-Tab destination is claimed.

Possible cover: none exact. `SPEC.md` Status and `SPEC_impl.md`'s row-menu keyboard contract describe the surrounding
interaction without accepting this loss. The existing update-control replacement finding concerns a different control
and trigger. Planned, BUGS, the queue and the ledger have no matching entry. Normal resizing or a layout update does not
itself establish the rare trigger required by the transient-glitch filter.

Proposed drop: none. Provenance: persistent Astra/high review, `ui-preferences-and-navigation-1/f4`.
