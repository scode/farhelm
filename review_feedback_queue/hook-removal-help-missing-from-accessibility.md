# Settings omits the hook removal steps from its accessibility tree

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

The Grok and Antigravity Settings pages can expose their setup switches without the persistence and removal guidance.

## Details

**other.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b crates/gui/farhelm-gui/src/settings/hook_settings.rs:9-10,26-53;
crates/gui/farhelm-gui/src/settings/card.rs:103-110; crates/gui/farhelm-gui/src/components/checkbox.rs:265-301,308-315;
crates/gui/farhelm-gui/src/components/mod.rs:345-364; vendor/gpui-pre/src/elements/text.rs:84-95,330-350,380-385;
vendor/gpui-pre/src/_accessibility.rs:58-66

Someone using a screen reader can find the Grok or Antigravity hook switch, but the Settings page does not expose the
path they need to remove the remaining hook files. The visible instructions and persistence warning are rendered through
card::help as plain SharedString under unannotated divs. SharedString supplies no element ID and GPUI requires an ID and
role to expose a node. Checkbox explicitly names only the short label and exposes only the tooltip as its description.
That tooltip says Farhelm installs hooks and omits their persistence and removal steps. SPEC.md at
1a53d9d35889d7dde96e31320cc1681dd013737d:3085-3090 says the setting names the path to remove. Give the supporting copy
accessible text IDs and roles or provide a complete accessible description with the same content. Correcting the removal
wording or the search result names does not expose the text on this page. Source confirms the omission for both Settings
pages. Platform speech and any alternate discovery behavior are unmeasured. The reviewer suggested other for
inaccessible maintenance instructions with a separate documentation path. No data loss or unintended setting write is
established by this route.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-settings/HS-9`.
