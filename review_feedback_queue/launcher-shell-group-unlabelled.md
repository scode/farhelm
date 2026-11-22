# The launcher's Shells group is identified only by a visual divider

Reviewed commit: 82c5c23129a913833d0a343026870ccbbc9579e5

## TLDR

The launcher separates Bash and Zsh from agent choices with a divider that exposes no accessible Shells label.

## Details

**other.** Source inspection.

82c5c23129a913833d0a343026870ccbbc9579e5 crates/gui/farhelm-gui/src/launcher/view/choices.rs:203-259;
crates/gui/farhelm-gui/src/components/radio_group.rs:666-688,708-717,762-779; 1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/launcher/view/choices.rs:204-260;
crates/gui/farhelm-gui/src/components/radio_group.rs:676-698,718-727,776-796

The launcher distinguishes Bash and Zsh from agent chips with a divider that has no accessible equivalent inside the
chooser. Their buttons are named Bash and Zsh (launcher/draft.rs:1431-1444) and get the generic 'Use … for the session'
description, just like the agent chips. The shared group intentionally omits a role and name, and the shell tail adds
only a divider, wrapper and debug selector. The Agent tab itself mentions plain shells in its help
(launcher/view.rs:2473-2478), so the information is available there, but there is no announced grouping or
shell-specific description at the chips. Before 82c5c23129a913833d0a343026870ccbbc9579e5,
42de88aa6135204857890791d078352985076fb1 launcher/view/choices.rs:197-221,247 and launcher/view.rs:1483-1506 rendered a
separate visible Shells heading and group hover help. The old container was also generic, so no prior named
accessibility group is claimed. Pinned SPEC.md:1053-1059 distinguishes the Shells group after the agent chips, and
:1439-1442 specifies the visual divider. Keep the divider and expose the Shells group name or provide shell-specific
option descriptions. The reviewer suggested other. Confidence confirmed for authored labels and semantics; exact
assistive-technology phrasing was not checked. ; the visual divider meets the visual layout requirement.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-controls/LC-08`, with
`launcher-contract/LC-11`.
