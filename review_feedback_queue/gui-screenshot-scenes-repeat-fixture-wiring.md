# GUI screenshot scenes repeat fixture setup

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Screenshot scenes each build similar fake sessions and set up dialogs independently. One scene also recreates the update
text it is meant to show.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Share the one-shot dialog setup for screenshots

These scenes observe the fake connection until it has a UI session, run setup once, and retain the connection, fake
helm, and subscription. Share that lifecycle in a scene helper that accepts each scene’s setup callback.

Reviewed locations: `crates/gui/farhelm-gui/src/goldens/hosts.rs:55-85`, :125-146, :154-176, :188-209, :795-817 and
:960-988, `crates/gui/farhelm-gui/src/goldens/other_dialogs.rs:235-253`, :282-299 and :332-350,
`crates/gui/farhelm-gui/src/goldens/templates.rs:52-69`, :129-167 and :208-228,
`crates/gui/farhelm-gui/src/goldens/settings.rs:122-183`. Haiku out-P3.md §1. Suggested bucket **other**.

### Share the timestamped session fixture builder

These builders set the title and derive the same activity and seen timestamps from `TEST_NOW`. Share the common builder
and let scenes set their own statuses.

Reviewed locations: `crates/gui/farhelm-gui/src/goldens/session_list.rs:42-47`, :114-119 and :154-160,
`crates/gui/farhelm-gui/src/goldens/other_dialogs.rs:112-117`, `crates/gui/farhelm-gui/src/goldens/mod.rs:301-307`.
Haiku out-P3.md §1. Suggested bucket **other**.

### Build the reinstall hover from the production formatter

The screenshot scene carries a handwritten copy of the upgrade-floor reinstall advice. Move its formatter to the GUI
crate’s update-readout module, which the desktop crate already uses, so the screenshot exercises the actual words.

Reviewed locations: `crates/gui/farhelm-gui/src/goldens/desktop.rs:371-384`,
`crates/farhelm-desktop/src/updater.rs:588-609`. Haiku out-P3.md §1. Suggested bucket **other**.
