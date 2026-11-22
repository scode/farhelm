# Pi’s Settings page describes the wrong trust action

Reviewed commit: 4949a79e8465ce1f25e0e5c42a1e76b80730ce0b

## TLDR

Pi’s Settings description promises folder-trust behavior even though the setting approves project resources.

## Details

**other.** Source inspection.

crates/gui/farhelm-gui/src/settings/frame.rs:104–119 and settings/agent_trust.rs:176–183, 231–249, 279–288,
SPEC.md:1770–1777, 1787–1793 at 4949a79e8465ce1f25e0e5c42a1e76b80730ce0b

The shared Settings frame describes the Pi page in terms of folder-trust questions. Pi's own switch and help describe
approving the project resources used by its launches, consistent with the separate product contract and absence of a
recognized Pi trust question. Adjust the page description while preserving the switch's specific help. See
`crates/gui/farhelm-gui/src/settings/frame.rs:104–124,390–417`, `settings/agent_trust.rs:176–183,231–249,279–288`, and
`SPEC.md:1770–1777,1787–1793`.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`agent-settings/agent-settings-pi-heading`.
