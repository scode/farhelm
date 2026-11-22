# Enter on an unavailable recent setup gives no keyboard explanation

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Enter on a disabled setup can do nothing without showing the refusal reason to a keyboard user.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/launcher/view/search/start.rs:87-135,248-270,287-331;
components/option_row.rs:1-18,175-177,189-208; components/mod.rs:359-384; view/search.rs:274-287. Introduced at
14fda80233aa69231c0b44431fc950764fbcdfd1.

Arrow keys can highlight an unavailable row while focus stays in the command editor. Enter silently returns on the empty
command pane. The typed search path can show the refusal below the field. Pointer hover and the accessibility
description both carry the reason. The option exposes its role and selection with no separate disabled state. Source
review did not establish how a screen reader announces the option. Show the reason on a keyboard activation and expose
the same availability that the row displays.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-command/LC-10`, with
`launcher-matching-contract/LMC-10`.
