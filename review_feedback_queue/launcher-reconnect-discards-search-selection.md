# A reconnect can replace an arrow-picked history result with a different action on Enter

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

A reconnect can clear the selected search result and make Enter apply a different result.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view/history.rs:73–97;
crates/gui/farhelm-gui/src/launcher/view/search.rs:170–205, 263–267;
crates/gui/farhelm-gui/src/launcher/search.rs:635–645, 688–719, 728–740;
crates/gui/farhelm-gui/src/launcher/view.rs:2113–2128

If the selected host reconnects while the user has a history result selected, Enter can apply a different search result.
For an unlabelled model word unique to history it can instead set the session name. follow_history clears the old
response and calls reset_host_history_highlight, which clears the explicit pick instead of leaving the missing selection
unarmed. The typed results remain open, search_highlight takes the fresh default, and search_enter applies it. GPUI can
redraw between these keys without showing that frame (SPEC_impl.md at 1a53d9d35889d7dde96e31320cc1681dd013737d:984–988,
1033–1035). SPEC.md at that tip:1136–1149 specifies that Enter picks the visible highlighted row. No direct launch
follows a typed-result pick. The tip retains the reset (launcher/view/history.rs:84–96 and
launcher/view/search.rs:174–206, 268–271). Preserve a lost selection as unarmed until navigation or a new query. The
reviewer suggested other for an unexpected, reversible draft edit during a reconnect, confidence confirmed, , Possible
coverage: LS-04/LS-06 share the presentation issue but reach different actions.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-search/LS-02`.
