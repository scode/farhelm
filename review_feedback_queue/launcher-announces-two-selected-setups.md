# The recent and template list reports two selected options for one Enter target

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The launcher can report both the setup that filled the form and the current keyboard target as selected.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d:crates/gui/farhelm-gui/src/launcher/view/search/start.rs:267–271,319–345;
crates/gui/farhelm-gui/src/components/option_row.rs:117–121,175–177

After a setup or template fills the form, assistive technology can receive two selected options as the user arrows to
another row, even though only one responds to Enter. The code ORs the remembered source with the keyboard highlight then
sends both through OptionRow::selected and aria_selected to one ListBox. The persistent source mark is intentional
(SPEC.md at the reviewed hash:1431–1435), but it should be conveyed separately from the current keyboard option or
distinguished in its accessible description. The reviewer suggested other; confidence confirmed for two selected nodes,
screen-reader behavior unverified; ;

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-match-actions/start-list-aria-selection`, with `launcher-search/LS-07`.
