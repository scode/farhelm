# Highlighting a recent setup removes the red YOLO cue

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

Selecting a recent setup replaces its red YOLO styling with ordinary highlight styling, leaving only the text cue.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view/search/start.rs:487-493;
f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6 crates/gui/farhelm-gui/src/launcher/view/recents.rs:389-397;
1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/launcher/view/search/start.rs:267-304,442-451,497-503,546-565,759-765,
crates/gui/farhelm-gui/src/components/key_chip.rs:7-19, crates/gui/farhelm-gui/src/theme.rs:259,340;
SPEC.md:1145-1146,1386-1389 at pinned tip

The prior strip drew YOLO with the danger foreground even on the focused row. The compact row chooses the generic
inverse selection foreground for YOLO when selected and uses the danger foreground only while unselected. The chip uses
exactly that foreground and its border is generic. This applies to the arrow-highlighted empty-pane recent, the recent
marked as the draft's source, and the selected wide typed recent. A user loses the usual danger color just as the
empty-pane setup becomes ready to submit. The YOLO text remains, and this presentation change does not change launch
admission. SPEC.md specifies that permission danger stays red. The reviewer suggested other, confirmed from source. A
warning treatment with suitable contrast on the selected background could preserve both cues.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-matches/LM-12`.
