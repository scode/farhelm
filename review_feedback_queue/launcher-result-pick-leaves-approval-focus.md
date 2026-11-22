# The contract requires a launcher result click to return focus from an approval card

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Picking a recent setup or template in the wide launcher can leave keyboard input focused on a separate approval card.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:1431-1438,3623-3645,3688-3695; SPEC_impl.md:686-697,704-709,812-826;
crates/gui/farhelm-gui/src/focus.rs:621-638; 14fda80233aa69231c0b44431fc950764fbcdfd1
crates/gui/farhelm-gui/src/launcher/view/search/start.rs:116-142,314-327 and components/option_row.rs:194-208

Contract recheck: allowing approval cards to take focus while a modal is open makes the starting state legitimate.
SPEC_impl.md:823-826 preserves card focus for a closing dialog and refuses the dialog's late focus move while the card
is focused. The empty-space policy in SPEC.md:3694-3695 is similarly for a click on empty space. A mouse pick on a
launcher row is an explicit user action, and SPEC.md:1436 requires that pick to return focus to the command field. This
does not require relaxing the coordinator's documented in-layer origin rule for programmatic moves. The row requests a
move through the wrong contract for this pointer handoff, and remains a confirmed other-priority finding. Use the
approved pointer focus route with visibility and top-layer checks, keeping the late-result protection. No destructive
resulting action is established. ;

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-command/LC-18`.
