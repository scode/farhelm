# The new environment picker omits its open and closed state from accessibility

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The Host picker gives assistive technology no way to tell whether Enter selects a destination or tries to launch.

## Details

**other.** Source inspection.

At 1a53d9d35889d7dde96e31320cc1681dd013737d,
crates/gui/farhelm-gui/src/launcher/view/host_picker.rs:164-194,428-479,536-558,623-674;
crates/gui/farhelm-gui/src/components/button.rs:334-335,376-395,522-534,1067-1069;
crates/gui/farhelm-gui/src/components/select.rs:397-405; vendor/gpui-pre/src/elements/div.rs:3629-3643. At
470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e, crates/gui/farhelm-gui/src/launcher/destination_rows.rs:104-115,137-169 and
components/radio_group.rs:556-560,607-614

The Host combobox does not expose whether its list is open. This matters for the new placement choices because Enter
picks a highlighted host or environment while open and attempts Launch while closed. The accessibility tree has the
trigger's name and labelled list choices, but the trigger never sets Button.expanded. GPUI only exports the state if
present. The ordinary Host trigger already lacked the state at baseline, where the new-environment selection was a
separate Runs on group with exposed selected choices and no popup state to infer. The adjacent Recipe select exposes
expanded state. Use the live layer state to expose expanded and collapsed while preserving the trigger's appearance, as
Button::discloses can do. SPEC.md:1010-1026 at the tip documents the two meanings of Enter. The reviewer suggested
other. Confidence: confirmed accessibility-tree omission and added placement scope; platform-specific announcements and
activation timing were not measured. proposed_drop: null. Possible coverage: distinct from held-host-name-clipped and
LF-1.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-address/LA-11`.
