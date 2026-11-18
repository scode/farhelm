# GUI labels and refusals disagree across surfaces

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Several dialogs, menus and accessible labels describe the same action differently. A few messages name the wrong
operation or cause.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Desktop startup

#### Use the product name in the startup alert

The macOS alert title calls the app “farhelm-desktop” while the window calls it “Farhelm”. Use “Farhelm cannot start” in
the alert and retain the executable name as the log tag.

Reviewed locations: `crates/farhelm-desktop/src/lib.rs:1678-1689`, `crates/gui/farhelm-gui/src/bootstrap.rs:66`. Haiku
out-P1.md §5. Suggested bucket **other**.

### Approvals

#### Make approval help match the answer shown

“Open” and “Open add dialog” both show help headed “Allow”, even though the host-add button only opens another dialog.
Give each label its own help and describe the direct opening action specifically when “Tunnel and open” is also offered.

Reviewed locations: `crates/gui/farhelm-gui/src/approvals/model.rs:1132-1139` and :1156-1173,
`crates/gui/farhelm-gui/src/approvals/card.rs:671-680`. Haiku out-P2.md §5. Suggested bucket **other**.

### Launcher

#### Use the same agent-type wording in model and search messages

“Choose a harness”, “Harness default”, and “Agent type” refer to the same choice. Use “agent type” in the launcher’s
instructions, and match the capitalization of “model ID” between the model-field and search messages.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/model.rs:20-21`,
`crates/gui/farhelm-gui/src/launcher/request.rs:323-327`, `crates/gui/farhelm-gui/src/launcher/search.rs:746-757`,
`crates/gui/farhelm-gui/src/launcher/view/model_field.rs:49-50` and :258-267,
`crates/gui/farhelm-gui/src/launcher/view/search.rs:271-278`, `crates/gui/farhelm-gui/src/launcher/view.rs:2092`. Haiku
out-P4.md §5, out-P5.md §5. Suggested bucket **other**.

#### Name Restart with and Replace with as the menus do

The launcher tells users to reopen “restart with”, and held-field errors shorten the names to “restart” and “replace”.
Use the full action labels where the message is referring to those launcher modes.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/request.rs:378-381` and :434-438,
`crates/gui/farhelm-gui/src/launcher/view/search.rs:226-235`, `crates/gui/farhelm-gui/src/launcher/entry.rs:389-405`.
Haiku out-P4.md §5, out-P5.md §5. Suggested bucket **other**.

#### Name the model catalog as the source of the retry

The retry tooltip says it asks the host for its model list, but Retry reads the helm’s catalog regardless of the
selected host. Say “Retry loading the model catalog”.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/view.rs:32-36`, :477-510 and :1983-1987. Haiku out-P4.md §5.
Suggested bucket **other**.

#### Align the launcher search name and its hint

The visible search hint includes names and hosts, which the accessible name omits, while the accessible name includes
efforts. Use one shared description so assistive technology users learn about the same search targets.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/view/search.rs:64-67`, :269-278 and :309-328. Haiku out-P5.md
§5. Suggested bucket **other**.

### Sessions

#### A Mark unread refusal is labeled “Seen”

The manual actions are “Mark read” and “Mark unread,” but both refusals start with “Seen.” Carry the toggle direction
into the outcome so the refusal names the action the user chose.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/seen.rs:55-61`,
`crates/gui/farhelm-gui/src/session_list/marking.rs:783-789`,
`crates/gui/farhelm-gui/src/session_list/outcome.rs:63-72`. Haiku out-P6.md §5. Suggested bucket **other**.

#### Container Suspend and Resume errors say “Sandbox”

The shared session actions work for containers too, but their refusal prefix names a sandbox. Use “Suspend” and
“Resume,” the action names that work for either provider.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/outcome.rs:81-85`,
`crates/gui/farhelm-gui/src/session_list/environment.rs:600-621`, `SPEC.md:4865-4868`. Haiku out-P6.md §5. Suggested
bucket **other**.

#### The interrupted card drops the ellipsis from “Replace with…”

Both controls open the same launcher and the spec names the card's action “Replace with…”. Use that label on the card
too.

Reviewed locations: `crates/gui/farhelm-gui/src/session_pane/card.rs:446-465`,
`crates/gui/farhelm-gui/src/session_pane/header.rs:311-316`, `SPEC.md:2306-2309`. Haiku out-P7.md §5. Suggested bucket
**other**.

### Templates

#### A failed template collection read has two labels

The Templates dialog says “Templates unavailable” for the same collection failure that template search reports as
“Couldn't load templates.” Share the lead-in and retain the surface-specific advice after the helm's explanation.

Reviewed locations: `crates/gui/farhelm-gui/src/templates/dialog.rs:1388-1399`,
`crates/gui/farhelm-gui/src/templates/rows.rs:65-73`, `crates/gui/farhelm-gui/src/templates/dispatch.rs:221-232`,
`crates/gui/farhelm-gui/src/templates/dispatch.rs:295-302`. Haiku out-P8.md §5. Suggested bucket **other**.

### Terminal

#### A veiled terminal can report a connection loss on file drop

A file dropped on the veil over an inert terminal gets “This terminal is not connected … reconnect and try again,” even
when the stream is live. Give the inert case its own refusal that explains that the terminal cannot take attachments
right now.

Reviewed locations: `crates/gui/farhelm-gui/src/terminal/attached.rs:1667-1678`,
`crates/gui/farhelm-gui/src/terminal/attached.rs:2460-2466`,
`crates/gui/farhelm-gui/src/terminal/attachments.rs:242-245`. Haiku out-P8.md §5. Suggested bucket **other**.

#### An unmeasurable tab font can look like a tab limit

A tab can have no renderer because measuring its font failed, but the placeholder always says there are more than 32
tabs and tells the user to close some. Distinguish the font failure and keep the existing message for tabs beyond the
attachment cap.

Reviewed locations: `crates/gui/farhelm-gui/src/terminal/set.rs:88-96`,
`crates/gui/farhelm-gui/src/terminal/set.rs:506-531`, `crates/gui/farhelm-gui/src/terminal/set.rs:899-910`. Haiku
out-P8.md §3. Suggested bucket **other**.
