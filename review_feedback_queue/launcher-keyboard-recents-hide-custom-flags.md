# Keyboard navigation no longer reveals the custom flags that distinguish recent setups

Reviewed commit: f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6

## TLDR

Keyboard users can no longer reveal the custom permission flags that distinguish otherwise identical recent setups.

## Details

**other.** Source inspection.

f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6 crates/gui/farhelm-gui/src/launcher/view/recents.rs:251–308, 365–396;
1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/launcher/view/search/start.rs:83–116, 273–304,
320–332, 450–477; crates/gui/farhelm-gui/src/components/button.rs:1290–1298;
crates/gui/farhelm-gui/src/components/option_row.rs:194–207; crates/gui/farhelm-gui/src/launcher/recents.rs:96–123,
279–285, 340–355; crates/gui/farhelm-gui/src/launcher/draft.rs:588–608

Two enabled recent setups differing only in Custom permission flags look the same to a sighted keyboard user. Grouping
keeps the different saved flags separate and taking either restores its flags. The compact summary shows the mode but
omits the words. The previous strip likewise used a compact summary, but selecting a row with the keyboard focused a
button whose help contained the flags. The new pane keeps focus on the command editor and gives the option's full help
no focus handle, so its keyboard highlight cannot reveal that help. Pointer hover and accessible row names still carry
the full choices. SPEC.md:1386–1389 and 1431–1436 permit Enter to submit the enabled highlighted setup and leave the
editor focused. SPEC_impl.md:886–895 documents old focus help. Provide keyboard access to the distinguishing details
without adding a Tab stop. The reviewer suggested other for loss of inspectability before keyboard submission,
confidence source-confirmed, tooltip rendering and actual user confusion unverified. ; Possible coverage: LC-10 concerns
disabled rows' refusal help, while this concerns distinguishing enabled setups with different saved settings.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-search/LS-14`.
