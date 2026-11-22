# A recent-setup highlight omits the installation identity

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The launcher keys an armed recent setup by registry entry and setup, leaving cross-installation rebinding possible if
matching history arrives without resetting that launcher.

## Details

**other.** Investigated uncertainty.

1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:431-441,1386-1389,1405-1410,5533-5552,
SPEC_impl.md:5085-5098,5198-5203, crates/gui/farhelm-gui/src/launcher/view/history.rs:71-97,100-142,184-196,235-256,
view/search.rs:175-191,245-260, view/search/start.rs:48-54,87-118,139-146,267-271, view/recents.rs:69-88,102-124,
view.rs:1154-1164,2127-2143, draft.rs:388-405,1047-1077, request.rs:598-638,
crates/farhelm-helm/src/sessions.rs:730-763,1955-1987,2162-2171,2309-2345 and
crates/farhelm-helm/src/store.rs:8666-8696,9430-9441,9658-9682

The stored highlight key contains the logical host row and setup identity. A history refresh can preserve that key while
rebuilding the offered rows. The uninstall or adoption route to an actual successor launch still requires a surviving
launcher, fresh matching eligible history and authorization to use the successor installation. Adoption retires
predecessor history, and the UI normally gates adoption and launcher use separately. Source inspection did not establish
all of these conditions together. The confirmed key omission is at `launcher/view/search/start.rs:64–76,107–115` and
`launcher/recents.rs:171–186`. Bind the highlight to the same installation identity as the remembered setup so its
lifetime does not rely on indirect UI transitions.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-matches/LM-05`, with
`launcher-search/LS-08`, `launcher-matching-contract/LMC-06`,
`launcher-match-actions/recent-highlight-cross-install-rebind`.
