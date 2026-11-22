# Enter silently stops working when a highlighted recent vanishes

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

When the highlighted recent setup disappears, Enter can stay bound to the missing choice and do nothing.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view/history.rs:71-97,249-256,
crates/gui/farhelm-gui/src/launcher/view/search.rs:178-187,241-256,607-617,
crates/gui/farhelm-gui/src/launcher/view/search/start.rs:102-114,239-268; 1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/launcher/view/search.rs:182-191,245-260,607-617 and
crates/gui/farhelm-gui/src/launcher/view/search/start.rs:106-118,243-271; SPEC.md:1136-1140,1431-1439 at pinned tip

After a connected host changes connection or drops, its recent setups disappear. The history reset retires only typed
picks, leaving the empty-field highlight key. With no highlighted row drawn and the header still advertising Enter to
launch, every Enter takes the missing-key branch, sends no launch, and shows no explanation, including when the ordinary
draft can launch after a reconnect whose history lacks that setup. Up or an outside click clears the invisible state, or
the footer action remains usable. Preserve the safety of consuming the stale answer, but make the loss visible and
restore ordinary Enter after the changed action is presented. The reviewer suggested other, confirmed keyboard recovery
and diagnostic defect.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-matches/LM-06`, with
`launcher-matching-contract/LMC-05`.
